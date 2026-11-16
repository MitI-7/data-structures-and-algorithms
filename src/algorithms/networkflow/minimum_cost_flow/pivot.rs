use super::spanning_tree::{EdgeState, Index, InternalEdge, PricingData, SpanningTreeStructure};
use crate::numeric::number::Number;
use std::marker::PhantomData;

pub trait PivotRule<Flow, V: Index = u32, N: Index = usize, E: Index = usize> {
    fn new(num_edges: usize) -> Self;
    fn find_entering_edge<
        'a,
        F: Fn(&InternalEdge<Flow, V>, &'a SpanningTreeStructure<Flow, V, N, E>) -> Flow,
    >(
        &mut self,
        st: &'a SpanningTreeStructure<Flow, V, N, E>,
        calculate_violation: F,
    ) -> Option<usize>;
}

pub struct BlockSearchPivotRule<Flow: Number, const WIDTH: usize> {
    cursor: usize,
    _marker: PhantomData<fn() -> Flow>,
}

impl<Flow: Number, const WIDTH: usize> BlockSearchPivotRule<Flow, WIDTH> {
    pub fn new(num_edges: usize) -> Self {
        Self::with_edges(num_edges)
    }

    pub fn with_edges(_: usize) -> Self {
        assert!(WIDTH > 0, "pricing block width must be positive");
        Self {
            cursor: 0,
            _marker: PhantomData,
        }
    }

    #[inline(always)]
    fn scalar_scan<V: Index, N: Index, E: Index>(
        &self,
        st: &SpanningTreeStructure<Flow, V, N, E>,
        data: &PricingData<Flow, V>,
        start: usize,
        len: usize,
        best_value: &mut Flow,
        best_id: &mut Option<usize>,
    ) {
        let end = start + len;
        for i in start..end {
            unsafe {
                let edge = st.edges.get_unchecked(i);
                let u = data.froms.get_unchecked(i).to_usize();
                let v = data.tos.get_unchecked(i).to_usize();
                let reduced = *data.costs.get_unchecked(i) + *st.potentials.get_unchecked(u)
                    - *st.potentials.get_unchecked(v);
                let violation = if edge.state == EdgeState::Upper {
                    reduced
                } else {
                    -reduced
                };
                if violation > *best_value {
                    *best_value = violation;
                    *best_id = Some(i);
                }
            }
        }
    }
}

impl<Flow: Number, const WIDTH: usize, V: Index, N: Index, E: Index> PivotRule<Flow, V, N, E>
    for BlockSearchPivotRule<Flow, WIDTH>
{
    fn new(edges: usize) -> Self {
        Self::with_edges(edges)
    }

    fn find_entering_edge<
        'a,
        F: Fn(&InternalEdge<Flow, V>, &'a SpanningTreeStructure<Flow, V, N, E>) -> Flow,
    >(
        &mut self,
        st: &'a SpanningTreeStructure<Flow, V, N, E>,
        _: F,
    ) -> Option<usize> {
        let n = st.num_edges;
        if n == 0 {
            return None;
        }
        let data = st.pricing_data();
        if self.cursor >= n {
            self.cursor = 0;
        }
        let mut block_left = WIDTH;
        let mut scanned = 0;
        let mut best_value = Flow::zero();
        let mut best_id = None;
        while scanned < n {
            let chunk = block_left.min(n - scanned).min(n - self.cursor);
            self.scalar_scan(st, data, self.cursor, chunk, &mut best_value, &mut best_id);
            self.cursor += chunk;
            if self.cursor == n {
                self.cursor = 0;
            }
            block_left -= chunk;
            scanned += chunk;
            if block_left == 0 {
                if best_id.is_some() {
                    return best_id;
                }
                block_left = WIDTH;
            }
        }
        best_id
    }
}
