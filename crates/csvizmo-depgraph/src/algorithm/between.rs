use std::collections::HashSet;

use clap::Parser;
use petgraph::Direction;
use petgraph::graph::NodeIndex;

use super::{MatchKey, build_globset};
use crate::{DepGraph, FlatGraphView};

#[derive(Clone, Debug, Default, Parser)]
pub struct BetweenArgs {
    /// Glob pattern selecting query endpoints (can be repeated, OR logic)
    #[clap(short = 'g', long)]
    pub include: Vec<String>,

    /// Glob pattern to exclude nodes from result (can be repeated, OR logic)
    #[clap(short = 'x', long)]
    pub exclude: Vec<String>,

    /// Match patterns against 'id' or 'label'
    #[clap(long, default_value_t = MatchKey::default())]
    pub key: MatchKey,
}

impl BetweenArgs {
    pub fn include(mut self, p: impl Into<String>) -> Self {
        self.include.push(p.into());
        self
    }

    pub fn exclude(mut self, p: impl Into<String>) -> Self {
        self.exclude.push(p.into());
        self
    }

    pub fn key(mut self, k: MatchKey) -> Self {
        self.key = k;
        self
    }
}

/// Extract the subgraph formed by all directed paths between any pair of matched query nodes.
///
/// For matched query nodes q1..qk, computes forward and backward reachability from each,
/// then for each ordered pair (qi, qj) collects nodes on directed paths from qi to qj
/// via `forward(qi) & backward(qj)`. The union of all pairwise results is the keep set.
pub fn between(graph: &DepGraph, args: &BetweenArgs) -> eyre::Result<DepGraph> {
    let globset = build_globset(&args.include)?;
    let view = FlatGraphView::new(graph);

    // Match query nodes by glob pattern (OR logic).
    let matched: Vec<NodeIndex> = graph
        .all_nodes()
        .iter()
        .filter_map(|(id, info)| {
            let text = match args.key {
                MatchKey::Id => id.as_str(),
                MatchKey::Label => info.label.as_str(),
            };
            if globset.is_match(text) {
                view.id_to_idx.get(id.as_str()).copied()
            } else {
                None
            }
        })
        .collect();

    // Need at least 2 matched nodes to have a path between them.
    if matched.len() < 2 {
        return Ok(view.filter(&HashSet::new()));
    }

    // BFS forward and backward from each query node.
    let forwards: Vec<HashSet<NodeIndex>> = matched
        .iter()
        .map(|&q| view.bfs([q], Direction::Outgoing, None))
        .collect();
    let backwards: Vec<HashSet<NodeIndex>> = matched
        .iter()
        .map(|&q| view.bfs([q], Direction::Incoming, None))
        .collect();

    // Pairwise intersect: for each pair (i, j) where i != j, nodes on directed paths
    // from qi to qj are in forward(qi) & backward(qj).
    let mut keep = HashSet::new();
    for (i, fwd) in forwards.iter().enumerate() {
        for (j, bwd) in backwards.iter().enumerate() {
            if i == j {
                continue;
            }
            for &node in fwd {
                if bwd.contains(&node) {
                    keep.insert(node);
                }
            }
        }
    }

    // Remove nodes matching --exclude patterns from keep set.
    if !args.exclude.is_empty() {
        let exclude_globset = build_globset(&args.exclude)?;
        for (id, info) in graph.all_nodes() {
            let text = match args.key {
                MatchKey::Id => id.as_str(),
                MatchKey::Label => info.label.as_str(),
            };
            if exclude_globset.is_match(text)
                && let Some(&idx) = view.id_to_idx.get(id.as_str())
            {
                keep.remove(&idx);
            }
        }
    }

    Ok(view.filter(&keep))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DepGraph, Edge, NodeInfo};

    fn make_graph(
        nodes: &[(&str, &str)],
        edges: &[(&str, &str)],
        subgraphs: Vec<DepGraph>,
    ) -> DepGraph {
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
            subgraphs,
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
    fn direct_path() {
        // a -> b: between a and b yields both
        let g = make_graph(&[("a", "a"), ("b", "b")], &[("a", "b")], vec![]);
        let args = BetweenArgs::default().include("a").include("b");
        let result = between(&g, &args).unwrap();
        assert_eq!(sorted_node_ids(&result), vec!["a", "b"]);
        assert_eq!(sorted_edge_pairs(&result), vec![("a", "b")]);
    }

    #[test]
    fn intermediate_nodes() {
        // a -> b -> c: between a and c includes intermediate b
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c")],
            &[("a", "b"), ("b", "c")],
            vec![],
        );
        let args = BetweenArgs::default().include("a").include("c");
        let result = between(&g, &args).unwrap();
        assert_eq!(sorted_node_ids(&result), vec!["a", "b", "c"]);
        assert_eq!(sorted_edge_pairs(&result), vec![("a", "b"), ("b", "c")]);
    }

    #[test]
    fn no_path_returns_empty() {
        // a -> b, c -> d: no path between a and c
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")],
            &[("a", "b"), ("c", "d")],
            vec![],
        );
        let args = BetweenArgs::default().include("a").include("c");
        let result = between(&g, &args).unwrap();
        assert!(result.nodes.is_empty());
        assert!(result.edges.is_empty());
    }

    #[test]
    fn diamond() {
        // a -> b -> d, a -> c -> d: between a and d includes both paths
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")],
            &[("a", "b"), ("a", "c"), ("b", "d"), ("c", "d")],
            vec![],
        );
        let args = BetweenArgs::default().include("a").include("d");
        let result = between(&g, &args).unwrap();
        assert_eq!(sorted_node_ids(&result), vec!["a", "b", "c", "d"]);
        assert_eq!(
            sorted_edge_pairs(&result),
            vec![("a", "b"), ("a", "c"), ("b", "d"), ("c", "d")]
        );
    }

    #[test]
    fn multiple_query_nodes() {
        // a -> b -> c -> d: between a, b, and d includes everything on paths a->b, a->d, b->d
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")],
            &[("a", "b"), ("b", "c"), ("c", "d")],
            vec![],
        );
        let args = BetweenArgs::default()
            .include("a")
            .include("b")
            .include("d");
        let result = between(&g, &args).unwrap();
        assert_eq!(sorted_node_ids(&result), vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn no_match_returns_empty() {
        let g = make_graph(&[("a", "a"), ("b", "b")], &[("a", "b")], vec![]);
        let args = BetweenArgs::default().include("nonexistent");
        let result = between(&g, &args).unwrap();
        assert!(result.nodes.is_empty());
        assert!(result.edges.is_empty());
    }

    #[test]
    fn single_match_returns_empty() {
        // Only one node matches -- need at least 2 for a path
        let g = make_graph(&[("a", "a"), ("b", "b")], &[("a", "b")], vec![]);
        let args = BetweenArgs::default().include("a");
        let result = between(&g, &args).unwrap();
        assert!(result.nodes.is_empty());
        assert!(result.edges.is_empty());
    }

    #[test]
    fn cycle() {
        // a -> b -> c -> a: between a and c includes all nodes in the cycle
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c")],
            &[("a", "b"), ("b", "c"), ("c", "a")],
            vec![],
        );
        let args = BetweenArgs::default().include("a").include("c");
        let result = between(&g, &args).unwrap();
        assert_eq!(sorted_node_ids(&result), vec!["a", "b", "c"]);
    }

    #[test]
    fn match_by_id() {
        let g = make_graph(&[("1", "libfoo"), ("2", "libbar")], &[("1", "2")], vec![]);
        let args = BetweenArgs::default()
            .include("1")
            .include("2")
            .key(MatchKey::Id);
        let result = between(&g, &args).unwrap();
        assert_eq!(sorted_node_ids(&result), vec!["1", "2"]);
        assert_eq!(sorted_edge_pairs(&result), vec![("1", "2")]);
    }

    #[test]
    fn excludes_unrelated_nodes() {
        // a -> b -> c, d -> e: between a and c should not include d or e
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d"), ("e", "e")],
            &[("a", "b"), ("b", "c"), ("d", "e")],
            vec![],
        );
        let args = BetweenArgs::default().include("a").include("c");
        let result = between(&g, &args).unwrap();
        assert_eq!(sorted_node_ids(&result), vec!["a", "b", "c"]);
        assert_eq!(sorted_edge_pairs(&result), vec![("a", "b"), ("b", "c")]);
    }

    #[test]
    fn glob_matching_multiple_nodes() {
        // a -> b -> c: glob "?" matches a, b, c -- all pairs have paths
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c")],
            &[("a", "b"), ("b", "c")],
            vec![],
        );
        let args = BetweenArgs::default().include("?");
        let result = between(&g, &args).unwrap();
        assert_eq!(sorted_node_ids(&result), vec!["a", "b", "c"]);
        assert_eq!(sorted_edge_pairs(&result), vec![("a", "b"), ("b", "c")]);
    }

    #[test]
    fn exclude_removes_from_result() {
        // a -> b -> c: between a and c, exclude b
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c")],
            &[("a", "b"), ("b", "c")],
            vec![],
        );
        let args = BetweenArgs::default()
            .include("a")
            .include("c")
            .exclude("b");
        let result = between(&g, &args).unwrap();
        assert_eq!(sorted_node_ids(&result), vec!["a", "c"]);
        assert!(sorted_edge_pairs(&result).is_empty());
    }
}
