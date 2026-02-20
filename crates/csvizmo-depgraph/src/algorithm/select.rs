use std::collections::{HashSet, VecDeque};

use clap::Parser;
use petgraph::Direction;
use petgraph::graph::NodeIndex;

use super::{MatchKey, build_globset};
use crate::{DepGraph, Edge, FlatGraphView};

#[derive(Clone, Debug, Default, Parser)]
pub struct SelectArgs {
    /// Glob pattern to include nodes (can be repeated)
    #[clap(short = 'g', long)]
    pub include: Vec<String>,

    /// Glob pattern to exclude nodes (can be repeated, always OR)
    #[clap(short = 'x', long)]
    pub exclude: Vec<String>,

    /// Combine multiple include patterns with AND instead of OR
    #[clap(long)]
    pub and: bool,

    /// Match patterns against 'id' or 'label'
    #[clap(long, default_value_t = MatchKey::default())]
    pub key: MatchKey,

    /// Include all dependencies of selected nodes
    #[clap(long, alias = "children")]
    pub deps: bool,

    /// Include all reverse dependencies of selected nodes
    #[clap(long, alias = "parents", alias = "ancestors")]
    pub rdeps: bool,

    /// Traverse up to N layers (implies --deps if no direction given)
    #[clap(long)]
    pub depth: Option<usize>,

    /// Preserve graph connectivity when excluding nodes
    /// (creates direct edges, no self-loops or parallel edges)
    #[clap(long)]
    pub preserve_connectivity: bool,
}

impl SelectArgs {
    pub fn include(mut self, p: impl Into<String>) -> Self {
        self.include.push(p.into());
        self
    }

    pub fn exclude(mut self, p: impl Into<String>) -> Self {
        self.exclude.push(p.into());
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

    pub fn rdeps(mut self) -> Self {
        self.rdeps = true;
        self
    }

    pub fn depth(mut self, n: usize) -> Self {
        self.depth = Some(n);
        self
    }

    pub fn preserve_connectivity(mut self) -> Self {
        self.preserve_connectivity = true;
        self
    }
}

pub fn select(graph: &DepGraph, args: &SelectArgs) -> eyre::Result<DepGraph> {
    let view = FlatGraphView::new(graph);

    // No filters at all -> pass through the entire graph unchanged.
    let no_traversal = !args.deps && !args.rdeps && args.depth.is_none();
    if args.include.is_empty() && args.exclude.is_empty() && no_traversal {
        return Ok(graph.clone());
    }

    let include_globset = build_globset(&args.include)?;
    let exclude_globset = build_globset(&args.exclude)?;

    // Build the initial keep set from --include patterns (or all nodes / roots).
    let has_traversal = args.deps || args.rdeps || args.depth.is_some();
    let mut keep: HashSet<_> = if args.include.is_empty() {
        if has_traversal {
            view.roots().collect()
        } else {
            // Only --exclude given, no traversal: start with all nodes.
            view.id_to_idx.values().copied().collect()
        }
    } else {
        let mut matched = HashSet::new();
        for (id, info) in graph.all_nodes() {
            let text = match args.key {
                MatchKey::Id => id.as_str(),
                MatchKey::Label => info.label.as_str(),
            };

            let is_match = if args.and {
                include_globset.matches(text).len() == args.include.len()
            } else {
                include_globset.is_match(text)
            };

            if is_match && let Some(&idx) = view.id_to_idx.get(id.as_str()) {
                matched.insert(idx);
            }
        }
        matched
    };

    // Expand keep set via --deps/--rdeps/--depth (only applies to include).
    // --depth without an explicit direction implies --deps
    let deps = args.deps || args.depth.is_some();
    if deps && args.rdeps {
        let seeds = keep.clone();
        keep = view.bfs(seeds.clone(), Direction::Outgoing, args.depth);
        keep.extend(view.bfs(seeds, Direction::Incoming, args.depth));
    } else if args.rdeps {
        keep = view.bfs(keep, Direction::Incoming, args.depth);
    } else if deps {
        keep = view.bfs(keep, Direction::Outgoing, args.depth);
    }

    // Remove nodes matching --exclude patterns from keep set.
    let excluded: HashSet<NodeIndex> = if !args.exclude.is_empty() {
        let mut matched = HashSet::new();
        for (id, info) in graph.all_nodes() {
            let text = match args.key {
                MatchKey::Id => id.as_str(),
                MatchKey::Label => info.label.as_str(),
            };

            if exclude_globset.is_match(text)
                && let Some(&idx) = view.id_to_idx.get(id.as_str())
            {
                matched.insert(idx);
            }
        }
        let excluded = keep.intersection(&matched).copied().collect::<HashSet<_>>();
        for &idx in &excluded {
            keep.remove(&idx);
        }
        excluded
    } else {
        HashSet::new()
    };

    let mut result = view.filter(&keep);

    // Bypass excluded nodes: connect their surviving predecessors to surviving successors.
    // BFS through chains of excluded nodes so that A->B->C->D with B,C excluded produces A->D.
    if args.preserve_connectivity && !excluded.is_empty() {
        let mut existing: HashSet<(String, String)> = result
            .all_edges()
            .iter()
            .map(|e| (e.from.clone(), e.to.clone()))
            .collect();

        let mut bypass_edges = Vec::new();
        for &idx in &excluded {
            let preds = surviving_neighbors(&view.pg, idx, Direction::Incoming, &keep);
            let succs = surviving_neighbors(&view.pg, idx, Direction::Outgoing, &keep);

            for &pred in &preds {
                let from = view.idx_to_id[pred.index()];
                for &succ in &succs {
                    let to = view.idx_to_id[succ.index()];
                    if from != to && existing.insert((from.to_string(), to.to_string())) {
                        bypass_edges.push((from.to_string(), to.to_string()));
                    }
                }
            }
        }

        for (from, to) in bypass_edges {
            insert_edge(&mut result, &from, &to);
        }

        result.clear_caches();
    }

    Ok(result)
}

/// Insert a bypass edge into the deepest subgraph that contains both endpoints.
/// Falls back to the root graph if the endpoints are in different subgraphs.
fn insert_edge(graph: &mut DepGraph, from: &str, to: &str) {
    for sg in &mut graph.subgraphs {
        let has_from = sg.all_nodes().contains_key(from);
        let has_to = sg.all_nodes().contains_key(to);
        if has_from && has_to {
            return insert_edge(sg, from, to);
        }
    }
    graph.edges.push(Edge {
        from: from.to_string(),
        to: to.to_string(),
        ..Default::default()
    });
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

    // -- include pattern matching --

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
            vec![],
        );
        let args = SelectArgs::default().include("lib*");
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["libfoo", "libbar"]);
        assert_eq!(edge_pairs(&result), vec![("libfoo", "libbar")]);
    }

    #[test]
    fn match_by_id() {
        let g = make_graph(&[("1", "libfoo"), ("2", "libbar")], &[("1", "2")], vec![]);
        let args = SelectArgs::default().include("1").key(MatchKey::Id);
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["1"]);
    }

    #[test]
    fn match_by_label() {
        let g = make_graph(&[("1", "libfoo"), ("2", "libbar")], &[("1", "2")], vec![]);
        let args = SelectArgs::default().include("libbar");
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["2"]);
    }

    #[test]
    fn multiple_patterns_or() {
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c")],
            &[("a", "b"), ("b", "c")],
            vec![],
        );
        let args = SelectArgs::default().include("a").include("c");
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
            vec![],
        );
        let args = SelectArgs::default()
            .include("libfoo*")
            .include("*alpha")
            .and();
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["libfoo-alpha"]);
    }

    #[test]
    fn no_match_produces_empty_graph() {
        let g = make_graph(&[("a", "a"), ("b", "b")], &[("a", "b")], vec![]);
        let args = SelectArgs::default().include("nonexistent");
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
            vec![],
        );
        let args = SelectArgs::default().include("a").deps();
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "b", "c"]);
    }

    #[test]
    fn with_rdeps() {
        // a -> b -> c
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c")],
            &[("a", "b"), ("b", "c")],
            vec![],
        );
        let args = SelectArgs::default().include("c").rdeps();
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "b", "c"]);
    }

    #[test]
    fn with_depth_limit() {
        // a -> b -> c -> d
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")],
            &[("a", "b"), ("b", "c"), ("c", "d")],
            vec![],
        );
        let args = SelectArgs::default().include("a").deps().depth(1);
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "b"]);
        assert_eq!(edge_pairs(&result), vec![("a", "b")]);
    }

    #[test]
    fn with_deps_and_rdeps() {
        // a -> b -> c -> d: select b with both deps and rdeps
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")],
            &[("a", "b"), ("b", "c"), ("c", "d")],
            vec![],
        );
        let args = SelectArgs::default().include("b").deps().rdeps();
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "b", "c", "d"]);
        assert_eq!(
            edge_pairs(&result),
            vec![("a", "b"), ("b", "c"), ("c", "d")]
        );
    }

    #[test]
    fn with_deps_and_rdeps_depth_limited() {
        // a -> b -> c -> d -> e: select c with both directions, depth 1
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d"), ("e", "e")],
            &[("a", "b"), ("b", "c"), ("c", "d"), ("d", "e")],
            vec![],
        );
        let args = SelectArgs::default().include("c").deps().rdeps().depth(1);
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["b", "c", "d"]);
        assert_eq!(edge_pairs(&result), vec![("b", "c"), ("c", "d")]);
    }

    #[test]
    fn no_args_returns_full_graph() {
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")],
            &[("a", "b"), ("b", "c"), ("c", "d")],
            vec![],
        );
        let args = SelectArgs::default();
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "b", "c", "d"]);
        assert_eq!(
            edge_pairs(&result),
            vec![("a", "b"), ("b", "c"), ("c", "d")]
        );
    }

    #[test]
    fn depth_without_pattern_seeds_from_roots() {
        // a -> b -> c -> d
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")],
            &[("a", "b"), ("b", "c"), ("c", "d")],
            vec![],
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
            vec![],
        );
        let args = SelectArgs::default().depth(0);
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "b"]);
    }

    // -- subgraphs --

    #[test]
    fn preserves_subgraph_structure() {
        // root: a -> b, subgraph: { c }, edge b -> c at root
        // select a with deps keeps all nodes and preserves subgraph
        let g = make_graph(
            &[("a", "a"), ("b", "b")],
            &[("a", "b"), ("b", "c")],
            vec![make_graph(&[("c", "c")], &[], vec![])],
        );
        let args = SelectArgs::default().include("a").deps();
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "b"]);
        assert_eq!(result.subgraphs.len(), 1);
        assert_eq!(node_ids(&result.subgraphs[0]), vec!["c"]);
    }

    // -- exclude pattern matching --

    #[test]
    fn exclude_single_pattern() {
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
            vec![],
        );
        let args = SelectArgs::default().exclude("libfoo");
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["libbar", "myapp"]);
        assert_eq!(edge_pairs(&result), vec![("myapp", "libbar")]);
    }

    #[test]
    fn exclude_multiple_patterns_or() {
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c")],
            &[("a", "b"), ("b", "c")],
            vec![],
        );
        let args = SelectArgs::default().exclude("a").exclude("b");
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["c"]);
        assert!(edge_pairs(&result).is_empty());
    }

    #[test]
    fn exclude_no_match_returns_unchanged() {
        let g = make_graph(&[("a", "a"), ("b", "b")], &[("a", "b")], vec![]);
        let args = SelectArgs::default().exclude("nonexistent");
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "b"]);
        assert_eq!(edge_pairs(&result), vec![("a", "b")]);
    }

    // -- exclude with preserve connectivity --

    #[test]
    fn preserve_connectivity_bypass() {
        // a -> b -> c: exclude b, get a -> c
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c")],
            &[("a", "b"), ("b", "c")],
            vec![],
        );
        let args = SelectArgs::default().exclude("b").preserve_connectivity();
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "c"]);
        assert_eq!(edge_pairs(&result), vec![("a", "c")]);
    }

    #[test]
    fn preserve_connectivity_chain() {
        // a -> b -> c -> d: exclude b and c, get a -> d
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")],
            &[("a", "b"), ("b", "c"), ("c", "d")],
            vec![],
        );
        let args = SelectArgs::default()
            .exclude("b")
            .exclude("c")
            .preserve_connectivity();
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "d"]);
        assert_eq!(edge_pairs(&result), vec![("a", "d")]);
    }

    #[test]
    fn preserve_connectivity_diamond_through_excluded() {
        // a -> b -> d, a -> c -> d: exclude b and c, get single a -> d
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")],
            &[("a", "b"), ("a", "c"), ("b", "d"), ("c", "d")],
            vec![],
        );
        let args = SelectArgs::default()
            .exclude("b")
            .exclude("c")
            .preserve_connectivity();
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "d"]);
        assert_eq!(edge_pairs(&result), vec![("a", "d")]);
    }

    #[test]
    fn preserve_connectivity_no_self_loops() {
        // a -> b -> a: exclude b, should not create a -> a
        let g = make_graph(&[("a", "a"), ("b", "b")], &[("a", "b"), ("b", "a")], vec![]);
        let args = SelectArgs::default().exclude("b").preserve_connectivity();
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a"]);
        assert!(edge_pairs(&result).is_empty());
    }

    #[test]
    fn preserve_connectivity_no_parallel_edges() {
        // a -> b -> c, a -> c: exclude b, should not duplicate a -> c
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c")],
            &[("a", "b"), ("b", "c"), ("a", "c")],
            vec![],
        );
        let args = SelectArgs::default().exclude("b").preserve_connectivity();
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "c"]);
        assert_eq!(edge_pairs(&result), vec![("a", "c")]);
    }

    // -- exclude with subgraphs --

    #[test]
    fn exclude_preserves_subgraph_structure() {
        // root: a, subgraph: { b, c, b->c }, edge a->b at root
        // exclude a keeps b, c in subgraph with their edge
        let g = make_graph(
            &[("a", "a")],
            &[("a", "b")],
            vec![make_graph(&[("b", "b"), ("c", "c")], &[("b", "c")], vec![])],
        );
        let args = SelectArgs::default().exclude("a");
        let result = select(&g, &args).unwrap();
        assert!(result.nodes.is_empty());
        assert!(result.edges.is_empty());
        assert_eq!(result.subgraphs.len(), 1);
        assert_eq!(node_ids(&result.subgraphs[0]), vec!["b", "c"]);
        assert_eq!(edge_pairs(&result.subgraphs[0]), vec![("b", "c")]);
    }

    // -- preserve connectivity with subgraphs --

    #[test]
    fn preserve_connectivity_bypass_in_subgraph() {
        // subgraph { a -> b -> c }: exclude b, bypass a -> c should be in the subgraph
        let g = make_graph(
            &[],
            &[],
            vec![make_graph(
                &[("a", "a"), ("b", "b"), ("c", "c")],
                &[("a", "b"), ("b", "c")],
                vec![],
            )],
        );
        let args = SelectArgs::default().exclude("b").preserve_connectivity();
        let result = select(&g, &args).unwrap();
        // bypass a -> c should be inside the subgraph, not at root
        assert!(result.edges.is_empty());
        assert_eq!(result.subgraphs.len(), 1);
        let sg = &result.subgraphs[0];
        assert_eq!(node_ids(sg), vec!["a", "c"]);
        assert_eq!(edge_pairs(sg), vec![("a", "c")]);
    }

    #[test]
    fn preserve_connectivity_no_parallel_edges_in_subgraph() {
        // subgraph { a -> b -> c, a -> c }: exclude b, should not duplicate a -> c
        let g = make_graph(
            &[],
            &[],
            vec![make_graph(
                &[("a", "a"), ("b", "b"), ("c", "c")],
                &[("a", "b"), ("b", "c"), ("a", "c")],
                vec![],
            )],
        );
        let args = SelectArgs::default().exclude("b").preserve_connectivity();
        let result = select(&g, &args).unwrap();
        assert!(result.edges.is_empty());
        assert_eq!(result.subgraphs.len(), 1);
        let sg = &result.subgraphs[0];
        assert_eq!(node_ids(sg), vec!["a", "c"]);
        assert_eq!(edge_pairs(sg), vec![("a", "c")]);
    }

    #[test]
    fn preserve_connectivity_cross_subgraph_bypass_at_root() {
        // subgraph1 { a }, subgraph2 { c }, root: b, edges a->b, b->c at root
        // exclude b, bypass a->c should be at root
        let g = make_graph(
            &[("b", "b")],
            &[("a", "b"), ("b", "c")],
            vec![
                make_graph(&[("a", "a")], &[], vec![]),
                make_graph(&[("c", "c")], &[], vec![]),
            ],
        );
        let args = SelectArgs::default().exclude("b").preserve_connectivity();
        let result = select(&g, &args).unwrap();
        assert!(result.nodes.is_empty());
        assert_eq!(edge_pairs(&result), vec![("a", "c")]);
        assert_eq!(result.subgraphs.len(), 2);
    }

    // -- combined include + exclude --

    #[test]
    fn include_with_exclude() {
        // a -> b -> c -> d: include a with deps, then exclude c
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")],
            &[("a", "b"), ("b", "c"), ("c", "d")],
            vec![],
        );
        let args = SelectArgs::default().include("a").deps().exclude("c");
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "b", "d"]);
        assert_eq!(edge_pairs(&result), vec![("a", "b")]);
    }

    #[test]
    fn include_with_exclude_preserve_connectivity() {
        // a -> b -> c -> d: include a with deps, exclude c with preserve connectivity
        let g = make_graph(
            &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")],
            &[("a", "b"), ("b", "c"), ("c", "d")],
            vec![],
        );
        let args = SelectArgs::default()
            .include("a")
            .deps()
            .exclude("c")
            .preserve_connectivity();
        let result = select(&g, &args).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "b", "d"]);
        assert_eq!(edge_pairs(&result), vec![("a", "b"), ("b", "d")]);
    }
}
