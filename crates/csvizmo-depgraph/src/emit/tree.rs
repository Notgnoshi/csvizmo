use std::io::Write;

use super::walk::{self, TreeVisitor, VisitContext, VisitStatus};
use crate::DepGraph;

/// Emit a [`DepGraph`] as a box-drawing tree (matching `tree` CLI output).
///
/// Performs a DFS tree walk and emits every node with box-drawing
/// prefixes that show the tree structure. Root nodes appear at the
/// left margin; children are indented with branch characters.
///
/// Nodes whose subtrees are truncated carry a suffix marker:
/// - ` (*)` for nodes whose children were already expanded elsewhere
/// - ` (cycle)` for back-edges (cycles)
///
/// Childless nodes that were already visited are emitted without a
/// marker since there is no subtree being suppressed.
///
/// Preserves node labels. Everything else is silently dropped:
/// graph attrs, node attrs, edge labels, edge attrs.
pub fn emit(graph: &DepGraph, writer: &mut dyn Write) -> eyre::Result<()> {
    let mut visitor = TreeEmitVisitor {
        writer,
        prefix_stack: Vec::new(),
    };
    walk::walk(graph, &mut visitor)
}

struct TreeEmitVisitor<'w> {
    writer: &'w mut dyn Write,
    // prefix_stack[i] = is_last for the ancestor at depth i+1.
    // Used to decide continuation columns: is_last draws spaces,
    // otherwise draws a vertical bar.
    prefix_stack: Vec<bool>,
}

impl TreeVisitor for TreeEmitVisitor<'_> {
    fn visit(&mut self, ctx: &VisitContext) -> eyre::Result<()> {
        // Keep only the ancestor entries for depths 1..ctx.depth.
        self.prefix_stack.truncate(ctx.depth.saturating_sub(1));

        if ctx.depth > 0 {
            // Continuation columns for each ancestor.
            for &ancestor_is_last in &self.prefix_stack {
                if ancestor_is_last {
                    write!(self.writer, "    ")?;
                } else {
                    write!(self.writer, "│   ")?;
                }
            }
            // Branch for this node.
            if ctx.is_last {
                write!(self.writer, "└── ")?;
            } else {
                write!(self.writer, "├── ")?;
            }
        }

        let label = &ctx.info.label;
        write!(self.writer, "{label}")?;

        match ctx.status {
            VisitStatus::AlreadyExpanded if ctx.child_count > 0 => {
                write!(self.writer, " (*)")?;
            }
            VisitStatus::Cycle => {
                write!(self.writer, " (cycle)")?;
            }
            _ => {}
        }

        writeln!(self.writer)?;
        self.prefix_stack.push(ctx.is_last);

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
    fn single_root() {
        let graph = DepGraph {
            nodes: IndexMap::from([("r".into(), NodeInfo::new("root"))]),
            ..Default::default()
        };
        assert_eq!(emit_to_string(&graph), "root\n");
    }

    #[test]
    fn simple_children() {
        // root -> a, root -> b
        let graph = DepGraph {
            nodes: IndexMap::from([
                ("root".into(), NodeInfo::new("root")),
                ("a".into(), NodeInfo::new("alpha")),
                ("b".into(), NodeInfo::new("bravo")),
            ]),
            edges: vec![
                Edge {
                    from: "root".into(),
                    to: "a".into(),
                    ..Default::default()
                },
                Edge {
                    from: "root".into(),
                    to: "b".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(
            emit_to_string(&graph),
            "\
root
├── alpha
└── bravo
"
        );
    }

    #[test]
    fn nested() {
        // root -> a -> b, root -> c
        let graph = DepGraph {
            nodes: IndexMap::from([
                ("root".into(), NodeInfo::new("root")),
                ("a".into(), NodeInfo::new("a")),
                ("b".into(), NodeInfo::new("b")),
                ("c".into(), NodeInfo::new("c")),
            ]),
            edges: vec![
                Edge {
                    from: "root".into(),
                    to: "a".into(),
                    ..Default::default()
                },
                Edge {
                    from: "root".into(),
                    to: "c".into(),
                    ..Default::default()
                },
                Edge {
                    from: "a".into(),
                    to: "b".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(
            emit_to_string(&graph),
            "\
root
├── a
│   └── b
└── c
"
        );
    }

    #[test]
    fn deep_nesting_continuation() {
        // root -> a -> b -> c, root -> d
        // Verifies continuation columns render correctly at depth 3.
        let graph = DepGraph {
            nodes: IndexMap::from([
                ("root".into(), NodeInfo::new("root")),
                ("a".into(), NodeInfo::new("a")),
                ("b".into(), NodeInfo::new("b")),
                ("c".into(), NodeInfo::new("c")),
                ("d".into(), NodeInfo::new("d")),
            ]),
            edges: vec![
                Edge {
                    from: "root".into(),
                    to: "a".into(),
                    ..Default::default()
                },
                Edge {
                    from: "root".into(),
                    to: "d".into(),
                    ..Default::default()
                },
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
        // a is not last (d follows), so its column draws a vertical bar.
        // b is last child of a, so its column draws spaces.
        assert_eq!(
            emit_to_string(&graph),
            "\
root
├── a
│   └── b
│       └── c
└── d
"
        );
    }

    #[test]
    fn parallel_continuation_bars() {
        // root -> a -> b -> x, a -> c, root -> d
        // Both a and b have siblings after them, so two vertical bars
        // appear in parallel when rendering x at depth 3.
        let graph = DepGraph {
            nodes: IndexMap::from([
                ("root".into(), NodeInfo::new("root")),
                ("a".into(), NodeInfo::new("a")),
                ("b".into(), NodeInfo::new("b")),
                ("c".into(), NodeInfo::new("c")),
                ("d".into(), NodeInfo::new("d")),
                ("x".into(), NodeInfo::new("x")),
            ]),
            edges: vec![
                Edge {
                    from: "root".into(),
                    to: "a".into(),
                    ..Default::default()
                },
                Edge {
                    from: "root".into(),
                    to: "d".into(),
                    ..Default::default()
                },
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
                    to: "x".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(
            emit_to_string(&graph),
            "\
root
├── a
│   ├── b
│   │   └── x
│   └── c
└── d
"
        );
    }

    #[test]
    fn diamond_leaf_no_marker() {
        // a -> b -> d, a -> c -> d
        // d is a leaf -- no subtree suppressed, no marker.
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
        assert_eq!(
            emit_to_string(&graph),
            "\
a
├── b
│   └── d
└── c
    └── d
"
        );
    }

    #[test]
    fn already_expanded_with_children() {
        // a -> b, a -> c, b -> c, c -> d
        // c is expanded under b (showing d), then truncated under a.
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
        assert_eq!(
            emit_to_string(&graph),
            "\
a
├── b
│   └── c
│       └── d
└── c (*)
"
        );
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
        assert_eq!(
            emit_to_string(&graph),
            "\
a
└── b
    └── c
        └── b (cycle)
"
        );
    }

    #[test]
    fn falls_back_to_node_id() {
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
        assert_eq!(
            emit_to_string(&graph),
            "\
root
└── child
"
        );
    }

    #[test]
    fn sample_graph() {
        // a(alpha) -> b(bravo) -> c, a -> c
        // c has no label, falls back to "c". c is a leaf, no marker.
        let graph = crate::emit::fixtures::sample_graph();
        assert_eq!(
            emit_to_string(&graph),
            "\
alpha
├── bravo
│   └── c
└── c
"
        );
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
    fn roundtrip() {
        let input = "\
root
├── a
│   └── b
└── c
";
        let graph = crate::parse::parse(crate::parse::InputFormat::Tree, input).unwrap();
        assert_eq!(emit_to_string(&graph), input);
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
        assert_eq!(
            emit_to_string(&graph),
            "\
a
└── b
"
        );
    }
}
