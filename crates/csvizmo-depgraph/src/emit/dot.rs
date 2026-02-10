use std::io::Write;

use crate::DepGraph;

/// DOT double-quoted string: wrap in `"..."` and escape `"` -> `\"`.
///
/// We intentionally do NOT escape `\` -> `\\`. DOT uses backslash sequences like
/// `\n`, `\l`, `\r` as label formatting directives, and our internal representation
/// preserves all DOT backslash sequences verbatim (see `unquote` in `parse/dot.rs`).
/// Escaping backslashes here would corrupt formatting directives on DOT -> DOT round-trip.
fn quote(s: &str) -> String {
    let escaped = s.replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// Returns true if `s` is a bare DOT identifier (alphanumeric + underscore, not
/// digit-leading, and not a DOT reserved keyword).
fn is_bare_id(s: &str) -> bool {
    !s.is_empty()
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !s.starts_with(|c: char| c.is_ascii_digit())
        && !is_dot_keyword(s)
}

fn is_dot_keyword(s: &str) -> bool {
    s.eq_ignore_ascii_case("node")
        || s.eq_ignore_ascii_case("edge")
        || s.eq_ignore_ascii_case("graph")
        || s.eq_ignore_ascii_case("digraph")
        || s.eq_ignore_ascii_case("subgraph")
        || s.eq_ignore_ascii_case("strict")
}

/// Quote a DOT ID: bare identifiers pass through, everything else gets double-quoted.
fn quote_id(s: &str) -> String {
    if is_bare_id(s) {
        s.to_string()
    } else {
        quote(s)
    }
}

/// Emit a [`DepGraph`] as a DOT `digraph`.
///
/// All graph features are preserved:
/// - Graph name is taken from `graph.id`.
/// - Graph-level attrs are emitted as top-level `key="val";` statements.
/// - Node labels and arbitrary attrs are emitted as `[key="val", ...]`.
/// - Edge labels and arbitrary attrs are emitted the same way.
/// - Subgraphs are emitted as nested `subgraph <id> { ... }` blocks.
/// - Identifiers are bare when safe (alphanumeric, non-keyword), otherwise
///   double-quoted. Backslash sequences (`\n`, `\l`, `\r`) are preserved
///   verbatim for DOT -> DOT round-trips.
pub fn emit(graph: &DepGraph, writer: &mut dyn Write) -> eyre::Result<()> {
    // Emit graph header with optional name.
    if let Some(name) = &graph.id {
        writeln!(writer, "digraph {} {{", quote_id(name))?;
    } else {
        writeln!(writer, "digraph {{")?;
    }

    emit_body(graph, writer, 1)?;

    writeln!(writer, "}}")?;
    Ok(())
}

/// Emit the body of a graph or subgraph: attrs, subgraphs, nodes, edges.
fn emit_body(graph: &DepGraph, writer: &mut dyn Write, depth: usize) -> eyre::Result<()> {
    let indent = "    ".repeat(depth);

    // Emit graph-level attributes.
    for (k, v) in &graph.attrs {
        writeln!(writer, "{indent}{}={};", quote_id(k), quote(v))?;
    }

    // Emit subgraphs before nodes/edges (matches typical DOT convention).
    for sg in &graph.subgraphs {
        emit_subgraph(sg, writer, depth)?;
    }

    // Emit nodes.
    for (id, info) in &graph.nodes {
        emit_node(id, info, writer, depth)?;
    }

    // Emit edges.
    for edge in &graph.edges {
        emit_edge(edge, writer, depth)?;
    }

    Ok(())
}

fn emit_node(
    id: &str,
    info: &crate::NodeInfo,
    writer: &mut dyn Write,
    depth: usize,
) -> eyre::Result<()> {
    let indent = "    ".repeat(depth);
    let mut attrs = Vec::new();
    if let Some(label) = &info.label {
        attrs.push(format!("label={}", quote(label)));
    }
    for (k, v) in &info.attrs {
        attrs.push(format!("{}={}", quote_id(k), quote(v)));
    }

    if attrs.is_empty() {
        writeln!(writer, "{indent}{};", quote_id(id))?;
    } else {
        writeln!(writer, "{indent}{} [{}];", quote_id(id), attrs.join(", "))?;
    }
    Ok(())
}

fn emit_edge(edge: &crate::Edge, writer: &mut dyn Write, depth: usize) -> eyre::Result<()> {
    let indent = "    ".repeat(depth);
    let mut attrs = Vec::new();
    if let Some(label) = &edge.label {
        attrs.push(format!("label={}", quote(label)));
    }
    for (k, v) in &edge.attrs {
        attrs.push(format!("{}={}", quote_id(k), quote(v)));
    }

    if attrs.is_empty() {
        writeln!(
            writer,
            "{indent}{} -> {};",
            quote_id(&edge.from),
            quote_id(&edge.to)
        )?;
    } else {
        writeln!(
            writer,
            "{indent}{} -> {} [{}];",
            quote_id(&edge.from),
            quote_id(&edge.to),
            attrs.join(", ")
        )?;
    }
    Ok(())
}

fn emit_subgraph(sg: &DepGraph, writer: &mut dyn Write, depth: usize) -> eyre::Result<()> {
    let indent = "    ".repeat(depth);

    if let Some(id) = &sg.id {
        writeln!(writer, "{indent}subgraph {} {{", quote_id(id))?;
    } else {
        writeln!(writer, "{indent}subgraph {{")?;
    }

    emit_body(sg, writer, depth + 1)?;

    writeln!(writer, "{indent}}}")?;
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
    fn quote_plain() {
        assert_eq!(quote("hello"), r#""hello""#);
    }

    #[test]
    fn quote_with_quotes() {
        assert_eq!(quote(r#"say "hi""#), r#""say \"hi\"""#);
    }

    #[test]
    fn quote_backslash_preserved() {
        // Backslashes pass through unchanged -- DOT formatting directives
        // like \n, \l, \r must not be double-escaped.
        assert_eq!(quote(r"a\nb"), r#""a\nb""#);
        assert_eq!(quote(r"a\\b"), r#""a\\b""#);
    }

    #[test]
    fn quote_backslash_before_quote() {
        // Internal \\" (backslash + quote) must produce \\\" in DOT.
        assert_eq!(quote(r#"\\"b"#), r#""\\\"b""#);
    }

    #[test]
    fn quote_unquote_roundtrip() {
        let cases = [
            "hello",
            r#"say "hi""#,
            r"path\to\file",
            r"line1\nline2",
            r"a\\b",
            r#"a\\"b"#,
            "",
        ];
        for s in cases {
            let quoted = quote(s);
            let roundtripped = crate::parse::dot::unquote(&quoted);
            assert_eq!(roundtripped, s, "round-trip failed for {s:?}");
        }
    }

    #[test]
    fn empty_graph() {
        let output = emit_to_string(&DepGraph::default());
        assert_eq!(output, "digraph {\n}\n");
    }

    #[test]
    fn sample() {
        let output = emit_to_string(&sample_graph());
        assert_eq!(
            output,
            "\
digraph {
    a [label=\"alpha\"];
    b [label=\"bravo\"];
    c;
    a -> b [label=\"depends\"];
    b -> c;
    a -> c;
}
"
        );
    }

    #[test]
    fn nodes_only() {
        let mut nodes = IndexMap::new();
        nodes.insert(
            "x".into(),
            NodeInfo {
                label: Some("X Node".into()),
                ..Default::default()
            },
        );
        nodes.insert("y".into(), NodeInfo::default());
        let graph = DepGraph {
            nodes,
            edges: vec![],
            ..Default::default()
        };
        let output = emit_to_string(&graph);
        assert_eq!(
            output,
            "\
digraph {
    x [label=\"X Node\"];
    y;
}
"
        );
    }

    #[test]
    fn node_attrs() {
        let mut nodes = IndexMap::new();
        nodes.insert(
            "mylib".into(),
            NodeInfo {
                label: Some("My Library".into()),
                attrs: IndexMap::from([
                    ("shape".into(), "box".into()),
                    ("version".into(), "1.0".into()),
                ]),
            },
        );
        let graph = DepGraph {
            nodes,
            edges: vec![],
            ..Default::default()
        };
        let output = emit_to_string(&graph);
        assert_eq!(
            output,
            "\
digraph {
    mylib [label=\"My Library\", shape=\"box\", version=\"1.0\"];
}
"
        );
    }

    #[test]
    fn special_chars_in_ids() {
        let mut nodes = IndexMap::new();
        nodes.insert("my node".into(), NodeInfo::default());
        nodes.insert(
            "has\"quotes".into(),
            NodeInfo {
                label: Some("a \"label\"".into()),
                ..Default::default()
            },
        );
        let graph = DepGraph {
            nodes,
            edges: vec![Edge {
                from: "my node".into(),
                to: "has\"quotes".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let output = emit_to_string(&graph);
        assert_eq!(
            output,
            "\
digraph {
    \"my node\";
    \"has\\\"quotes\" [label=\"a \\\"label\\\"\"];
    \"my node\" -> \"has\\\"quotes\";
}
"
        );
    }

    #[test]
    fn bare_ids_not_quoted() {
        let mut nodes = IndexMap::new();
        nodes.insert("foo_bar".into(), NodeInfo::default());
        nodes.insert("Baz123".into(), NodeInfo::default());
        let graph = DepGraph {
            nodes,
            edges: vec![Edge {
                from: "foo_bar".into(),
                to: "Baz123".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let output = emit_to_string(&graph);
        assert_eq!(
            output,
            "\
digraph {
    foo_bar;
    Baz123;
    foo_bar -> Baz123;
}
"
        );
    }

    #[test]
    fn dot_keyword_ids_are_quoted() {
        let mut nodes = IndexMap::new();
        nodes.insert("node".into(), NodeInfo::default());
        nodes.insert("edge".into(), NodeInfo::default());
        let graph = DepGraph {
            nodes,
            edges: vec![Edge {
                from: "node".into(),
                to: "edge".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let output = emit_to_string(&graph);
        assert_eq!(
            output,
            "\
digraph {
    \"node\";
    \"edge\";
    \"node\" -> \"edge\";
}
"
        );
    }

    #[test]
    fn dot_keyword_ids_case_insensitive() {
        let mut nodes = IndexMap::new();
        nodes.insert("Node".into(), NodeInfo::default());
        nodes.insert("GRAPH".into(), NodeInfo::default());
        nodes.insert("Subgraph".into(), NodeInfo::default());
        let graph = DepGraph {
            nodes,
            ..Default::default()
        };
        let output = emit_to_string(&graph);
        assert_eq!(
            output,
            "\
digraph {
    \"Node\";
    \"GRAPH\";
    \"Subgraph\";
}
"
        );
    }

    #[test]
    fn digit_leading_id_is_quoted() {
        let mut nodes = IndexMap::new();
        nodes.insert("1abc".into(), NodeInfo::default());
        let graph = DepGraph {
            nodes,
            edges: vec![],
            ..Default::default()
        };
        let output = emit_to_string(&graph);
        assert_eq!(
            output,
            "\
digraph {
    \"1abc\";
}
"
        );
    }

    #[test]
    fn edge_labels() {
        let graph = DepGraph {
            nodes: IndexMap::new(),
            edges: vec![
                Edge {
                    from: "a".into(),
                    to: "b".into(),
                    label: Some("uses".into()),
                    ..Default::default()
                },
                Edge {
                    from: "a".into(),
                    to: "c".into(),
                    label: Some("has space".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let output = emit_to_string(&graph);
        assert_eq!(
            output,
            "\
digraph {
    a -> b [label=\"uses\"];
    a -> c [label=\"has space\"];
}
"
        );
    }

    #[test]
    fn edge_attrs_emitted() {
        let graph = DepGraph {
            nodes: IndexMap::new(),
            edges: vec![Edge {
                from: "a".into(),
                to: "b".into(),
                label: Some("uses".into()),
                attrs: IndexMap::from([
                    ("style".into(), "dashed".into()),
                    ("color".into(), "red".into()),
                ]),
            }],
            ..Default::default()
        };
        let output = emit_to_string(&graph);
        assert_eq!(
            output,
            "\
digraph {
    a -> b [label=\"uses\", style=\"dashed\", color=\"red\"];
}
"
        );
    }

    #[test]
    fn graph_name_emitted() {
        let graph = DepGraph {
            id: Some("deps".into()),
            nodes: IndexMap::new(),
            edges: vec![],
            ..Default::default()
        };
        let output = emit_to_string(&graph);
        assert_eq!(output, "digraph deps {\n}\n");
    }

    #[test]
    fn graph_attrs_emitted() {
        let graph = DepGraph {
            id: Some("deps".into()),
            attrs: IndexMap::from([("rankdir".into(), "LR".into())]),
            nodes: IndexMap::new(),
            edges: vec![],
            ..Default::default()
        };
        let output = emit_to_string(&graph);
        assert_eq!(
            output,
            "\
digraph deps {
    rankdir=\"LR\";
}
"
        );
    }

    #[test]
    fn graph_and_node_and_edge_attrs_combined() {
        let graph = DepGraph {
            id: Some("deps".into()),
            attrs: IndexMap::from([("rankdir".into(), "LR".into())]),
            nodes: IndexMap::from([
                (
                    "a".into(),
                    NodeInfo {
                        label: Some("A".into()),
                        attrs: IndexMap::from([("shape".into(), "box".into())]),
                    },
                ),
                ("b".into(), NodeInfo::default()),
            ]),
            edges: vec![Edge {
                from: "a".into(),
                to: "b".into(),
                attrs: IndexMap::from([
                    ("style".into(), "dashed".into()),
                    ("color".into(), "red".into()),
                ]),
                ..Default::default()
            }],
            ..Default::default()
        };
        let output = emit_to_string(&graph);
        assert_eq!(
            output,
            "\
digraph deps {
    rankdir=\"LR\";
    a [label=\"A\", shape=\"box\"];
    b;
    a -> b [style=\"dashed\", color=\"red\"];
}
"
        );
    }

    #[test]
    fn subgraph_emitted() {
        let graph = DepGraph {
            nodes: IndexMap::from([("top".into(), NodeInfo::default())]),
            subgraphs: vec![DepGraph {
                id: Some("cluster0".into()),
                attrs: IndexMap::from([("label".into(), "Group A".into())]),
                nodes: IndexMap::from([
                    ("a".into(), NodeInfo::default()),
                    ("b".into(), NodeInfo::default()),
                ]),
                edges: vec![Edge {
                    from: "a".into(),
                    to: "b".into(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            edges: vec![Edge {
                from: "top".into(),
                to: "a".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let output = emit_to_string(&graph);
        assert_eq!(
            output,
            "\
digraph {
    subgraph cluster0 {
        label=\"Group A\";
        a;
        b;
        a -> b;
    }
    top;
    top -> a;
}
"
        );
    }
}
