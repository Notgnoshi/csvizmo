use std::collections::{HashMap, HashSet};

use clap::Parser;
use indexmap::IndexMap;
use petgraph::algo::tarjan_scc;
use petgraph::graph::NodeIndex;

use crate::{DepGraph, Edge, FlatGraphView, NodeInfo};

/// Detect cycles (strongly connected components with 2+ nodes) in the graph.
#[derive(Clone, Debug, Default, Parser)]
pub struct CyclesArgs {}

/// Find all cycles in the dependency graph and output them as subgraphs.
///
/// Uses Tarjan's SCC algorithm to find strongly connected components. SCCs with 2+ nodes
/// are reported as cycles, each in its own subgraph named `cycle_0`, `cycle_1`, etc.
/// Edges between different cycle SCCs appear at the top level. Self-loops (SCCs with 1
/// node) are ignored. If no cycles exist, returns an empty graph.
pub fn cycles(graph: &DepGraph, _args: &CyclesArgs) -> eyre::Result<DepGraph> {
    let view = FlatGraphView::new(graph);
    let sccs = tarjan_scc(&view.pg);

    // Filter to SCCs with 2+ nodes (ignore self-loops).
    let cycle_sccs: Vec<HashSet<NodeIndex>> = sccs
        .into_iter()
        .filter(|scc| scc.len() >= 2)
        .map(|scc| scc.into_iter().collect())
        .collect();

    if cycle_sccs.is_empty() {
        return Ok(DepGraph::default());
    }

    // Map each cycle node to its SCC index.
    let mut node_to_scc: HashMap<NodeIndex, usize> = HashMap::new();
    for (i, scc) in cycle_sccs.iter().enumerate() {
        for &node in scc {
            node_to_scc.insert(node, i);
        }
    }

    let all_nodes = graph.all_nodes();
    let all_edges = graph.all_edges();

    // Build a subgraph per SCC, preserving original node/edge order.
    let mut subgraphs = Vec::new();
    for (i, scc) in cycle_sccs.iter().enumerate() {
        let nodes: IndexMap<String, NodeInfo> = all_nodes
            .iter()
            .filter(|(id, _)| {
                view.id_to_idx
                    .get(id.as_str())
                    .is_some_and(|idx| scc.contains(idx))
            })
            .map(|(id, info)| (id.clone(), info.clone()))
            .collect();

        let edges: Vec<Edge> = all_edges
            .iter()
            .filter(|e| {
                let from = view.id_to_idx.get(e.from.as_str());
                let to = view.id_to_idx.get(e.to.as_str());
                match (from, to) {
                    (Some(f), Some(t)) => scc.contains(f) && scc.contains(t),
                    _ => false,
                }
            })
            .cloned()
            .collect();

        subgraphs.push(DepGraph {
            id: Some(format!("cycle_{i}")),
            nodes,
            edges,
            ..Default::default()
        });
    }

    // Cross-cycle edges: both endpoints in cycle nodes but in different SCCs.
    let cross_edges: Vec<Edge> = all_edges
        .iter()
        .filter(|e| {
            let from = view.id_to_idx.get(e.from.as_str());
            let to = view.id_to_idx.get(e.to.as_str());
            match (from, to) {
                (Some(f), Some(t)) => match (node_to_scc.get(f), node_to_scc.get(t)) {
                    (Some(sf), Some(st)) => sf != st,
                    _ => false,
                },
                _ => false,
            }
        })
        .cloned()
        .collect();

    Ok(DepGraph {
        edges: cross_edges,
        subgraphs,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_graph(nodes: &[(&str, &str)], edges: &[(&str, &str)]) -> DepGraph {
        DepGraph {
            nodes: nodes
                .iter()
                .map(|(id, label)| (id.to_string(), NodeInfo::new(*label)))
                .collect(),
            edges: edges
                .iter()
                .map(|(from, to)| Edge {
                    from: from.to_string(),
                    to: to.to_string(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }

    fn sorted_node_ids(graph: &DepGraph) -> Vec<&str> {
        let mut ids: Vec<&str> = graph.nodes.keys().map(|s| s.as_str()).collect();
        ids.sort();
        ids
    }

    fn sorted_edge_pairs(graph: &DepGraph) -> Vec<(&str, &str)> {
        let mut pairs: Vec<(&str, &str)> = graph
            .edges
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str()))
            .collect();
        pairs.sort();
        pairs
    }

    #[test]
    fn no_cycles_returns_empty() {
        // a -> b -> c: DAG, no cycles
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c")],
            &[("a", "b"), ("b", "c")],
        );
        let result = cycles(&g, &CyclesArgs::default()).unwrap();
        assert!(result.nodes.is_empty());
        assert!(result.edges.is_empty());
        assert!(result.subgraphs.is_empty());
    }

    #[test]
    fn simple_cycle() {
        // a -> b -> c -> a
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c")],
            &[("a", "b"), ("b", "c"), ("c", "a")],
        );
        let result = cycles(&g, &CyclesArgs::default()).unwrap();
        assert!(result.nodes.is_empty(), "no top-level nodes");
        assert!(result.edges.is_empty(), "no cross-cycle edges");
        assert_eq!(result.subgraphs.len(), 1);

        let sg = &result.subgraphs[0];
        assert_eq!(sg.id.as_deref(), Some("cycle_0"));
        assert_eq!(sorted_node_ids(sg), vec!["a", "b", "c"]);
        assert_eq!(
            sorted_edge_pairs(sg),
            vec![("a", "b"), ("b", "c"), ("c", "a")]
        );
    }

    #[test]
    fn self_loop_ignored() {
        // a -> a: self-loop is an SCC of size 1, should be ignored
        let g = make_graph(&[("a", "a")], &[("a", "a")]);
        let result = cycles(&g, &CyclesArgs::default()).unwrap();
        assert!(result.nodes.is_empty());
        assert!(result.edges.is_empty());
        assert!(result.subgraphs.is_empty());
    }

    #[test]
    fn multiple_disjoint_cycles() {
        // cycle1: a -> b -> a
        // cycle2: c -> d -> c
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")],
            &[("a", "b"), ("b", "a"), ("c", "d"), ("d", "c")],
        );
        let result = cycles(&g, &CyclesArgs::default()).unwrap();
        assert_eq!(result.subgraphs.len(), 2);

        // Collect all cycle nodes across subgraphs.
        let mut all_cycle_nodes: Vec<&str> = result
            .subgraphs
            .iter()
            .flat_map(|sg| sg.nodes.keys().map(|s| s.as_str()))
            .collect();
        all_cycle_nodes.sort();
        assert_eq!(all_cycle_nodes, vec!["a", "b", "c", "d"]);

        // Each subgraph has 2 nodes.
        for sg in &result.subgraphs {
            assert_eq!(sg.nodes.len(), 2);
            assert_eq!(sg.edges.len(), 2);
        }
    }

    #[test]
    fn mixed_graph_excludes_acyclic_nodes() {
        // x -> a -> b -> a, b -> y: only a and b are in a cycle
        let g = make_graph(
            &[("x", "x"), ("a", "a"), ("b", "b"), ("y", "y")],
            &[("x", "a"), ("a", "b"), ("b", "a"), ("b", "y")],
        );
        let result = cycles(&g, &CyclesArgs::default()).unwrap();
        assert_eq!(result.subgraphs.len(), 1);

        let sg = &result.subgraphs[0];
        assert_eq!(sorted_node_ids(sg), vec!["a", "b"]);
        assert_eq!(sorted_edge_pairs(sg), vec![("a", "b"), ("b", "a")]);
        // x and y are not in any cycle
        assert!(result.nodes.is_empty());
    }

    #[test]
    fn cross_cycle_edges() {
        // cycle1: a -> b -> a
        // cycle2: c -> d -> c
        // cross edge: b -> c
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")],
            &[("a", "b"), ("b", "a"), ("c", "d"), ("d", "c"), ("b", "c")],
        );
        let result = cycles(&g, &CyclesArgs::default()).unwrap();
        assert_eq!(result.subgraphs.len(), 2);
        // The cross-cycle edge b -> c should be at the top level.
        assert_eq!(sorted_edge_pairs(&result), vec![("b", "c")]);
    }

    #[test]
    fn empty_graph() {
        let g = DepGraph::default();
        let result = cycles(&g, &CyclesArgs::default()).unwrap();
        assert!(result.nodes.is_empty());
        assert!(result.edges.is_empty());
        assert!(result.subgraphs.is_empty());
    }
}
