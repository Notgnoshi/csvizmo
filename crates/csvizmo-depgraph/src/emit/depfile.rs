use std::io::Write;

use indexmap::IndexMap;

use crate::DepGraph;

/// Emit a `DepGraph` as a makefile-style `.d` depfile.
///
/// Each unique edge source becomes a target line: `target: dep1 dep2 ...`.
/// Nodes that have no outgoing edges are not emitted (they appear implicitly
/// as dependencies). Labels and attributes are silently dropped.
pub fn emit(graph: &DepGraph, writer: &mut dyn Write) -> eyre::Result<()> {
    // Group deps by target, preserving first-seen order for targets
    // and edge order for deps within each target.
    let mut targets: IndexMap<&str, Vec<&str>> = IndexMap::new();
    for edge in &graph.edges {
        targets.entry(&edge.from).or_default().push(&edge.to);
    }

    for (target, deps) in &targets {
        write!(writer, "{target}:")?;
        for dep in deps {
            write!(writer, " {dep}")?;
        }
        writeln!(writer)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;

    use super::*;
    use crate::emit::fixtures::sample_graph;
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
    fn single_target() {
        let graph = DepGraph {
            edges: vec![
                Edge {
                    from: "main.o".into(),
                    to: "main.c".into(),
                    ..Default::default()
                },
                Edge {
                    from: "main.o".into(),
                    to: "config.h".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(emit_to_string(&graph), "main.o: main.c config.h\n");
    }

    #[test]
    fn multiple_targets() {
        let graph = DepGraph {
            edges: vec![
                Edge {
                    from: "a.o".into(),
                    to: "a.c".into(),
                    ..Default::default()
                },
                Edge {
                    from: "b.o".into(),
                    to: "b.c".into(),
                    ..Default::default()
                },
                Edge {
                    from: "b.o".into(),
                    to: "common.h".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(emit_to_string(&graph), "a.o: a.c\nb.o: b.c common.h\n");
    }

    #[test]
    fn sample() {
        // a -> b, b -> c, a -> c
        let output = emit_to_string(&sample_graph());
        assert_eq!(output, "a: b c\nb: c\n");
    }

    #[test]
    fn nodes_only_produces_empty() {
        let graph = DepGraph {
            nodes: IndexMap::from([
                ("x".into(), NodeInfo::default()),
                ("y".into(), NodeInfo::default()),
            ]),
            edges: vec![],
            ..Default::default()
        };
        assert_eq!(emit_to_string(&graph), "");
    }

    #[test]
    fn rich_graph_drops_labels_and_attrs() {
        let graph = DepGraph {
            attrs: IndexMap::from([("name".into(), "deps".into())]),
            nodes: IndexMap::from([(
                "a".into(),
                NodeInfo {
                    label: Some("Alpha".into()),
                    attrs: IndexMap::from([("shape".into(), "box".into())]),
                },
            )]),
            edges: vec![Edge {
                from: "a".into(),
                to: "b".into(),
                label: Some("uses".into()),
                attrs: IndexMap::from([("style".into(), "dashed".into())]),
            }],
        };
        assert_eq!(emit_to_string(&graph), "a: b\n");
    }

    #[test]
    fn preserves_target_order() {
        let graph = DepGraph {
            edges: vec![
                Edge {
                    from: "z.o".into(),
                    to: "z.c".into(),
                    ..Default::default()
                },
                Edge {
                    from: "a.o".into(),
                    to: "a.c".into(),
                    ..Default::default()
                },
                Edge {
                    from: "m.o".into(),
                    to: "m.c".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(emit_to_string(&graph), "z.o: z.c\na.o: a.c\nm.o: m.c\n");
    }
}
