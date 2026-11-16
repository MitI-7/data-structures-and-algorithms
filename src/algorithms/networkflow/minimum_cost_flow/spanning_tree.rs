use crate::numeric::number::Number;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    NotSolved,
    BadInput,
    Unbalanced,
    Infeasible,
    Optimal,
}

// Sealed because lossy conversions would invalidate pricing's bounds checks.
mod index_sealed {
    pub trait Sealed {}
    impl Sealed for u8 {}
    impl Sealed for u16 {}
    impl Sealed for u32 {}
    impl Sealed for usize {}
}

/// Storage width for graph indices; arithmetic still uses `usize`.
/// The largest value is reserved for missing node/edge links.
pub trait Index: index_sealed::Sealed + Copy + Default + Eq {
    const MAX: usize;
    fn from_usize(value: usize) -> Self;
    fn to_usize(self) -> usize;
}
macro_rules! impl_index {
    ($($t:ty),*) => {$(
        impl Index for $t {
            const MAX: usize = <$t>::MAX as usize;
            #[inline(always)] fn from_usize(value: usize) -> Self {
                debug_assert!(value <= <Self as Index>::MAX);
                value as Self
            }
            #[inline(always)] fn to_usize(self) -> usize { self as usize }
        }
    )*};
}
impl_index!(u8, u16, u32, usize);

#[derive(Clone)]
pub struct Node<N: Index = usize, E: Index = usize> {
    pub parent: N,
    pub parent_edge_id: E,
}

impl<N: Index, E: Index> Node<N, E> {
    fn empty() -> Self {
        Self {
            parent: N::from_usize(N::MAX),
            parent_edge_id: E::from_usize(E::MAX),
        }
    }

    #[inline(always)]
    pub fn parent(&self) -> usize {
        let p = self.parent.to_usize();
        if p == N::MAX { usize::MAX } else { p }
    }

    #[inline(always)]
    pub fn parent_edge_id(&self) -> usize {
        let e = self.parent_edge_id.to_usize();
        if e == E::MAX { usize::MAX } else { e }
    }

    #[inline(always)]
    fn set_parent(&mut self, value: usize) {
        debug_assert!(value == usize::MAX || value < N::MAX);
        self.parent = N::from_usize(if value == usize::MAX { N::MAX } else { value });
    }

    #[inline(always)]
    fn set_parent_edge_id(&mut self, value: usize) {
        debug_assert!(value == usize::MAX || value < E::MAX);
        self.parent_edge_id = E::from_usize(if value == usize::MAX { E::MAX } else { value });
    }

    #[inline(always)]
    fn set(&mut self, p: usize, e: usize) {
        self.set_parent(p);
        self.set_parent_edge_id(e);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeState {
    Upper,
    Tree,
    Lower,
}

#[derive(Clone, Debug)]
pub struct Edge<Flow> {
    pub from: usize,
    pub to: usize,
    pub lower: Flow,
    pub upper: Flow,
    pub cost: Flow,
    pub flow: Flow,
}

pub struct InternalEdge<Flow, V: Index = u32> {
    pub from: V,
    pub to: V,
    pub upper: Flow,
    pub cost: Flow,
    pub flow: Flow,
    pub state: EdgeState,
}

impl<Flow, V: Index> InternalEdge<Flow, V>
where
    Flow: Number,
{
    #[inline(always)]
    pub fn from(&self) -> usize {
        self.from.to_usize()
    }

    #[inline(always)]
    pub fn to(&self) -> usize {
        self.to.to_usize()
    }

    pub fn is_feasible(&self) -> bool {
        Flow::zero() <= self.flow && self.flow <= self.upper
    }

    pub fn is_lower(&self) -> bool {
        self.flow == Flow::zero()
    }

    pub fn is_upper(&self) -> bool {
        self.flow == self.upper
    }

    pub fn residual_capacity(&self) -> Flow {
        self.upper - self.flow
    }

    pub fn opposite_side(&self, u: usize) -> usize {
        u ^ self.to() ^ self.from()
    }
}

pub(crate) struct PricingData<Flow, V: Index> {
    pub froms: Vec<V>,
    pub tos: Vec<V>,
    pub costs: Vec<Flow>,
}

#[derive(Default)]
pub(crate) struct IndexVec<I: Index> {
    data: Vec<I>,
}

impl<I: Index> IndexVec<I> {
    fn encode(value: usize) -> I {
        I::from_usize(if value == usize::MAX { I::MAX } else { value })
    }

    fn with_capacity(n: usize) -> Self {
        Self {
            data: Vec::with_capacity(n),
        }
    }

    fn push(&mut self, v: usize) {
        self.data.push(Self::encode(v));
    }

    fn resize(&mut self, n: usize, v: usize) {
        self.data.resize(n, Self::encode(v));
    }

    fn truncate(&mut self, n: usize) {
        self.data.truncate(n);
    }

    #[inline(always)]
    pub(crate) fn get(&self, i: usize) -> usize {
        self.data[i].to_usize()
    }

    #[inline(always)]
    pub(crate) fn set(&mut self, i: usize, v: usize) {
        self.data[i] = Self::encode(v);
    }
}

#[derive(Default)]
pub struct SpanningTreeStructure<Flow, V: Index = u32, N: Index = usize, E: Index = usize> {
    pub(crate) num_nodes: usize,
    pub(crate) num_edges: usize,
    pub(crate) b: Vec<Flow>,
    pub(crate) excesses: Vec<Flow>,
    pub(crate) lowers: Vec<Flow>,
    pub(crate) is_rev: Vec<bool>,

    pub(crate) nodes: Vec<Node<N, E>>,
    pub(crate) potentials: Vec<Flow>,
    pub(crate) edges: Vec<InternalEdge<Flow, V>>,

    pub(crate) next_node_dft: IndexVec<N>, // next nodes in depth-first thread
    pub(crate) prev_node_dft: IndexVec<N>, // previous nodes in depth-first thread
    pub(crate) last_descendant_dft: IndexVec<N>, // last descendants in depth-first thread
    ancestors: Vec<usize>,
    pub(crate) num_successors: IndexVec<N>,
    pricing_data: OnceLock<PricingData<Flow, V>>,
}

impl<Flow, V: Index, N: Index, E: Index> SpanningTreeStructure<Flow, V, N, E>
where
    Flow: Number,
{
    pub fn with_capacity(num_nodes: usize, num_edges: usize) -> Self {
        let node_capacity = num_nodes + 3;
        let edge_capacity = num_edges + num_nodes + 4;
        Self {
            b: Vec::with_capacity(node_capacity),
            excesses: Vec::with_capacity(node_capacity),
            lowers: Vec::with_capacity(edge_capacity),
            is_rev: Vec::with_capacity(edge_capacity),
            nodes: Vec::with_capacity(node_capacity),
            potentials: Vec::with_capacity(node_capacity),
            edges: Vec::with_capacity(edge_capacity),
            next_node_dft: IndexVec::with_capacity(node_capacity),
            prev_node_dft: IndexVec::with_capacity(node_capacity),
            last_descendant_dft: IndexVec::with_capacity(node_capacity),
            ancestors: Vec::with_capacity(node_capacity),
            num_successors: IndexVec::with_capacity(node_capacity),
            ..Self::default()
        }
    }

    #[inline]
    pub fn num_nodes(&self) -> usize {
        self.num_nodes
    }

    #[inline]
    pub fn num_edges(&self) -> usize {
        self.num_edges
    }

    pub(crate) fn pricing_data(&self) -> &PricingData<Flow, V> {
        self.pricing_data.get_or_init(|| {
            assert!(
                self.edges
                    .iter()
                    .all(|e| e.from() < self.num_nodes && e.to() < self.num_nodes),
                "remove incident edges before removing their vertices"
            );
            PricingData {
                froms: self.edges.iter().map(|e| e.from).collect(),
                tos: self.edges.iter().map(|e| e.to).collect(),
                costs: self.edges.iter().map(|e| e.cost).collect(),
            }
        })
    }

    pub fn potentials(&self) -> &[Flow] {
        &self.potentials
    }

    pub fn add_node(&mut self) -> usize {
        assert!(
            self.num_nodes < N::MAX && self.num_nodes <= V::MAX,
            "node index capacity exceeded"
        );
        self.b.push(Flow::zero());
        self.num_successors.push(0);
        self.excesses.push(Flow::zero());
        self.nodes.push(Node::empty());
        self.potentials.push(Flow::zero());
        self.next_node_dft.push(usize::MAX);
        self.prev_node_dft.push(usize::MAX);
        self.last_descendant_dft.push(usize::MAX);

        self.num_nodes += 1;
        self.num_nodes - 1
    }

    pub fn add_nodes(&mut self, num_nodes: usize) -> Vec<usize> {
        let new_num_nodes = self
            .num_nodes
            .checked_add(num_nodes)
            .expect("node count overflow");
        assert!(
            new_num_nodes <= N::MAX && new_num_nodes.saturating_sub(1) <= V::MAX,
            "node index capacity exceeded"
        );
        self.b.resize(new_num_nodes, Flow::zero());
        self.num_successors.resize(new_num_nodes, 0);
        self.excesses.resize(new_num_nodes, Flow::zero());
        self.nodes.resize(new_num_nodes, Node::empty());
        self.potentials.resize(new_num_nodes, Flow::zero());
        self.next_node_dft.resize(new_num_nodes, usize::MAX);
        self.prev_node_dft.resize(new_num_nodes, usize::MAX);
        self.last_descendant_dft.resize(new_num_nodes, usize::MAX);

        let node_ids = (self.num_nodes..new_num_nodes).collect();
        self.num_nodes = new_num_nodes;
        node_ids
    }

    pub fn add_directed_edge(
        &mut self,
        from: usize,
        to: usize,
        lower: Flow,
        upper: Flow,
        cost: Flow,
    ) -> Option<usize> {
        if lower > upper
            || from >= self.num_nodes
            || to >= self.num_nodes
            || from > V::MAX
            || to > V::MAX
            || self.num_edges >= E::MAX
        {
            return None;
        }

        self.pricing_data.take();
        if cost >= Flow::zero() {
            self.edges.push(InternalEdge {
                from: V::from_usize(from),
                to: V::from_usize(to),
                upper: upper - lower,
                cost,
                flow: Flow::zero(),
                state: EdgeState::Lower,
            });
            self.excesses[from] -= lower;
            self.excesses[to] += lower;
            self.lowers.push(lower);
            self.is_rev.push(false);
        } else {
            self.edges.push(InternalEdge {
                from: V::from_usize(to),
                to: V::from_usize(from),
                upper: upper - lower,
                cost: -cost,
                flow: Flow::zero(),
                state: EdgeState::Lower,
            });
            self.excesses[from] -= upper;
            self.excesses[to] += upper;
            self.lowers.push(lower);
            self.is_rev.push(true);
        }

        self.num_edges += 1;
        Some(self.num_edges - 1)
    }

    pub fn add_supply(&mut self, u: usize, supply: Flow) {
        self.b[u] += supply;
        self.excesses[u] += supply;
    }

    pub fn get_node(&self, u: usize) -> Option<&Node<N, E>> {
        self.nodes.get(u)
    }

    pub fn get_edge(&self, edge_id: usize) -> Option<Edge<Flow>> {
        let e = self.edges.get(edge_id)?;
        let lower = self.lowers[edge_id];
        if self.is_rev[edge_id] {
            Some(Edge {
                from: e.to(),
                to: e.from(),
                lower,
                upper: e.upper + lower,
                cost: -e.cost,
                flow: e.upper - e.flow + lower,
            })
        } else {
            Some(Edge {
                from: e.from(),
                to: e.to(),
                lower,
                upper: e.upper + lower,
                cost: e.cost,
                flow: e.flow + lower,
            })
        }
    }

    pub fn pop_node(&mut self) {
        self.pop_nodes(1);
    }

    pub fn pop_nodes(&mut self, num_nodes: usize) {
        self.pricing_data.take();
        if self.num_nodes >= num_nodes {
            self.b.truncate(self.num_nodes - num_nodes);
            self.num_successors.truncate(self.num_nodes - num_nodes);
            self.excesses.truncate(self.num_nodes - num_nodes);
            self.nodes.truncate(self.num_nodes - num_nodes);
            self.potentials.truncate(self.num_nodes - num_nodes);
            self.next_node_dft.truncate(self.num_nodes - num_nodes);
            self.prev_node_dft.truncate(self.num_nodes - num_nodes);
            self.last_descendant_dft
                .truncate(self.num_nodes - num_nodes);

            self.num_nodes = self.nodes.len();
        }
    }

    pub fn pop_edge(&mut self) {
        self.pop_edges(1);
    }

    pub fn pop_edges(&mut self, num_edges: usize) {
        if self.num_edges >= num_edges {
            self.pricing_data.take();
            self.edges.truncate(self.num_edges - num_edges);
            self.lowers.truncate(self.num_edges - num_edges);
            self.is_rev.truncate(self.num_edges - num_edges);
            self.num_edges = self.edges.len();
        }
    }

    pub(crate) fn is_unbalanced(&self) -> bool {
        self.excesses
            .iter()
            .fold(Flow::zero(), |sum, &excess| sum + excess)
            != Flow::zero()
    }

    pub fn minimum_cost(&self) -> Flow {
        (0..self.num_edges).fold(Flow::zero(), |sum, edge_id| {
            let edge = self.get_edge(edge_id).unwrap();
            sum + edge.flow * edge.cost
        })
    }

    #[inline]
    pub(crate) fn reduced_cost(&self, edge: &InternalEdge<Flow, V>) -> Flow {
        edge.cost + self.potentials[edge.from()] - self.potentials[edge.to()]
    }

    pub(crate) fn construct_extend_network_feasible_solution(
        &mut self,
    ) -> (usize, std::ops::Range<usize>) {
        let inf_cost = self
            .edges
            .iter()
            .map(|e| e.cost)
            .fold(Flow::one(), |acc, cost| acc + cost); // all edge costs are non-negative

        // add artificial nodes
        let root = self.add_node();
        self.nodes[root].set(usize::MAX, usize::MAX);

        // add artificial edges
        let first_artificial_edge = self.num_edges;
        let mut prev_node = root;
        for u in 0..self.num_nodes {
            if u == root {
                continue;
            }

            let excess = self.excesses[u];
            if excess >= Flow::zero() {
                // u -> root
                let idx = self
                    .add_directed_edge(u, root, Flow::zero(), excess, inf_cost)
                    .unwrap();
                (
                    self.potentials[u],
                    self.edges[idx].flow,
                    self.edges[idx].state,
                ) = (-inf_cost, excess, EdgeState::Tree);
            } else {
                // root -> u
                let idx = self
                    .add_directed_edge(root, u, Flow::zero(), -excess, inf_cost)
                    .unwrap();
                (
                    self.potentials[u],
                    self.edges[idx].flow,
                    self.edges[idx].state,
                ) = (inf_cost, -excess, EdgeState::Tree);
            }

            self.nodes[u].set(root, self.num_edges - 1);
            self.next_node_dft.set(prev_node, u);
            self.prev_node_dft.set(u, prev_node);
            self.last_descendant_dft.set(u, u);
            self.num_successors.set(u, 1);
            self.excesses[u] = Flow::zero();
            prev_node = u;
        }
        self.next_node_dft.set(prev_node, root);
        self.prev_node_dft.set(root, prev_node);
        self.last_descendant_dft.set(root, prev_node);

        self.num_successors.set(root, self.num_nodes);
        (root, first_artificial_edge..self.num_edges)
    }

    pub(crate) fn update_flow_in_cycle(
        &mut self,
        entering_edge_id: usize,
        delta: Flow,
        apex: usize,
    ) {
        let delta = match self.edges[entering_edge_id].state {
            EdgeState::Upper => -delta,
            _ => delta,
        };
        self.edges[entering_edge_id].flow += delta;

        for (mut node, amount) in [
            (self.edges[entering_edge_id].from(), -delta),
            (self.edges[entering_edge_id].to(), delta),
        ] {
            while node != apex {
                let edge = &mut self.edges[self.nodes[node].parent_edge_id()];
                edge.flow += if node == edge.from() { amount } else { -amount };
                node = self.nodes[node].parent();
            }
        }
    }

    // Reverse the path to new_root, preserving the depth-first thread.
    // Then shift the potentials of the whole detached subtree.
    pub(crate) fn reroot_tree(&mut self, new_root: usize, entering_edge_id: usize) {
        self.ancestors.clear();
        let mut now = new_root;
        while now != usize::MAX {
            self.ancestors.push(now);
            now = self.nodes[now].parent();
        }

        // The path was collected from the new root upward. Process it downward.
        for pair in self.ancestors.windows(2).rev() {
            let (p, q) = (pair[1], pair[0]);
            let size_p = self.num_successors.get(p);
            let last_q = self.last_descendant_dft.get(q);

            self.nodes[p].set_parent(q);
            self.nodes[q].set_parent(usize::MAX);
            let old_edge = self.nodes[q].parent_edge_id();
            self.nodes[p].set_parent_edge_id(old_edge);
            self.nodes[q].set_parent_edge_id(usize::MAX);
            self.num_successors
                .set(p, size_p - self.num_successors.get(q));
            self.num_successors.set(q, size_p);

            let prev_q = self.prev_node_dft.get(q);
            let next_last_q = self.next_node_dft.get(last_q);
            self.next_node_dft.set(prev_q, next_last_q);
            self.prev_node_dft.set(next_last_q, prev_q);
            self.next_node_dft.set(last_q, q);
            self.prev_node_dft.set(q, last_q);

            let mut last_p = self.last_descendant_dft.get(p);
            if last_p == last_q {
                self.last_descendant_dft.set(p, prev_q);
                last_p = prev_q;
            }

            self.prev_node_dft.set(p, last_q);
            self.next_node_dft.set(last_q, p);
            self.next_node_dft.set(last_p, q);
            self.prev_node_dft.set(q, last_p);
            self.last_descendant_dft.set(q, last_p);
        }

        // update potential
        let entering_edge = &self.edges[entering_edge_id];
        let delta = if new_root == entering_edge.from() {
            -self.reduced_cost(entering_edge)
        } else {
            self.reduced_cost(entering_edge)
        };

        let last = self.last_descendant_dft.get(new_root);
        let mut now = new_root;
        loop {
            self.potentials[now] += delta;
            if now == last {
                break;
            }
            now = self.next_node_dft.get(now);
        }
    }

    // remove leaving_edge_id
    pub(crate) fn detach_tree(&mut self, sub_tree_root: usize, leaving_edge_id: usize) {
        let leaving_edge = &mut self.edges[leaving_edge_id];
        leaving_edge.state = if leaving_edge.is_lower() {
            EdgeState::Lower
        } else {
            EdgeState::Upper
        };

        // detach sub tree
        self.nodes[sub_tree_root].set_parent(usize::MAX);
        self.nodes[sub_tree_root].set_parent_edge_id(usize::MAX);

        let prev_t = self.prev_node_dft.get(sub_tree_root);
        let last_t = self.last_descendant_dft.get(sub_tree_root);
        let next_last_t = self.next_node_dft.get(last_t);
        self.next_node_dft.set(prev_t, next_last_t);
        self.prev_node_dft.set(next_last_t, prev_t);
        self.next_node_dft.set(last_t, sub_tree_root);
        self.prev_node_dft.set(sub_tree_root, last_t);

        let sub_tree_size = self.num_successors.get(sub_tree_root);
        let mut now = leaving_edge.opposite_side(sub_tree_root);
        while now != usize::MAX {
            self.num_successors
                .set(now, self.num_successors.get(now) - sub_tree_size);
            if self.last_descendant_dft.get(now) == last_t {
                self.last_descendant_dft.set(now, prev_t);
            }
            now = self.nodes[now].parent();
        }
    }

    // Splice the subtree into the DFS thread and update the ancestors.
    pub(crate) fn attach_tree(
        &mut self,
        attach_node: usize,
        sub_tree_root: usize,
        entering_edge_id: usize,
    ) {
        self.edges[entering_edge_id].state = EdgeState::Tree;

        let (p, q) = (attach_node, sub_tree_root); // p -> q

        // attach tree
        self.nodes[q].set_parent(p);
        self.nodes[q].set_parent_edge_id(entering_edge_id);

        let last_p = self.last_descendant_dft.get(attach_node);
        let next_last_p = self.next_node_dft.get(last_p);
        let last_q = self.last_descendant_dft.get(q);
        self.next_node_dft.set(last_p, q);
        self.prev_node_dft.set(q, last_p);
        self.prev_node_dft.set(next_last_p, last_q);
        self.next_node_dft.set(last_q, next_last_p);

        let sub_tree_size = self.num_successors.get(q);
        let mut now = attach_node;
        while now != usize::MAX {
            self.num_successors
                .set(now, self.num_successors.get(now) + sub_tree_size);
            if self.last_descendant_dft.get(now) == last_p {
                self.last_descendant_dft.set(now, last_q);
            }
            now = self.nodes[now].parent();
        }
    }

    pub fn satisfy_constraints(&self) -> bool {
        self.edges.iter().all(InternalEdge::is_feasible)
            && self.excesses.iter().all(|&excess| excess == Flow::zero())
    }

    pub fn satisfy_optimality_conditions(&self) -> bool {
        self.edges.iter().all(|edge| match edge.state {
            EdgeState::Tree => self.reduced_cost(edge) == Flow::zero(),
            EdgeState::Lower => {
                edge.upper == Flow::zero() || self.reduced_cost(edge) >= Flow::zero()
            }
            EdgeState::Upper => {
                edge.upper == Flow::zero() || self.reduced_cost(edge) <= Flow::zero()
            }
        })
    }

    pub fn validate_num_successors(&self, root: usize) -> bool {
        let mut order = Vec::new();
        let mut now = root;
        loop {
            order.push(now);
            now = self.next_node_dft.get(now);
            if now == root {
                break;
            }
        }

        let mut num_successors = vec![1; self.num_nodes];
        for &u in order.iter().rev() {
            if num_successors[u] != self.num_successors.get(u) {
                return false;
            }
            if self.nodes[u].parent() != usize::MAX {
                num_successors[self.nodes[u].parent()] += num_successors[u];
            }
        }

        true
    }
}
