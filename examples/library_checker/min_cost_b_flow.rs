// https://judge.yosupo.jp/problem/min_cost_b_flow
#![cfg_attr(all(target_os = "linux", not(test)), no_main)]
#![cfg_attr(test, allow(dead_code))]

use data_structures_and_algorithms::algorithms::networkflow::minimum_cost_flow::{
    network_simplex::PrimalNetworkSimplex,
    pivot::BlockSearchPivotRule,
    spanning_tree::{SpanningTreeStructure, Status},
};
use data_structures_and_algorithms::io::fastio::{Scanner, append_num, write_stdout};
use data_structures_and_algorithms::numeric::number::{Flow, Number};

fn run() {
    let mut input = Scanner::from_stdin();
    let num_nodes = input.usize();
    let num_edges = input.usize();
    assert!(num_nodes < u8::MAX as usize && num_edges <= u16::MAX as usize - num_nodes);

    let mut st = SpanningTreeStructure::<Flow, u8, u8, u16>::with_capacity(num_nodes, num_edges);
    st.add_nodes(num_nodes);
    for u in 0..num_nodes {
        let x: Flow = input.number();
        st.add_supply(u, x);
    }
    for _ in 0..num_edges {
        let u = input.usize();
        let v = input.usize();
        let lower: Flow = input.number();
        let upper: Flow = input.number();
        let cost: Flow = input.number();
        st.add_directed_edge(u, v, lower, upper, cost).unwrap();
    }

    let mut pivot = BlockSearchPivotRule::<Flow, 88>::with_edges(st.num_edges());
    let mut ns = PrimalNetworkSimplex::new(&mut st);
    let status = ns.solve(&mut pivot);

    let mut output = Vec::<u8>::with_capacity(64 + (num_edges + num_nodes) * 22);
    if status == Status::Optimal {
        let mut cost: i128 = 0;
        for i in 0..num_edges {
            let e = st.get_edge(i).unwrap();
            cost += e.flow.to_i128() * e.cost.to_i128();
        }
        append_num(&mut output, cost);

        for &potential in st.potentials() {
            append_num(&mut output, potential);
        }

        for i in 0..num_edges {
            append_num(&mut output, st.get_edge(i).unwrap().flow);
        }
    } else {
        output.extend_from_slice(b"infeasible\n");
    }
    write_stdout(&output).unwrap();
}

// Platform entry points. This single-threaded judge program does not use Rust's
// argument handling or runtime signal setup. A panic at the C boundary aborts.
#[cfg(all(target_os = "linux", not(test)))]
#[unsafe(no_mangle)]
pub extern "C" fn main(_: std::ffi::c_int, _: *const *const std::ffi::c_char) -> std::ffi::c_int {
    run();
    0
}

#[cfg(not(target_os = "linux"))]
fn main() {
    run();
}
