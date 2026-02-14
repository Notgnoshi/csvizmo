use clap::Parser;

use super::{MatchKey, build_globset};
use crate::DepGraph;

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
    let _globset = build_globset(&args.pattern)?;
    // TODO: implement filter logic
    Ok(graph.clone())
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
    #[ignore]
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
    #[ignore]
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
    #[ignore]
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
    #[ignore]
    fn no_match_returns_unchanged() {
        let g = make_graph(&[("a", "a"), ("b", "b")], &[("a", "b")]);
        let args = FilterArgs::default().pattern("nonexistent");
        let result = filter(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "b"]);
        assert_eq!(edge_pairs(&result), vec![("a", "b")]);
    }

    // -- traversal --

    #[test]
    #[ignore]
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
    #[ignore]
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

    // -- preserve connectivity --

    #[test]
    #[ignore]
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
    #[ignore]
    fn preserve_connectivity_no_self_loops() {
        // a -> b -> a: remove b, should not create a -> a
        let g = make_graph(&[("a", "a"), ("b", "b")], &[("a", "b"), ("b", "a")]);
        let args = FilterArgs::default().pattern("b").preserve_connectivity();
        let result = filter(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a"]);
        assert!(edge_pairs(&result).is_empty());
    }

    #[test]
    #[ignore]
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
