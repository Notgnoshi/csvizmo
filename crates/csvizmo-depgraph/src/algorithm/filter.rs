use std::collections::{HashSet, VecDeque};

use clap::Parser;
use petgraph::Direction;
use petgraph::graph::NodeIndex;

use super::{MatchKey, build_globset};
use crate::{DepGraph, Edge, FlatGraphView};

#[derive(Clone, Debug, Default, Parser)]
pub struct FilterArgs {
    /// Glob pattern to remove nodes (can be repeated)
    #[clap(short, long)]
    pub pattern: Vec<String>,

    /// Combine multiple patterns with AND instead of OR
    #[clap(long)]
    pub and: bool,

    /// Match patterns against 'id' or 'label'
    #[clap(long, default_value_t = MatchKey::default())]
    pub key: MatchKey,

    /// Also remove all dependencies of matched nodes (cascade)
    #[clap(long)]
    pub deps: bool,

    /// Also remove all ancestors of matched nodes (cascade)
    #[clap(long)]
    pub ancestors: bool,

    /// Preserve graph connectivity when removing nodes
    /// (creates direct edges, no self-loops or parallel edges)
    #[clap(long)]
    pub preserve_connectivity: bool,
}

impl FilterArgs {
    pub fn pattern(mut self, p: impl Into<String>) -> Self {
        self.pattern.push(p.into());
        self
    }

    pub fn and(mut self) -> Self {
        self.and = true;
        self
    }

    pub fn key(mut self, k: MatchKey) -> Self {
        self.key = k;
        self
    }

    pub fn deps(mut self) -> Self {
        self.deps = true;
        self
    }

    pub fn ancestors(mut self) -> Self {
        self.ancestors = true;
        self
    }

    pub fn preserve_connectivity(mut self) -> Self {
        self.preserve_connectivity = true;
        self
    }
}

pub fn filter(graph: &DepGraph, args: &FilterArgs) -> eyre::Result<DepGraph> {
    let globset = build_globset(&args.pattern)?;
    let view = FlatGraphView::new(graph);

    // Find nodes that match the patterns (these will be removed).
    let mut matched = HashSet::new();
    for (id, info) in graph.all_nodes() {
        let text = match args.key {
            MatchKey::Id => id,
            MatchKey::Label => info.label.as_str(),
        };

        let is_match = if args.and {
            globset.matches(text).len() == args.pattern.len()
        } else {
            globset.is_match(text)
        };

        if is_match && let Some(&idx) = view.id_to_idx.get(id) {
            matched.insert(idx);
        }
    }

    // Cascade removal via BFS if --deps or --ancestors is set.
    if args.deps && args.ancestors {
        let seeds = matched.clone();
        matched = view.bfs(seeds.clone(), Direction::Outgoing, None);
        matched.extend(view.bfs(seeds, Direction::Incoming, None));
    } else if args.ancestors {
        matched = view.bfs(matched, Direction::Incoming, None);
    } else if args.deps {
        matched = view.bfs(matched, Direction::Outgoing, None);
    }

    // Keep set = all nodes minus matched nodes.
    let all_nodes: HashSet<_> = view.id_to_idx.values().copied().collect();
    let keep: HashSet<_> = all_nodes.difference(&matched).copied().collect();

    let mut result = view.filter(&keep);

    // Bypass removed nodes: connect their surviving predecessors to surviving successors.
    // BFS through chains of removed nodes so that A->B->C->D with B,C removed produces A->D.
    if args.preserve_connectivity {
        let mut existing: HashSet<(String, String)> = result
            .edges
            .iter()
            .map(|e| (e.from.clone(), e.to.clone()))
            .collect();

        for &idx in &matched {
            let preds = surviving_neighbors(&view.pg, idx, Direction::Incoming, &keep);
            let succs = surviving_neighbors(&view.pg, idx, Direction::Outgoing, &keep);

            for &pred in &preds {
                let from = view.idx_to_id[pred.index()];
                for &succ in &succs {
                    let to = view.idx_to_id[succ.index()];
                    if from != to && existing.insert((from.to_string(), to.to_string())) {
                        result.edges.push(Edge {
                            from: from.to_string(),
                            to: to.to_string(),
                            ..Default::default()
                        });
                    }
                }
            }
        }
    }

    Ok(result)
}

/// BFS from `start` in `direction`, traversing through removed nodes (those not in `keep`),
/// returning surviving nodes found at the boundary.
fn surviving_neighbors(
    pg: &petgraph::Graph<(), ()>,
    start: NodeIndex,
    direction: Direction,
    keep: &HashSet<NodeIndex>,
) -> Vec<NodeIndex> {
    let mut result = Vec::new();
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    visited.insert(start);
    queue.push_back(start);

    while let Some(node) = queue.pop_front() {
        for neighbor in pg.neighbors_directed(node, direction) {
            if !visited.insert(neighbor) {
                continue;
            }
            if keep.contains(&neighbor) {
                result.push(neighbor);
            } else {
                queue.push_back(neighbor);
            }
        }
    }

    result
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

    fn node_ids(graph: &DepGraph) -> Vec<&str> {
        graph.nodes.keys().map(|s| s.as_str()).collect()
    }

    fn edge_pairs(graph: &DepGraph) -> Vec<(&str, &str)> {
        graph
            .edges
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str()))
            .collect()
    }

    // -- pattern matching --

    #[test]
    fn single_pattern() {
        // myapp -> libfoo -> libbar, myapp -> libbar
        let g = make_graph(
            &[
                ("libfoo", "libfoo"),
                ("libbar", "libbar"),
                ("myapp", "myapp"),
            ],
            &[
                ("myapp", "libfoo"),
                ("myapp", "libbar"),
                ("libfoo", "libbar"),
            ],
        );
        let args = FilterArgs::default().pattern("libfoo");
        let result = filter(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["libbar", "myapp"]);
        assert_eq!(edge_pairs(&result), vec![("myapp", "libbar")]);
    }

    #[test]
    fn multiple_patterns_or() {
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c")],
            &[("a", "b"), ("b", "c")],
        );
        let args = FilterArgs::default().pattern("a").pattern("b");
        let result = filter(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["c"]);
        assert!(edge_pairs(&result).is_empty());
    }

    #[test]
    fn multiple_patterns_and() {
        let g = make_graph(
            &[
                ("libfoo-alpha", "libfoo-alpha"),
                ("libfoo-beta", "libfoo-beta"),
                ("libbar-alpha", "libbar-alpha"),
            ],
            &[],
        );
        let args = FilterArgs::default()
            .pattern("libfoo*")
            .pattern("*alpha")
            .and();
        let result = filter(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["libfoo-beta", "libbar-alpha"]);
    }

    #[test]
    fn no_match_returns_unchanged() {
        let g = make_graph(&[("a", "a"), ("b", "b")], &[("a", "b")]);
        let args = FilterArgs::default().pattern("nonexistent");
        let result = filter(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "b"]);
        assert_eq!(edge_pairs(&result), vec![("a", "b")]);
    }

    // -- traversal --

    #[test]
    fn with_deps_cascade() {
        // a -> b -> c, a -> c
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c")],
            &[("a", "b"), ("b", "c"), ("a", "c")],
        );
        let args = FilterArgs::default().pattern("a").deps();
        let result = filter(&g, &args).unwrap();
        assert!(node_ids(&result).is_empty());
    }

    #[test]
    fn with_ancestors_cascade() {
        // a -> b -> c
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c")],
            &[("a", "b"), ("b", "c")],
        );
        let args = FilterArgs::default().pattern("c").ancestors();
        let result = filter(&g, &args).unwrap();
        assert!(node_ids(&result).is_empty());
    }

    #[test]
    fn with_deps_and_ancestors_cascade() {
        // a -> b -> c -> d: filter b with both deps and ancestors
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")],
            &[("a", "b"), ("b", "c"), ("c", "d")],
        );
        let args = FilterArgs::default().pattern("b").deps().ancestors();
        let result = filter(&g, &args).unwrap();
        // b + ancestors (a) + deps (c, d) = all removed
        assert!(node_ids(&result).is_empty());
    }

    #[test]
    fn with_deps_and_ancestors_cascade_partial() {
        // a -> b -> c, d -> c: filter b with both directions
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")],
            &[("a", "b"), ("b", "c"), ("d", "c")],
        );
        let args = FilterArgs::default().pattern("b").deps().ancestors();
        let result = filter(&g, &args).unwrap();
        // b removed, ancestors (a) removed, deps (c) removed, but d survives
        assert_eq!(node_ids(&result), vec!["d"]);
        assert!(edge_pairs(&result).is_empty());
    }

    // -- preserve connectivity --

    #[test]
    fn preserve_connectivity_bypass() {
        // a -> b -> c: remove b, get a -> c
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c")],
            &[("a", "b"), ("b", "c")],
        );
        let args = FilterArgs::default().pattern("b").preserve_connectivity();
        let result = filter(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "c"]);
        assert_eq!(edge_pairs(&result), vec![("a", "c")]);
    }

    #[test]
    fn preserve_connectivity_chain() {
        // a -> b -> c -> d: remove b and c, get a -> d
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")],
            &[("a", "b"), ("b", "c"), ("c", "d")],
        );
        let args = FilterArgs::default()
            .pattern("b")
            .pattern("c")
            .preserve_connectivity();
        let result = filter(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "d"]);
        assert_eq!(edge_pairs(&result), vec![("a", "d")]);
    }

    #[test]
    fn preserve_connectivity_diamond_through_removed() {
        // a -> b -> d, a -> c -> d: remove b and c, get single a -> d
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")],
            &[("a", "b"), ("a", "c"), ("b", "d"), ("c", "d")],
        );
        let args = FilterArgs::default()
            .pattern("b")
            .pattern("c")
            .preserve_connectivity();
        let result = filter(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "d"]);
        assert_eq!(edge_pairs(&result), vec![("a", "d")]);
    }

    #[test]
    fn preserve_connectivity_no_self_loops() {
        // a -> b -> a: remove b, should not create a -> a
        let g = make_graph(&[("a", "a"), ("b", "b")], &[("a", "b"), ("b", "a")]);
        let args = FilterArgs::default().pattern("b").preserve_connectivity();
        let result = filter(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a"]);
        assert!(edge_pairs(&result).is_empty());
    }

    #[test]
    fn preserve_connectivity_no_parallel_edges() {
        // a -> b -> c, a -> c: remove b, should not duplicate a -> c
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c")],
            &[("a", "b"), ("b", "c"), ("a", "c")],
        );
        let args = FilterArgs::default().pattern("b").preserve_connectivity();
        let result = filter(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "c"]);
        assert_eq!(edge_pairs(&result), vec![("a", "c")]);
    }
}
