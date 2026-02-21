pub mod graphrs_bridge;
pub mod lpa;

use std::collections::HashMap;

use indexmap::IndexMap;
use petgraph::Direction;

use crate::{DepGraph, Edge, FlatGraphView, NodeInfo};

/// Precomputed neighbor lists from a flattened dependency graph.
///
/// In undirected mode, neighbors include both incoming and outgoing edges (deduplicated).
/// In directed mode, only outgoing neighbors are included.
pub struct Adjacency {
    /// For each node index, the set of neighbor node indices.
    pub neighbors: Vec<Vec<usize>>,
}

impl Adjacency {
    pub fn new(view: &FlatGraphView, directed: bool) -> Self {
        let n = view.idx_to_id.len();
        let mut neighbors = vec![Vec::new(); n];

        for idx in view.pg.node_indices() {
            let i = idx.index();
            let mut seen = Vec::new();

            for neighbor in view.pg.neighbors_directed(idx, Direction::Outgoing) {
                seen.push(neighbor.index());
            }

            if !directed {
                for neighbor in view.pg.neighbors_directed(idx, Direction::Incoming) {
                    if !seen.contains(&neighbor.index()) {
                        seen.push(neighbor.index());
                    }
                }
            }

            neighbors[i] = seen;
        }

        Adjacency { neighbors }
    }
}

/// Convert a partition (list of clusters, each a list of node IDs) into a DepGraph with
/// one subgraph per cluster. Intra-cluster edges go in the subgraph; cross-cluster edges
/// go at the top level.
pub fn clusters_to_depgraph(graph: &DepGraph, partition: &[Vec<&str>]) -> DepGraph {
    let all_nodes = graph.all_nodes();
    let all_edges = graph.all_edges();

    // Map each node ID to its cluster index.
    let mut node_to_cluster: HashMap<&str, usize> = HashMap::new();
    for (i, cluster) in partition.iter().enumerate() {
        for &id in cluster {
            node_to_cluster.insert(id, i);
        }
    }

    let mut subgraphs = Vec::new();
    for (i, cluster) in partition.iter().enumerate() {
        let cluster_ids: std::collections::HashSet<&str> = cluster.iter().copied().collect();

        let nodes: IndexMap<String, NodeInfo> = all_nodes
            .iter()
            .filter(|(id, _)| cluster_ids.contains(id.as_str()))
            .map(|(id, info)| (id.clone(), info.clone()))
            .collect();

        let edges: Vec<Edge> = all_edges
            .iter()
            .filter(|e| {
                cluster_ids.contains(e.from.as_str()) && cluster_ids.contains(e.to.as_str())
            })
            .cloned()
            .collect();

        subgraphs.push(DepGraph {
            id: Some(format!("cluster_{i}")),
            nodes,
            edges,
            ..Default::default()
        });
    }

    // Cross-cluster edges: both endpoints assigned to clusters but in different ones.
    let cross_edges: Vec<Edge> = all_edges
        .iter()
        .filter(|e| {
            match (
                node_to_cluster.get(e.from.as_str()),
                node_to_cluster.get(e.to.as_str()),
            ) {
                (Some(cf), Some(ct)) => cf != ct,
                _ => false,
            }
        })
        .cloned()
        .collect();

    DepGraph {
        edges: cross_edges,
        subgraphs,
        ..Default::default()
    }
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

    #[test]
    fn adjacency_undirected() {
        let g = make_graph(&[("a", "a"), ("b", "b"), ("c", "c")], &[("a", "b")]);
        let view = FlatGraphView::new(&g);
        let adj = Adjacency::new(&view, false);
        // a's neighbors: b (outgoing) -> [b_idx]
        let a_idx = view.id_to_idx["a"].index();
        let b_idx = view.id_to_idx["b"].index();
        assert!(adj.neighbors[a_idx].contains(&b_idx));
        // b's neighbors: a (incoming, undirected) -> [a_idx]
        assert!(adj.neighbors[b_idx].contains(&a_idx));
    }

    #[test]
    fn adjacency_directed() {
        let g = make_graph(&[("a", "a"), ("b", "b")], &[("a", "b")]);
        let view = FlatGraphView::new(&g);
        let adj = Adjacency::new(&view, true);
        let a_idx = view.id_to_idx["a"].index();
        let b_idx = view.id_to_idx["b"].index();
        // a -> b: a has neighbor b
        assert!(adj.neighbors[a_idx].contains(&b_idx));
        // b has no outgoing edges in directed mode
        assert!(adj.neighbors[b_idx].is_empty());
    }

    #[test]
    fn clusters_to_depgraph_basic() {
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")],
            &[("a", "b"), ("c", "d"), ("b", "c")],
        );
        let partition = vec![vec!["a", "b"], vec!["c", "d"]];
        let result = clusters_to_depgraph(&g, &partition);

        assert_eq!(result.subgraphs.len(), 2);
        assert_eq!(result.subgraphs[0].id.as_deref(), Some("cluster_0"));
        assert_eq!(result.subgraphs[0].nodes.len(), 2);
        assert_eq!(result.subgraphs[0].edges.len(), 1); // a->b
        assert_eq!(result.subgraphs[1].id.as_deref(), Some("cluster_1"));
        assert_eq!(result.subgraphs[1].nodes.len(), 2);
        assert_eq!(result.subgraphs[1].edges.len(), 1); // c->d
        // Cross-cluster edge: b->c
        assert_eq!(result.edges.len(), 1);
        assert_eq!(result.edges[0].from, "b");
        assert_eq!(result.edges[0].to, "c");
    }

    #[test]
    fn clusters_to_depgraph_empty() {
        let g = DepGraph::default();
        let partition: Vec<Vec<&str>> = vec![];
        let result = clusters_to_depgraph(&g, &partition);
        assert!(result.subgraphs.is_empty());
        assert!(result.edges.is_empty());
    }
}
