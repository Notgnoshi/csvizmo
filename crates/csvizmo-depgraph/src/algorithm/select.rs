use std::collections::HashSet;

use clap::Parser;
use petgraph::Direction;

use super::{MatchKey, build_globset};
use crate::{DepGraph, FlatGraphView};

#[derive(Clone, Debug, Default, Parser)]
pub struct SelectArgs {
    /// Glob pattern to select nodes (can be repeated)
    #[clap(short, long)]
    pub pattern: Vec<String>,

    /// Combine multiple patterns with AND instead of OR
    #[clap(long)]
    pub and: bool,

    /// Match patterns against 'id' or 'label'
    #[clap(long, default_value_t = MatchKey::default())]
    pub key: MatchKey,

    /// Include all dependencies of selected nodes
    #[clap(long)]
    pub deps: bool,

    /// Include all ancestors of selected nodes
    #[clap(long)]
    pub ancestors: bool,

    /// Traverse up to N layers (implies --deps if no direction given)
    #[clap(long)]
    pub depth: Option<usize>,
}

impl SelectArgs {
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

    pub fn depth(mut self, n: usize) -> Self {
        self.depth = Some(n);
        self
    }
}

pub fn select(graph: &DepGraph, args: &SelectArgs) -> eyre::Result<DepGraph> {
    let globset = build_globset(&args.pattern)?;
    let view = FlatGraphView::new(graph);

    // If no patterns given, seed from root nodes; otherwise match by pattern.
    let mut keep: HashSet<_> = if args.pattern.is_empty() {
        view.roots().collect()
    } else {
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
        matched
    };

    // --depth without an explicit direction implies --deps
    let direction = if args.ancestors {
        Some(Direction::Incoming)
    } else if args.deps || args.depth.is_some() {
        Some(Direction::Outgoing)
    } else {
        None
    };

    if let Some(dir) = direction {
        keep = view.bfs(keep, dir, args.depth);
    }

    Ok(view.filter(&keep))
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
    fn single_glob_pattern() {
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
        let args = SelectArgs::default().pattern("lib*");
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["libfoo", "libbar"]);
        assert_eq!(edge_pairs(&result), vec![("libfoo", "libbar")]);
    }

    #[test]
    fn match_by_id() {
        let g = make_graph(&[("1", "libfoo"), ("2", "libbar")], &[("1", "2")]);
        let args = SelectArgs::default().pattern("1").key(MatchKey::Id);
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["1"]);
    }

    #[test]
    fn match_by_label() {
        let g = make_graph(&[("1", "libfoo"), ("2", "libbar")], &[("1", "2")]);
        let args = SelectArgs::default().pattern("libbar");
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["2"]);
    }

    #[test]
    fn multiple_patterns_or() {
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c")],
            &[("a", "b"), ("b", "c")],
        );
        let args = SelectArgs::default().pattern("a").pattern("c");
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "c"]);
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
        let args = SelectArgs::default()
            .pattern("libfoo*")
            .pattern("*alpha")
            .and();
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["libfoo-alpha"]);
    }

    #[test]
    fn no_match_produces_empty_graph() {
        let g = make_graph(&[("a", "a"), ("b", "b")], &[("a", "b")]);
        let args = SelectArgs::default().pattern("nonexistent");
        let result = select(&g, &args).unwrap();
        assert!(result.nodes.is_empty());
        assert!(result.edges.is_empty());
    }

    // -- traversal --

    #[test]
    fn with_deps() {
        // a -> b -> c, a -> c
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c")],
            &[("a", "b"), ("b", "c"), ("a", "c")],
        );
        let args = SelectArgs::default().pattern("a").deps();
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "b", "c"]);
    }

    #[test]
    fn with_ancestors() {
        // a -> b -> c
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c")],
            &[("a", "b"), ("b", "c")],
        );
        let args = SelectArgs::default().pattern("c").ancestors();
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "b", "c"]);
    }

    #[test]
    fn with_depth_limit() {
        // a -> b -> c -> d
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")],
            &[("a", "b"), ("b", "c"), ("c", "d")],
        );
        let args = SelectArgs::default().pattern("a").deps().depth(1);
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "b"]);
        assert_eq!(edge_pairs(&result), vec![("a", "b")]);
    }

    #[test]
    fn depth_without_pattern_seeds_from_roots() {
        // a -> b -> c -> d
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")],
            &[("a", "b"), ("b", "c"), ("c", "d")],
        );
        let args = SelectArgs::default().depth(2);
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "b", "c"]);
        assert_eq!(edge_pairs(&result), vec![("a", "b"), ("b", "c")]);
    }

    #[test]
    fn depth_without_pattern_multiple_roots() {
        // a -> c, b -> c
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c")],
            &[("a", "c"), ("b", "c")],
        );
        let args = SelectArgs::default().depth(0);
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "b"]);
    }
}
