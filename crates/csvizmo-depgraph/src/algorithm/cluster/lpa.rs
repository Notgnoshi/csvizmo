use std::collections::HashMap;

use rand::SeedableRng;
use rand::prelude::SliceRandom;
use rand::rngs::StdRng;

use super::{Adjacency, clusters_to_depgraph};
use crate::{DepGraph, FlatGraphView};

/// Run Label Propagation Algorithm on the dependency graph.
///
/// Each node starts in its own cluster. Each iteration, nodes adopt the most common
/// cluster label among their neighbors (ties broken by smallest label). Stops when
/// no labels change or `max_iter` is reached.
///
/// If `seed` is provided, the node processing order is shuffled each iteration.
/// Otherwise, nodes are processed in graph order (deterministic).
pub fn lpa(graph: &DepGraph, directed: bool, max_iter: usize, seed: Option<u64>) -> DepGraph {
    let view = FlatGraphView::new(graph);
    let n = view.idx_to_id.len();

    if n == 0 {
        return DepGraph::default();
    }

    let adj = Adjacency::new(&view, directed);

    // Each node starts with its own label (index).
    let mut labels: Vec<usize> = (0..n).collect();

    let mut rng = seed.map(StdRng::seed_from_u64);
    let mut order: Vec<usize> = (0..n).collect();

    for _ in 0..max_iter {
        if let Some(rng) = rng.as_mut() {
            order.shuffle(rng);
        }

        let mut changed = false;
        for &i in &order {
            let neighbors = &adj.neighbors[i];
            if neighbors.is_empty() {
                continue;
            }

            // Count neighbor labels.
            let mut counts: HashMap<usize, usize> = HashMap::new();
            for &neighbor in neighbors {
                *counts.entry(labels[neighbor]).or_default() += 1;
            }

            // Find most common label; ties broken by smallest label.
            let mut best_label = labels[i];
            let mut best_count = 0;
            for (&label, &count) in &counts {
                if count > best_count || (count == best_count && label < best_label) {
                    best_label = label;
                    best_count = count;
                }
            }

            if best_label != labels[i] {
                labels[i] = best_label;
                changed = true;
            }
        }

        if !changed {
            break;
        }
    }

    // Convert label assignments to partition.
    let mut cluster_map: HashMap<usize, Vec<&str>> = HashMap::new();
    for (i, &label) in labels.iter().enumerate() {
        cluster_map
            .entry(label)
            .or_default()
            .push(view.idx_to_id[i]);
    }

    // Sort clusters by their smallest label for deterministic output.
    let mut clusters: Vec<(usize, Vec<&str>)> = cluster_map.into_iter().collect();
    clusters.sort_by_key(|(label, _)| *label);
    let partition: Vec<Vec<&str>> = clusters.into_iter().map(|(_, ids)| ids).collect();

    clusters_to_depgraph(graph, &partition)
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

    #[test]
    fn two_disconnected_components() {
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")],
            &[("a", "b"), ("c", "d")],
        );
        let result = lpa(&g, false, 100, None);
        assert_eq!(result.subgraphs.len(), 2);
        // No cross-cluster edges
        assert!(result.edges.is_empty());
    }

    #[test]
    fn single_clique() {
        // Fully connected: a-b, b-c, a-c -- should all be in one cluster
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c")],
            &[
                ("a", "b"),
                ("b", "c"),
                ("a", "c"),
                ("b", "a"),
                ("c", "b"),
                ("c", "a"),
            ],
        );
        let result = lpa(&g, false, 100, None);
        assert_eq!(result.subgraphs.len(), 1);
        assert_eq!(result.subgraphs[0].nodes.len(), 3);
    }

    #[test]
    fn empty_graph() {
        let g = DepGraph::default();
        let result = lpa(&g, false, 100, None);
        assert!(result.subgraphs.is_empty());
        assert!(result.edges.is_empty());
    }

    #[test]
    fn with_seed() {
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")],
            &[("a", "b"), ("c", "d")],
        );
        let result = lpa(&g, false, 100, Some(42));
        assert_eq!(result.subgraphs.len(), 2);
    }
}
