use std::collections::HashSet;

use indexmap::IndexMap;

use crate::{DepGraph, NodeInfo};

/// Status of a node during DFS tree traversal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisitStatus {
    /// First visit -- the walker will recurse into this node's children.
    First,
    /// Already fully expanded from a different path. Corresponds to `(*)` marker.
    AlreadyExpanded,
    /// Ancestor on the current DFS path (back-edge). Corresponds to `(cycle)` marker.
    Cycle,
}

/// Context passed to a [`TreeVisitor`] at each visited node.
pub struct VisitContext<'a> {
    /// Node ID.
    // the two DFS emitters only use the node label, but the ID is still available for future visitors
    #[allow(unused)]
    pub node_id: &'a str,
    /// Node metadata (label, attrs).
    pub info: &'a NodeInfo,
    /// Depth in the traversal tree (0 for roots).
    pub depth: usize,
    /// True if this is the last sibling at this depth.
    pub is_last: bool,
    /// Number of children in the adjacency list (regardless of visit status).
    pub child_count: usize,
    /// Visit status.
    pub status: VisitStatus,
}

/// Trait for visiting nodes during DFS tree traversal.
pub trait TreeVisitor {
    fn visit(&mut self, ctx: &VisitContext) -> eyre::Result<()>;
}

/// Walk a [`DepGraph`] as a tree using DFS.
///
/// Finds root nodes (no incoming edges), iterates over them in order, and performs DFS from each.
/// A shared visited set ensures each node's subtree is expanded only once. Already-expanded nodes
/// are reported with [`VisitStatus::AlreadyExpanded`]. Back-edges to ancestors on the current path
/// are reported with [`VisitStatus::Cycle`].
///
/// If no root nodes are found (due to all candidates being a part of a cycle), no nodes will be
/// visited.
pub fn walk(graph: &DepGraph, visitor: &mut dyn TreeVisitor) -> eyre::Result<()> {
    let data = GraphData {
        nodes: graph.all_nodes(),
        adj: graph.adjacency_list(),
        default_info: NodeInfo::new(""),
    };

    // Find roots: nodes with no incoming edges.
    let targets: HashSet<&str> = graph.all_edges().iter().map(|e| e.to.as_str()).collect();
    let roots: Vec<&str> = data
        .nodes
        .keys()
        .map(String::as_str)
        .filter(|n| !targets.contains(n))
        .collect();

    let mut visited = HashSet::new();
    let mut in_progress = HashSet::new();

    let root_count = roots.len();
    for (i, root) in roots.iter().enumerate() {
        dfs(
            root,
            0,
            i == root_count - 1,
            &data,
            &mut visited,
            &mut in_progress,
            visitor,
        )?;
    }

    Ok(())
}

struct GraphData<'a> {
    nodes: &'a IndexMap<String, NodeInfo>,
    adj: &'a IndexMap<String, Vec<String>>,
    default_info: NodeInfo,
}

fn dfs<'a>(
    node: &'a str,
    depth: usize,
    is_last: bool,
    data: &'a GraphData<'a>,
    visited: &mut HashSet<&'a str>,
    in_progress: &mut HashSet<&'a str>,
    visitor: &mut dyn TreeVisitor,
) -> eyre::Result<()> {
    let info = data.nodes.get(node).unwrap_or(&data.default_info);
    let children = data.adj.get(node);
    let child_count = children.map_or(0, |c| c.len());

    // Cycle: node is an ancestor on the current DFS path.
    if in_progress.contains(node) {
        visitor.visit(&VisitContext {
            node_id: node,
            info,
            depth,
            is_last,
            child_count,
            status: VisitStatus::Cycle,
        })?;
        return Ok(());
    }

    // Already expanded: node was fully visited from a different path.
    if visited.contains(node) {
        visitor.visit(&VisitContext {
            node_id: node,
            info,
            depth,
            is_last,
            child_count,
            status: VisitStatus::AlreadyExpanded,
        })?;
        return Ok(());
    }

    // First visit.
    visitor.visit(&VisitContext {
        node_id: node,
        info,
        depth,
        is_last,
        child_count,
        status: VisitStatus::First,
    })?;

    in_progress.insert(node);

    if let Some(children) = children {
        let len = children.len();
        for (i, child) in children.iter().enumerate() {
            dfs(
                child.as_str(),
                depth + 1,
                i == len - 1,
                data,
                visited,
                in_progress,
                visitor,
            )?;
        }
    }

    in_progress.remove(node);
    visited.insert(node);

    Ok(())
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;

    use super::*;
    use crate::{Edge, NodeInfo};

    #[derive(Debug, PartialEq)]
    struct Visit {
        node: String,
        depth: usize,
        is_last: bool,
        child_count: usize,
        status: VisitStatus,
    }

    fn v(
        node: &str,
        depth: usize,
        is_last: bool,
        child_count: usize,
        status: VisitStatus,
    ) -> Visit {
        Visit {
            node: node.to_string(),
            depth,
            is_last,
            child_count,
            status,
        }
    }

    struct CollectVisitor {
        visits: Vec<Visit>,
    }

    impl CollectVisitor {
        fn new() -> Self {
            Self { visits: Vec::new() }
        }
    }

    impl TreeVisitor for CollectVisitor {
        fn visit(&mut self, ctx: &VisitContext) -> eyre::Result<()> {
            self.visits.push(Visit {
                node: ctx.node_id.to_string(),
                depth: ctx.depth,
                is_last: ctx.is_last,
                child_count: ctx.child_count,
                status: ctx.status,
            });
            Ok(())
        }
    }

    #[test]
    fn empty_graph() {
        let mut visitor = CollectVisitor::new();
        walk(&DepGraph::default(), &mut visitor).unwrap();
        assert_eq!(visitor.visits, vec![]);
    }

    #[test]
    fn single_node() {
        let graph = DepGraph {
            nodes: IndexMap::from([(
                "a".into(),
                NodeInfo {
                    label: "a".into(),
                    node_type: None,
                    attrs: Default::default(),
                },
            )]),
            ..Default::default()
        };
        let mut visitor = CollectVisitor::new();
        walk(&graph, &mut visitor).unwrap();
        assert_eq!(visitor.visits, vec![v("a", 0, true, 0, VisitStatus::First)]);
    }

    #[test]
    fn linear_chain() {
        let graph = DepGraph {
            nodes: IndexMap::from([
                (
                    "a".into(),
                    NodeInfo {
                        label: "a".into(),
                        node_type: None,
                        attrs: Default::default(),
                    },
                ),
                (
                    "b".into(),
                    NodeInfo {
                        label: "b".into(),
                        node_type: None,
                        attrs: Default::default(),
                    },
                ),
                (
                    "c".into(),
                    NodeInfo {
                        label: "c".into(),
                        node_type: None,
                        attrs: Default::default(),
                    },
                ),
            ]),
            edges: vec![
                Edge {
                    from: "a".into(),
                    to: "b".into(),
                    ..Default::default()
                },
                Edge {
                    from: "b".into(),
                    to: "c".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mut visitor = CollectVisitor::new();
        walk(&graph, &mut visitor).unwrap();
        assert_eq!(
            visitor.visits,
            vec![
                v("a", 0, true, 1, VisitStatus::First),
                v("b", 1, true, 1, VisitStatus::First),
                v("c", 2, true, 0, VisitStatus::First),
            ]
        );
    }

    #[test]
    fn diamond_dag() {
        // a -> b -> d, a -> c -> d
        // d is expanded under b, then AlreadyExpanded under c.
        let graph = DepGraph {
            nodes: IndexMap::from([
                (
                    "a".into(),
                    NodeInfo {
                        label: "a".into(),
                        node_type: None,
                        attrs: Default::default(),
                    },
                ),
                (
                    "b".into(),
                    NodeInfo {
                        label: "b".into(),
                        node_type: None,
                        attrs: Default::default(),
                    },
                ),
                (
                    "c".into(),
                    NodeInfo {
                        label: "c".into(),
                        node_type: None,
                        attrs: Default::default(),
                    },
                ),
                (
                    "d".into(),
                    NodeInfo {
                        label: "d".into(),
                        node_type: None,
                        attrs: Default::default(),
                    },
                ),
            ]),
            edges: vec![
                Edge {
                    from: "a".into(),
                    to: "b".into(),
                    ..Default::default()
                },
                Edge {
                    from: "a".into(),
                    to: "c".into(),
                    ..Default::default()
                },
                Edge {
                    from: "b".into(),
                    to: "d".into(),
                    ..Default::default()
                },
                Edge {
                    from: "c".into(),
                    to: "d".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mut visitor = CollectVisitor::new();
        walk(&graph, &mut visitor).unwrap();
        assert_eq!(
            visitor.visits,
            vec![
                v("a", 0, true, 2, VisitStatus::First),
                v("b", 1, false, 1, VisitStatus::First),
                v("d", 2, true, 0, VisitStatus::First),
                v("c", 1, true, 1, VisitStatus::First),
                v("d", 2, true, 0, VisitStatus::AlreadyExpanded),
            ]
        );
    }

    #[test]
    fn visitor_skips_cycles() {
        // a -> b -> a
        let graph = DepGraph {
            nodes: IndexMap::from([
                (
                    "a".into(),
                    NodeInfo {
                        label: "a".into(),
                        node_type: None,
                        attrs: Default::default(),
                    },
                ),
                (
                    "b".into(),
                    NodeInfo {
                        label: "b".into(),
                        node_type: None,
                        attrs: Default::default(),
                    },
                ),
            ]),
            edges: vec![
                Edge {
                    from: "a".into(),
                    to: "b".into(),
                    ..Default::default()
                },
                Edge {
                    from: "b".into(),
                    to: "a".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mut visitor = CollectVisitor::new();
        walk(&graph, &mut visitor).unwrap();
        // Because the root is involved in a cycle, the visitor will never visit any of the nodes
        assert_eq!(visitor.visits, vec![]);
    }

    #[test]
    fn self_loop() {
        // a -> a
        let graph = DepGraph {
            nodes: IndexMap::from([(
                "a".into(),
                NodeInfo {
                    label: "a".into(),
                    node_type: None,
                    attrs: Default::default(),
                },
            )]),
            edges: vec![Edge {
                from: "a".into(),
                to: "a".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut visitor = CollectVisitor::new();
        walk(&graph, &mut visitor).unwrap();
        // a has an incoming edge (from itself), so it's not a root.
        // No roots, no visits.
        assert_eq!(visitor.visits, vec![]);
    }

    #[test]
    fn cycle_with_entry() {
        // a -> b -> c -> b (c cycles back to b, a is the root)
        let graph = DepGraph {
            nodes: IndexMap::from([
                (
                    "a".into(),
                    NodeInfo {
                        label: "a".into(),
                        node_type: None,
                        attrs: Default::default(),
                    },
                ),
                (
                    "b".into(),
                    NodeInfo {
                        label: "b".into(),
                        node_type: None,
                        attrs: Default::default(),
                    },
                ),
                (
                    "c".into(),
                    NodeInfo {
                        label: "c".into(),
                        node_type: None,
                        attrs: Default::default(),
                    },
                ),
            ]),
            edges: vec![
                Edge {
                    from: "a".into(),
                    to: "b".into(),
                    ..Default::default()
                },
                Edge {
                    from: "b".into(),
                    to: "c".into(),
                    ..Default::default()
                },
                Edge {
                    from: "c".into(),
                    to: "b".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mut visitor = CollectVisitor::new();
        walk(&graph, &mut visitor).unwrap();
        assert_eq!(
            visitor.visits,
            vec![
                v("a", 0, true, 1, VisitStatus::First),
                v("b", 1, true, 1, VisitStatus::First),
                v("c", 2, true, 1, VisitStatus::First),
                v("b", 3, true, 1, VisitStatus::Cycle),
            ]
        );
    }

    #[test]
    fn multiple_roots() {
        // a (isolated), b -> c
        let graph = DepGraph {
            nodes: IndexMap::from([
                (
                    "a".into(),
                    NodeInfo {
                        label: "a".into(),
                        node_type: None,
                        attrs: Default::default(),
                    },
                ),
                (
                    "b".into(),
                    NodeInfo {
                        label: "b".into(),
                        node_type: None,
                        attrs: Default::default(),
                    },
                ),
                (
                    "c".into(),
                    NodeInfo {
                        label: "c".into(),
                        node_type: None,
                        attrs: Default::default(),
                    },
                ),
            ]),
            edges: vec![Edge {
                from: "b".into(),
                to: "c".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut visitor = CollectVisitor::new();
        walk(&graph, &mut visitor).unwrap();
        assert_eq!(
            visitor.visits,
            vec![
                v("a", 0, false, 0, VisitStatus::First),
                v("b", 0, true, 1, VisitStatus::First),
                v("c", 1, true, 0, VisitStatus::First),
            ]
        );
    }

    #[test]
    fn shared_across_roots() {
        // a -> c, b -> c (both a and b are roots, c is shared)
        let graph = DepGraph {
            nodes: IndexMap::from([
                (
                    "a".into(),
                    NodeInfo {
                        label: "a".into(),
                        node_type: None,
                        attrs: Default::default(),
                    },
                ),
                (
                    "b".into(),
                    NodeInfo {
                        label: "b".into(),
                        node_type: None,
                        attrs: Default::default(),
                    },
                ),
                (
                    "c".into(),
                    NodeInfo {
                        label: "c".into(),
                        node_type: None,
                        attrs: Default::default(),
                    },
                ),
            ]),
            edges: vec![
                Edge {
                    from: "a".into(),
                    to: "c".into(),
                    ..Default::default()
                },
                Edge {
                    from: "b".into(),
                    to: "c".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mut visitor = CollectVisitor::new();
        walk(&graph, &mut visitor).unwrap();
        assert_eq!(
            visitor.visits,
            vec![
                v("a", 0, false, 1, VisitStatus::First),
                v("c", 1, true, 0, VisitStatus::First),
                v("b", 0, true, 1, VisitStatus::First),
                v("c", 1, true, 0, VisitStatus::AlreadyExpanded),
            ]
        );
    }

    #[test]
    fn sample_graph() {
        // a -> b -> c, a -> c
        let graph = crate::emit::fixtures::sample_graph();
        let mut visitor = CollectVisitor::new();
        walk(&graph, &mut visitor).unwrap();
        assert_eq!(
            visitor.visits,
            vec![
                v("a", 0, true, 2, VisitStatus::First),
                v("b", 1, false, 1, VisitStatus::First),
                v("c", 2, true, 0, VisitStatus::First),
                v("c", 1, true, 0, VisitStatus::AlreadyExpanded),
            ]
        );
    }

    #[test]
    fn subgraph_nodes_included() {
        let graph = DepGraph {
            nodes: IndexMap::from([(
                "root".into(),
                NodeInfo {
                    label: "root".into(),
                    node_type: None,
                    attrs: Default::default(),
                },
            )]),
            edges: vec![Edge {
                from: "root".into(),
                to: "sub_a".into(),
                ..Default::default()
            }],
            subgraphs: vec![DepGraph {
                nodes: IndexMap::from([
                    (
                        "sub_a".into(),
                        NodeInfo {
                            label: "sub_a".into(),
                            node_type: None,
                            attrs: Default::default(),
                        },
                    ),
                    (
                        "sub_b".into(),
                        NodeInfo {
                            label: "sub_b".into(),
                            node_type: None,
                            attrs: Default::default(),
                        },
                    ),
                ]),
                edges: vec![Edge {
                    from: "sub_a".into(),
                    to: "sub_b".into(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut visitor = CollectVisitor::new();
        walk(&graph, &mut visitor).unwrap();
        assert_eq!(
            visitor.visits,
            vec![
                v("root", 0, true, 1, VisitStatus::First),
                v("sub_a", 1, true, 1, VisitStatus::First),
                v("sub_b", 2, true, 0, VisitStatus::First),
            ]
        );
    }
}
