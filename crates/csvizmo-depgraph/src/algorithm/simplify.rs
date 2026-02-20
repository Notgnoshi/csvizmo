use std::collections::HashSet;

use petgraph::algo::toposort;
use petgraph::algo::tred::{dag_to_toposorted_adjacency_list, dag_transitive_reduction_closure};
use petgraph::visit::{IntoNeighbors, NodeCount};

use crate::{DepGraph, FlatGraphView};

/// Remove redundant edges via transitive reduction.
///
/// If A->B->C and A->C exist, the direct A->C edge is redundant and is removed.
/// Only works on DAGs. Returns an error if the graph contains cycles.
pub fn simplify(graph: &DepGraph) -> eyre::Result<DepGraph> {
    let view = FlatGraphView::new(graph);

    let sorted = toposort(&view.pg, None).map_err(|_| {
        eyre::eyre!(
            "graph contains cycles; transitive reduction requires a DAG. \
             Use `depfilter cycles` to identify them."
        )
    })?;

    // revmap maps original node index -> topo position.
    // sorted[topo_position] maps back to the original NodeIndex.
    let (adj, _revmap) = dag_to_toposorted_adjacency_list::<_, u32>(&view.pg, &sorted);
    let (reduction, _closure) = dag_transitive_reduction_closure(&adj);

    // Build set of edges to keep: (from_id, to_id) pairs present in the reduction.
    let mut keep_edges = HashSet::new();
    for from_topo in 0..reduction.node_count() {
        let from_id = view.idx_to_id[sorted[from_topo].index()];
        for to_topo in reduction.neighbors(from_topo as u32) {
            let to_id = view.idx_to_id[sorted[to_topo as usize].index()];
            keep_edges.insert((from_id, to_id));
        }
    }

    Ok(filter_edges(graph, &keep_edges))
}

fn filter_edges<'a>(graph: &DepGraph, keep: &HashSet<(&'a str, &'a str)>) -> DepGraph {
    DepGraph {
        id: graph.id.clone(),
        attrs: graph.attrs.clone(),
        nodes: graph.nodes.clone(),
        edges: graph
            .edges
            .iter()
            .filter(|e| keep.contains(&(e.from.as_str(), e.to.as_str())))
            .cloned()
            .collect(),
        subgraphs: graph
            .subgraphs
            .iter()
            .map(|sg| filter_edges(sg, keep))
            .collect(),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Edge, NodeInfo};

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

    fn edge_pairs(graph: &DepGraph) -> Vec<(&str, &str)> {
        let mut pairs: Vec<_> = graph
            .edges
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str()))
            .collect();
        pairs.sort();
        pairs
    }

    #[test]
    fn removes_redundant_edge() {
        // a -> b -> c, a -> c: the direct a->c is redundant
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c"), ("a", "c")],
        );
        let result = simplify(&g).unwrap();
        assert_eq!(edge_pairs(&result), vec![("a", "b"), ("b", "c")]);
    }

    #[test]
    fn keeps_all_edges_when_none_redundant() {
        // a -> b -> c: no redundant edges
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c")],
        );
        let result = simplify(&g).unwrap();
        assert_eq!(edge_pairs(&result), vec![("a", "b"), ("b", "c")]);
    }

    #[test]
    fn diamond_reduces() {
        // a -> b -> d, a -> c -> d, a -> d: a->d is redundant
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C"), ("d", "D")],
            &[("a", "b"), ("a", "c"), ("b", "d"), ("c", "d"), ("a", "d")],
        );
        let result = simplify(&g).unwrap();
        assert_eq!(
            edge_pairs(&result),
            vec![("a", "b"), ("a", "c"), ("b", "d"), ("c", "d")]
        );
    }

    #[test]
    fn errors_on_cycle() {
        let g = make_graph(&[("a", "A"), ("b", "B")], &[("a", "b"), ("b", "a")]);
        let msg = simplify(&g).err().expect("expected error").to_string();
        assert!(msg.contains("cycles"), "error message: {msg}");
    }

    #[test]
    fn empty_graph() {
        let g = DepGraph::default();
        let result = simplify(&g).unwrap();
        assert!(result.edges.is_empty());
    }

    #[test]
    fn reverse_insertion_order() {
        // Nodes inserted in reverse topological order to exercise the topo mapping.
        // c -> b -> a with c -> a redundant.
        let g = make_graph(
            &[("c", "C"), ("b", "B"), ("a", "A")],
            &[("c", "b"), ("b", "a"), ("c", "a")],
        );
        let result = simplify(&g).unwrap();
        assert_eq!(edge_pairs(&result), vec![("b", "a"), ("c", "b")]);
    }
}
