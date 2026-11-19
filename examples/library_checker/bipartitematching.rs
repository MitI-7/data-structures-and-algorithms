// https://judge.yosupo.jp/problem/bipartitematching
#![cfg_attr(all(target_os = "linux", not(test)), no_main)]
#![cfg_attr(test, allow(dead_code))]

use data_structures_and_algorithms::algorithms::graph::matching::bipartite::{
    bipartite_graph::BipartiteGraph, hopcroft_karp::HopcroftKarp,
};
use data_structures_and_algorithms::io::fastio::{Scanner, append_num, write_stdout};

fn run() {
    let mut input = Scanner::from_stdin();
    let num_left_nodes = input.usize();
    let num_right_nodes = input.usize();
    let num_edges = input.usize();

    let mut graph = BipartiteGraph::default();
    graph.add_left_nodes(num_left_nodes);
    graph.add_right_nodes(num_right_nodes);

    for _ in 0..num_edges {
        let a = input.usize();
        let b = input.usize();
        graph.add_edge(a, b).unwrap();
    }

    let matching = HopcroftKarp::default().solve(&graph);

    let mut output = Vec::<u8>::with_capacity(64 + matching.len() * 44);
    append_num(&mut output, matching.len() as i64);

    for &edge_id in matching.iter() {
        let edge = graph.get_edge(edge_id).unwrap();
        append_num(&mut output, edge.left as i64);
        output.pop();
        output.push(b' ');
        append_num(&mut output, edge.right as i64);
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
