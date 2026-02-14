use std::io::Write;

use super::walk::{self, TreeVisitor, VisitContext, VisitStatus};
use crate::DepGraph;

/// Emit a [`DepGraph`] as a pathlist (one path per line).
///
/// Performs a DFS tree walk and emits one line per leaf node, joining
/// ancestor labels with `/` to form a path. Intermediate nodes (those
/// with children that are being expanded) do not produce output lines --
/// they appear only as path prefixes.
///
/// Nodes whose subtrees are truncated are annotated with tab-separated
/// markers and a trailing `/`:
/// - `path/to/node/\t(*)` for nodes whose children were already expanded elsewhere
/// - `path/to/node/\t(cycle)` for back-edges (cycles)
///
/// Childless nodes that were already visited are emitted as plain leaves
/// (no marker) since there is no subtree being suppressed.
///
/// The markers can be stripped with `cut -f1` or filtered with `cut -f2`.
///
/// Preserves node labels (as path components). Everything else is
/// silently dropped: graph attrs, node attrs, edge labels, edge attrs.
pub fn emit(graph: &DepGraph, writer: &mut dyn Write) -> eyre::Result<()> {
    let mut visitor = PathlistVisitor {
        writer,
        stack: Vec::new(),
    };
    walk::walk(graph, &mut visitor)
}

struct PathlistVisitor<'w> {
    writer: &'w mut dyn Write,
    stack: Vec<String>,
}

impl TreeVisitor for PathlistVisitor<'_> {
    fn visit(&mut self, ctx: &VisitContext) -> eyre::Result<()> {
        self.stack.truncate(ctx.depth);
        let label = &ctx.info.label;
        self.stack.push(label.to_string());

        let is_leaf = ctx.child_count == 0;

        match ctx.status {
            // Leaf node (no children): emit plain path.
            _ if is_leaf => {
                let path = self.stack.join("/");
                writeln!(self.writer, "{path}")?;
            }
            // Non-leaf already expanded elsewhere: subtree suppressed.
            VisitStatus::AlreadyExpanded => {
                let path = self.stack.join("/");
                writeln!(self.writer, "{path}/\t(*)")?;
            }
            // Non-leaf cycle back-edge: subtree suppressed.
            VisitStatus::Cycle => {
                let path = self.stack.join("/");
                writeln!(self.writer, "{path}/\t(cycle)")?;
            }
            // Non-leaf first visit: intermediate, don't emit.
            VisitStatus::First => {}
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;

    use super::*;
    use crate::{Edge, NodeInfo};

    fn emit_to_string(graph: &DepGraph) -> String {
        let mut buf = Vec::new();
        emit(graph, &mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn empty_graph() {
        assert_eq!(emit_to_string(&DepGraph::default()), "");
    }

    #[test]
    fn single_node() {
        let graph = DepGraph {
            nodes: IndexMap::from([("readme".into(), NodeInfo::new("README.md"))]),
            ..Default::default()
        };
        assert_eq!(emit_to_string(&graph), "README.md\n");
    }

    #[test]
    fn linear_chain() {
        let graph = DepGraph {
            nodes: IndexMap::from([
                ("src".into(), NodeInfo::new("src")),
                ("src/parse".into(), NodeInfo::new("parse")),
                ("src/parse/tgf.rs".into(), NodeInfo::new("tgf.rs")),
            ]),
            edges: vec![
                Edge {
                    from: "src".into(),
                    to: "src/parse".into(),
                    ..Default::default()
                },
                Edge {
                    from: "src/parse".into(),
                    to: "src/parse/tgf.rs".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(emit_to_string(&graph), "src/parse/tgf.rs\n");
    }

    #[test]
    fn branching() {
        let graph = DepGraph {
            nodes: IndexMap::from([
                ("src".into(), NodeInfo::new("src")),
                ("src/a.rs".into(), NodeInfo::new("a.rs")),
                ("src/b.rs".into(), NodeInfo::new("b.rs")),
            ]),
            edges: vec![
                Edge {
                    from: "src".into(),
                    to: "src/a.rs".into(),
                    ..Default::default()
                },
                Edge {
                    from: "src".into(),
                    to: "src/b.rs".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(emit_to_string(&graph), "src/a.rs\nsrc/b.rs\n");
    }

    #[test]
    fn diamond_already_expanded() {
        // a -> b -> d, a -> c -> d
        let graph = DepGraph {
            nodes: IndexMap::from([
                ("a".into(), NodeInfo::new("a")),
                ("b".into(), NodeInfo::new("b")),
                ("c".into(), NodeInfo::new("c")),
                ("d".into(), NodeInfo::new("d")),
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
        // d is a leaf with no children; it was already visited but nothing
        // is suppressed, so it appears as a plain leaf both times.
        assert_eq!(emit_to_string(&graph), "a/b/d\na/c/d\n");
    }

    #[test]
    fn already_expanded_with_children() {
        // a -> b -> c -> d, a -> c -> d
        // c has children (d), so when revisited under a it's a suppressed subtree.
        let graph = DepGraph {
            nodes: IndexMap::from([
                ("a".into(), NodeInfo::new("a")),
                ("b".into(), NodeInfo::new("b")),
                ("c".into(), NodeInfo::new("c")),
                ("d".into(), NodeInfo::new("d")),
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
                    to: "c".into(),
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
        // c was expanded under b (showing c/d), so under a it's truncated.
        assert_eq!(emit_to_string(&graph), "a/b/c/d\na/c/\t(*)\n");
    }

    #[test]
    fn cycle_marker() {
        // a -> b -> c -> b
        let graph = DepGraph {
            nodes: IndexMap::from([
                ("a".into(), NodeInfo::new("a")),
                ("b".into(), NodeInfo::new("b")),
                ("c".into(), NodeInfo::new("c")),
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
        assert_eq!(emit_to_string(&graph), "a/b/c/b/\t(cycle)\n");
    }

    #[test]
    fn falls_back_to_node_id() {
        // Nodes without labels use the node ID as the path component.
        let graph = DepGraph {
            nodes: IndexMap::from([
                ("root".into(), NodeInfo::new("root")),
                ("child".into(), NodeInfo::new("child")),
            ]),
            edges: vec![Edge {
                from: "root".into(),
                to: "child".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(emit_to_string(&graph), "root/child\n");
    }

    #[test]
    fn sample_graph() {
        // a(alpha) -> b(bravo) -> c, a -> c
        // c has no label, so falls back to node ID "c".
        let graph = crate::emit::fixtures::sample_graph();
        // c has no children, so no subtree is suppressed -- plain leaf.
        assert_eq!(emit_to_string(&graph), "alpha/bravo/c\nalpha/c\n");
    }

    #[test]
    fn multiple_roots() {
        let graph = DepGraph {
            nodes: IndexMap::from([
                ("a".into(), NodeInfo::new("a")),
                ("b".into(), NodeInfo::new("b")),
            ]),
            ..Default::default()
        };
        assert_eq!(emit_to_string(&graph), "a\nb\n");
    }

    #[test]
    fn drops_attrs_and_edge_labels() {
        let graph = DepGraph {
            attrs: IndexMap::from([("name".into(), "deps".into())]),
            nodes: IndexMap::from([
                (
                    "a".into(),
                    NodeInfo {
                        label: "a".into(),
                        node_type: None,
                        attrs: IndexMap::from([("shape".into(), "box".into())]),
                    },
                ),
                ("b".into(), NodeInfo::new("b")),
            ]),
            edges: vec![Edge {
                from: "a".into(),
                to: "b".into(),
                label: Some("uses".into()),
                attrs: IndexMap::from([("style".into(), "dashed".into())]),
            }],
            ..Default::default()
        };
        assert_eq!(emit_to_string(&graph), "a/b\n");
    }

    #[test]
    fn roundtrip_simple() {
        let input = "src/a.rs\nsrc/b.rs\n";
        let graph = crate::parse::parse(crate::parse::InputFormat::Pathlist, input).unwrap();
        assert_eq!(emit_to_string(&graph), input);
    }

    #[test]
    fn roundtrip_nested() {
        let input = "a/b/c\na/b/d\na/e\n";
        let graph = crate::parse::parse(crate::parse::InputFormat::Pathlist, input).unwrap();
        assert_eq!(emit_to_string(&graph), input);
    }

    #[test]
    fn subgraph_nodes_included() {
        let graph = DepGraph {
            nodes: IndexMap::from([("root".into(), NodeInfo::new("root"))]),
            edges: vec![Edge {
                from: "root".into(),
                to: "child".into(),
                ..Default::default()
            }],
            subgraphs: vec![DepGraph {
                nodes: IndexMap::from([("child".into(), NodeInfo::new("child"))]),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(emit_to_string(&graph), "root/child\n");
    }
}
