use super::pivot::PivotRule;
use super::spanning_tree::{EdgeState, Index, InternalEdge, SpanningTreeStructure, Status};
use crate::numeric::number::Number;

pub struct PrimalNetworkSimplex<'a, Flow, V: Index = u32, N: Index = usize, E: Index = usize> {
    st: &'a mut SpanningTreeStructure<Flow, V, N, E>,
    root: usize,
}

impl<'a, Flow, V: Index, N: Index, E: Index> PrimalNetworkSimplex<'a, Flow, V, N, E>
where
    Flow: Number,
{
    pub fn new(st: &'a mut SpanningTreeStructure<Flow, V, N, E>) -> Self {
        Self {
            st,
            root: usize::MAX,
        }
    }

    pub fn solve<Pivot: PivotRule<Flow, V, N, E>>(&mut self, pivot: &mut Pivot) -> Status {
        if self.st.is_unbalanced() {
            return Status::Unbalanced;
        }

        let (root, artificial_edges) = self.st.construct_extend_network_feasible_solution();
        self.root = root;

        self.run(pivot, artificial_edges.clone());

        let status = if self.st.satisfy_constraints() {
            Status::Optimal
        } else {
            Status::Infeasible
        };
        self.st.pop_edges(artificial_edges.len());
        self.st.pop_node();

        status
    }

    pub fn solve_max_flow_with_min_cost<Pivot: PivotRule<Flow, V, N, E>>(
        &mut self,
        source: usize,
        sink: usize,
        pivot: &mut Pivot,
    ) -> (Status, Flow) {
        if source >= self.st.num_nodes || sink >= self.st.num_nodes {
            return (Status::BadInput, Flow::zero());
        }

        let status = self.solve(pivot);
        if status == Status::Infeasible {
            return (status, Flow::zero());
        }

        let infinite_flow = self
            .st
            .edges
            .iter()
            .filter(|e| e.from() == source)
            .map(|e| e.upper)
            .fold(Flow::zero(), |acc, upper| acc + upper);

        self.st.add_supply(source, infinite_flow);
        self.st.add_supply(sink, -infinite_flow);
        self.solve(pivot);
        self.st.excesses[source] = Flow::zero();
        self.st.excesses[sink] = Flow::zero();

        let maximum_flow = self
            .st
            .edges
            .iter()
            .filter(|e| e.from() == source)
            .map(|e| e.flow)
            .fold(Flow::zero(), |acc, upper| acc + upper);
        (Status::Optimal, maximum_flow)
    }

    pub(crate) fn run<Pivot: PivotRule<Flow, V, N, E>>(
        &mut self,
        pivot: &mut Pivot,
        artificial_edges: std::ops::Range<usize>,
    ) {
        while let Some(entering_edge_id) =
            pivot.find_entering_edge(self.st, Self::calculate_violation)
        {
            let (leaving_edge_id, apex, delta, t2_now_root, t2_new_root) =
                self.select_leaving_edge(entering_edge_id);
            self.st.update_flow_in_cycle(entering_edge_id, delta, apex);
            self.pivot(leaving_edge_id, entering_edge_id, t2_now_root, t2_new_root);
        }

        // if there is remaining flow on the artificial edge, revert it
        for edge_id in artificial_edges {
            let edge = &mut self.st.edges[edge_id];
            if edge.flow > Flow::zero() {
                self.st.excesses[edge.from()] += edge.flow;
                self.st.excesses[edge.to()] -= edge.flow;
                edge.flow = Flow::zero();
            }
        }
    }

    fn calculate_violation(
        edge: &InternalEdge<Flow, V>,
        st: &SpanningTreeStructure<Flow, V, N, E>,
    ) -> Flow {
        match edge.state {
            EdgeState::Upper => st.reduced_cost(edge),
            _ => -st.reduced_cost(edge),
        }
    }

    // keep strongly feasible solution
    pub fn select_leaving_edge(
        &self,
        entering_edge_id: usize,
    ) -> (usize, usize, Flow, usize, usize) {
        let entering_edge = &self.st.edges[entering_edge_id];

        let (from, to) = match entering_edge.state {
            EdgeState::Tree => panic!("state of entering edge {entering_edge_id} is invalid."),
            EdgeState::Lower => (entering_edge.from(), entering_edge.to()),
            EdgeState::Upper => (entering_edge.to(), entering_edge.from()),
        };

        let (mut leaving_edge_id, mut min_delta, mut t2_now_root, mut t2_new_root) = (
            entering_edge_id,
            entering_edge.upper,
            usize::MAX,
            usize::MAX,
        );

        let apex = {
            let (mut u, mut v) = (from, to);
            while u != v {
                let (u_num, v_num) = (self.st.num_successors.get(u), self.st.num_successors.get(v));

                if u_num <= v_num {
                    let edge_id = self.st.nodes[u].parent_edge_id();
                    let edge = &self.st.edges[edge_id];
                    let delta = if u == edge.to() {
                        edge.residual_capacity()
                    } else {
                        edge.flow
                    };

                    // search first blocking arc
                    if delta < min_delta {
                        (leaving_edge_id, min_delta, t2_now_root, t2_new_root) =
                            (edge_id, delta, u, from);
                    }
                    u = self.st.nodes[u].parent();
                }

                if v_num <= u_num {
                    let edge_id = self.st.nodes[v].parent_edge_id();
                    let edge = &self.st.edges[edge_id];
                    let delta = if v == edge.from() {
                        edge.residual_capacity()
                    } else {
                        edge.flow
                    };

                    // search last blocking arc
                    if delta <= min_delta {
                        (leaving_edge_id, min_delta, t2_now_root, t2_new_root) =
                            (edge_id, delta, v, to);
                    }
                    v = self.st.nodes[v].parent();
                }
            }
            u
        };

        (leaving_edge_id, apex, min_delta, t2_now_root, t2_new_root)
    }

    pub fn pivot(
        &mut self,
        leaving_edge_id: usize,
        entering_edge_id: usize,
        t2_now_root: usize,
        t2_new_root: usize,
    ) {
        if leaving_edge_id == entering_edge_id {
            self.st.edges[entering_edge_id].state = match self.st.edges[entering_edge_id].state {
                EdgeState::Upper => EdgeState::Lower,
                EdgeState::Lower => EdgeState::Upper,
                _ => panic!("state of entering edge {entering_edge_id} is invalid."),
            };
            return;
        }

        // drop leaving edge and detach tree
        self.st.detach_tree(t2_now_root, leaving_edge_id);

        // if the size of subtree t2 is larger than that of subtree t1, swap t1 and t2.
        let (t1_new_root, t2_new_root, new_attach_node) =
            if self.st.num_successors.get(t2_now_root) * 2 >= self.st.num_nodes {
                (
                    t2_now_root,
                    self.st.edges[entering_edge_id].opposite_side(t2_new_root),
                    t2_new_root,
                )
            } else {
                (
                    self.root,
                    t2_new_root,
                    self.st.edges[entering_edge_id].opposite_side(t2_new_root),
                )
            };

        // enter entering edge and attach tree
        self.st.reroot_tree(t2_new_root, entering_edge_id);
        self.st
            .attach_tree(new_attach_node, t2_new_root, entering_edge_id);
        self.root = t1_new_root;
        assert_eq!(self.st.nodes[self.root].parent(), usize::MAX);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algorithms::networkflow::minimum_cost_flow::pivot::BlockSearchPivotRule;
    use crate::numeric::number::Flow;

    #[test]
    fn test_simple_flow() {
        let mut st = SpanningTreeStructure::<Flow>::default();
        st.add_nodes(2);

        st.add_supply(0, 1);
        st.add_supply(1, -1);
        st.add_directed_edge(0, 1, 0, 2, 3).unwrap();

        let mut pivot = BlockSearchPivotRule::<Flow, 30>::with_edges(st.num_edges());
        let mut solver = PrimalNetworkSimplex::new(&mut st);

        assert_eq!(solver.solve(&mut pivot), Status::Optimal);
        assert_eq!(st.get_edge(0).unwrap().flow, 1);
        assert_eq!(st.minimum_cost(), 3);
    }
}
