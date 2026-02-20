use std::collections::HashSet;

use clap::Parser;
use petgraph::Direction;
use petgraph::algo::toposort;
use petgraph::graph::NodeIndex;

use super::OutputFields;
use crate::algorithm::{MatchKey, build_globset};
use crate::{DepGraph, FlatGraphView};

#[derive(Debug, Default, Clone, Copy, clap::ValueEnum)]
pub enum NodeSelection {
    #[default]
    All,
    Roots,
    Leaves,
}

impl std::fmt::Display for NodeSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use clap::ValueEnum;

        f.write_str(self.to_possible_value().unwrap().get_name())
    }
}

#[derive(Debug, Default, Clone, Copy, clap::ValueEnum)]
pub enum NodeSort {
    #[default]
    None,
    Topo,
    InDegree,
    OutDegree,
    Ancestors,
    Descendants,
}

impl std::fmt::Display for NodeSort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use clap::ValueEnum;

        f.write_str(self.to_possible_value().unwrap().get_name())
    }
}

#[derive(Clone, Debug, Default, Parser)]
pub struct NodesArgs {
    /// Which nodes to start from
    #[clap(long, default_value_t = NodeSelection::All)]
    pub select: NodeSelection,

    /// Include pattern (repeatable, OR by default)
    #[clap(short = 'g', long)]
    pub include: Vec<String>,

    /// Exclude pattern (repeatable, OR)
    #[clap(short = 'x', long)]
    pub exclude: Vec<String>,

    /// Combine include patterns with AND instead of OR
    #[clap(long)]
    pub and: bool,

    /// What patterns match against
    #[clap(long, default_value_t = MatchKey::default())]
    pub key: MatchKey,

    /// Sort order
    #[clap(long, default_value_t = NodeSort::None)]
    pub sort: NodeSort,

    /// Reverse the sort order
    #[clap(short = 'r', long)]
    pub reverse: bool,

    /// Show only first N results (applied after sort)
    #[clap(short = 'n', long)]
    pub limit: Option<usize>,

    /// What to print
    #[clap(long, default_value_t = OutputFields::Label)]
    pub format: OutputFields,

    /// Include dependencies of selected nodes
    #[clap(long, alias = "children")]
    pub deps: bool,

    /// Include reverse dependencies (ancestors) of selected nodes
    #[clap(long, alias = "parents")]
    pub rdeps: bool,

    /// Max traversal depth (implies --deps)
    #[clap(long)]
    pub depth: Option<usize>,
}

/// Returns (id, label, count) tuples for matching nodes.
///
/// The count is `Some(n)` when the sort order is numeric (in-degree,
/// out-degree, ancestors, descendants) and `None` otherwise.
pub fn nodes(
    graph: &DepGraph,
    args: &NodesArgs,
) -> eyre::Result<Vec<(String, String, Option<usize>)>> {
    let view = FlatGraphView::new(graph);
    let all_nodes = graph.all_nodes();

    // 1. Select initial set
    let mut selected: Vec<NodeIndex> = match args.select {
        NodeSelection::All => view.pg.node_indices().collect(),
        NodeSelection::Roots => view.roots().collect(),
        NodeSelection::Leaves => view
            .pg
            .node_indices()
            .filter(|&idx| {
                view.pg
                    .neighbors_directed(idx, Direction::Outgoing)
                    .next()
                    .is_none()
            })
            .collect(),
    };

    // 2. Apply include patterns
    if !args.include.is_empty() {
        let include_set = build_globset(&args.include)?;
        selected.retain(|&idx| {
            let id = view.idx_to_id[idx.index()];
            let info = &all_nodes[id];
            let text = match args.key {
                MatchKey::Id => id,
                MatchKey::Label => info.label.as_str(),
            };
            if args.and {
                include_set.matches(text).len() == args.include.len()
            } else {
                include_set.is_match(text)
            }
        });
    }

    // 3. Apply exclude patterns
    if !args.exclude.is_empty() {
        let exclude_set = build_globset(&args.exclude)?;
        selected.retain(|&idx| {
            let id = view.idx_to_id[idx.index()];
            let info = &all_nodes[id];
            let text = match args.key {
                MatchKey::Id => id,
                MatchKey::Label => info.label.as_str(),
            };
            !exclude_set.is_match(text)
        });
    }

    // 4. Expand with --deps / --rdeps / --depth
    let deps = args.deps || args.depth.is_some();
    if deps || args.rdeps {
        let seeds: HashSet<NodeIndex> = selected.iter().copied().collect();
        let mut expanded = HashSet::new();

        if deps {
            expanded.extend(view.bfs(seeds.iter().copied(), Direction::Outgoing, args.depth));
        }
        if args.rdeps {
            expanded.extend(view.bfs(seeds.iter().copied(), Direction::Incoming, args.depth));
        }

        // Keep seeds in all cases
        expanded.extend(seeds);
        selected = expanded.into_iter().collect();
    }

    // 5. Sort (returns sorted indices with optional counts)
    let sorted = sort_nodes(&selected, &args.sort, args.reverse, &view);

    // 6. Apply limit
    let sorted = if let Some(limit) = args.limit {
        &sorted[..limit.min(sorted.len())]
    } else {
        &sorted
    };

    // 7. Map to output
    let result = sorted
        .iter()
        .map(|&(idx, count)| {
            let id = view.idx_to_id[idx.index()];
            let info = &all_nodes[id];
            (id.to_string(), info.label.clone(), count)
        })
        .collect();

    Ok(result)
}

fn sort_nodes(
    nodes: &[NodeIndex],
    sort: &NodeSort,
    reverse: bool,
    view: &FlatGraphView,
) -> Vec<(NodeIndex, Option<usize>)> {
    let mut result: Vec<(NodeIndex, Option<usize>)> = match sort {
        NodeSort::None => nodes.iter().map(|&idx| (idx, None)).collect(),
        NodeSort::Topo => {
            if let Ok(sorted) = toposort(&view.pg, Option::None) {
                let node_set: HashSet<NodeIndex> = nodes.iter().copied().collect();
                sorted
                    .into_iter()
                    .filter(|idx| node_set.contains(idx))
                    .map(|idx| (idx, None))
                    .collect()
            } else {
                // Graph has cycles, fall back to insertion order
                nodes.iter().map(|&idx| (idx, None)).collect()
            }
        }
        NodeSort::InDegree => {
            let mut v: Vec<(NodeIndex, Option<usize>)> = nodes
                .iter()
                .map(|&idx| {
                    let count = view.pg.neighbors_directed(idx, Direction::Incoming).count();
                    (idx, Some(count))
                })
                .collect();
            v.sort_by(|a, b| {
                b.1.cmp(&a.1)
                    .then_with(|| view.idx_to_id[a.0.index()].cmp(view.idx_to_id[b.0.index()]))
            });
            v
        }
        NodeSort::OutDegree => {
            let mut v: Vec<(NodeIndex, Option<usize>)> = nodes
                .iter()
                .map(|&idx| {
                    let count = view.pg.neighbors_directed(idx, Direction::Outgoing).count();
                    (idx, Some(count))
                })
                .collect();
            v.sort_by(|a, b| {
                b.1.cmp(&a.1)
                    .then_with(|| view.idx_to_id[a.0.index()].cmp(view.idx_to_id[b.0.index()]))
            });
            v
        }
        NodeSort::Ancestors => {
            let mut v: Vec<(NodeIndex, Option<usize>)> = nodes
                .iter()
                .map(|&idx| {
                    let count = view.bfs([idx], Direction::Incoming, None).len() - 1;
                    (idx, Some(count))
                })
                .collect();
            v.sort_by(|a, b| {
                b.1.cmp(&a.1)
                    .then_with(|| view.idx_to_id[a.0.index()].cmp(view.idx_to_id[b.0.index()]))
            });
            v
        }
        NodeSort::Descendants => {
            let mut v: Vec<(NodeIndex, Option<usize>)> = nodes
                .iter()
                .map(|&idx| {
                    let count = view.bfs([idx], Direction::Outgoing, None).len() - 1;
                    (idx, Some(count))
                })
                .collect();
            v.sort_by(|a, b| {
                b.1.cmp(&a.1)
                    .then_with(|| view.idx_to_id[a.0.index()].cmp(view.idx_to_id[b.0.index()]))
            });
            v
        }
    };
    if reverse {
        result.reverse();
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

    fn labels(result: &[(String, String, Option<usize>)]) -> Vec<&str> {
        result.iter().map(|(_, l, _)| l.as_str()).collect()
    }

    fn ids(result: &[(String, String, Option<usize>)]) -> Vec<&str> {
        result.iter().map(|(id, _, _)| id.as_str()).collect()
    }

    fn counts(result: &[(String, String, Option<usize>)]) -> Vec<Option<usize>> {
        result.iter().map(|(_, _, c)| *c).collect()
    }

    #[test]
    fn all_nodes_default() {
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c")],
        );
        let result = nodes(&g, &NodesArgs::default()).unwrap();
        assert_eq!(ids(&result), vec!["a", "b", "c"]);
    }

    #[test]
    fn select_roots() {
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c")],
        );
        let args = NodesArgs {
            select: NodeSelection::Roots,
            ..Default::default()
        };
        let result = nodes(&g, &args).unwrap();
        assert_eq!(ids(&result), vec!["a"]);
    }

    #[test]
    fn select_leaves() {
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c")],
        );
        let args = NodesArgs {
            select: NodeSelection::Leaves,
            ..Default::default()
        };
        let result = nodes(&g, &args).unwrap();
        assert_eq!(ids(&result), vec!["c"]);
    }

    #[test]
    fn include_pattern() {
        let g = make_graph(&[("a", "alpha"), ("b", "beta"), ("c", "gamma")], &[]);
        let args = NodesArgs {
            include: vec!["al*".to_string()],
            ..Default::default()
        };
        let result = nodes(&g, &args).unwrap();
        assert_eq!(labels(&result), vec!["alpha"]);
    }

    #[test]
    fn exclude_pattern() {
        let g = make_graph(&[("a", "alpha"), ("b", "beta"), ("c", "gamma")], &[]);
        let args = NodesArgs {
            exclude: vec!["b*".to_string()],
            ..Default::default()
        };
        let result = nodes(&g, &args).unwrap();
        assert_eq!(labels(&result), vec!["alpha", "gamma"]);
    }

    #[test]
    fn include_and_mode() {
        let g = make_graph(
            &[("a", "foo-alpha"), ("b", "foo-beta"), ("c", "bar-alpha")],
            &[],
        );
        let args = NodesArgs {
            include: vec!["foo*".to_string(), "*alpha".to_string()],
            and: true,
            ..Default::default()
        };
        let result = nodes(&g, &args).unwrap();
        assert_eq!(labels(&result), vec!["foo-alpha"]);
    }

    #[test]
    fn sort_topo() {
        // a -> c, b -> c (a and b are roots, c is leaf)
        let g = make_graph(
            &[("c", "C"), ("a", "A"), ("b", "B")],
            &[("a", "c"), ("b", "c")],
        );
        let args = NodesArgs {
            sort: NodeSort::Topo,
            ..Default::default()
        };
        let result = nodes(&g, &args).unwrap();
        // c must come after a and b
        let c_pos = ids(&result).iter().position(|&x| x == "c").unwrap();
        let a_pos = ids(&result).iter().position(|&x| x == "a").unwrap();
        let b_pos = ids(&result).iter().position(|&x| x == "b").unwrap();
        assert!(a_pos < c_pos);
        assert!(b_pos < c_pos);
    }

    #[test]
    fn sort_out_degree() {
        // a -> b, a -> c, b -> c
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("a", "c"), ("b", "c")],
        );
        let args = NodesArgs {
            sort: NodeSort::OutDegree,
            ..Default::default()
        };
        let result = nodes(&g, &args).unwrap();
        // a has out-degree 2, b has 1, c has 0 (descending)
        assert_eq!(ids(&result), vec!["a", "b", "c"]);
        assert_eq!(counts(&result), vec![Some(2), Some(1), Some(0)]);
    }

    #[test]
    fn sort_in_degree() {
        // a -> c, b -> c
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "c"), ("b", "c")],
        );
        let args = NodesArgs {
            sort: NodeSort::InDegree,
            ..Default::default()
        };
        let result = nodes(&g, &args).unwrap();
        // c has in-degree 2, a and b have 0
        assert_eq!(ids(&result)[0], "c");
        assert_eq!(counts(&result), vec![Some(2), Some(0), Some(0)]);
    }

    #[test]
    fn limit() {
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c")],
        );
        let args = NodesArgs {
            sort: NodeSort::Topo,
            limit: Some(2),
            ..Default::default()
        };
        let result = nodes(&g, &args).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn deps_expansion() {
        // a -> b -> c: select roots, then expand deps
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c")],
        );
        let args = NodesArgs {
            select: NodeSelection::Roots,
            deps: true,
            sort: NodeSort::Topo,
            ..Default::default()
        };
        let result = nodes(&g, &args).unwrap();
        assert_eq!(ids(&result), vec!["a", "b", "c"]);
    }

    #[test]
    fn rdeps_expansion() {
        // a -> b -> c: select leaves, then expand rdeps
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c")],
        );
        let args = NodesArgs {
            select: NodeSelection::Leaves,
            rdeps: true,
            sort: NodeSort::Topo,
            ..Default::default()
        };
        let result = nodes(&g, &args).unwrap();
        assert_eq!(ids(&result), vec!["a", "b", "c"]);
    }

    #[test]
    fn depth_limited_expansion() {
        // a -> b -> c -> d: select root a, deps with depth 1
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C"), ("d", "D")],
            &[("a", "b"), ("b", "c"), ("c", "d")],
        );
        let args = NodesArgs {
            select: NodeSelection::Roots,
            depth: Some(1),
            sort: NodeSort::Topo,
            ..Default::default()
        };
        let result = nodes(&g, &args).unwrap();
        assert_eq!(ids(&result), vec!["a", "b"]);
    }

    #[test]
    fn reverse_sort() {
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c")],
        );
        let args = NodesArgs {
            sort: NodeSort::Topo,
            reverse: true,
            ..Default::default()
        };
        let result = nodes(&g, &args).unwrap();
        assert_eq!(ids(&result), vec!["c", "b", "a"]);
    }

    #[test]
    fn match_by_id() {
        let g = make_graph(&[("node1", "Alpha"), ("node2", "Beta")], &[]);
        let args = NodesArgs {
            include: vec!["node1".to_string()],
            key: MatchKey::Id,
            ..Default::default()
        };
        let result = nodes(&g, &args).unwrap();
        assert_eq!(ids(&result), vec!["node1"]);
    }
}
