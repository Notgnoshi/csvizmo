use dot_parser::{ast, canonical};

use crate::{DepGraph, Edge, NodeInfo};

/// Decode a string value from the `dot-parser` crate.
///
/// The dot-parser is inconsistent: it strips outer quotes from attribute values
/// but preserves them on node IDs, edge endpoints, and graph names. Escape
/// sequences like `\"` are preserved as-is in both cases.
///
/// This function:
/// 1. Strips surrounding `"..."` if present (needed for node IDs / endpoints).
/// 2. Unescapes `\"` → `"` (needed for both — attribute values still have escapes).
///
/// We intentionally do NOT decode `\\` → `\`. DOT uses `\n`, `\l`, `\r` as label
/// formatting directives, and decoding `\\` would make `\\n` (literal backslash + n)
/// indistinguishable from `\n` (centered newline), corrupting DOT→DOT round-trips.
pub(crate) fn unquote(s: &str) -> String {
    let inner = if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        &s[1..s.len() - 1]
    } else {
        s
    };
    inner.replace("\\\"", "\"")
}

pub fn parse(input: &str) -> eyre::Result<DepGraph> {
    let ast_graph = ast::Graph::try_from(input).map_err(|e| eyre::eyre!("DOT parse error: {e}"))?;
    let graph = canonical::Graph::from(ast_graph);

    let mut dep = DepGraph::default();

    // Store graph name if present.
    if let Some(name) = &graph.name {
        dep.attrs.insert("name".into(), unquote(name));
    }

    // Store graph-level attributes from `graph [key=val]` statements.
    for attr_stmt in &graph.attr {
        if let canonical::AttrStmt::Graph(attr) = attr_stmt {
            let (k, v) = attr;
            let k: String = k.clone().into();
            let v: String = v.clone().into();
            dep.attrs.insert(unquote(&k), unquote(&v));
        }
    }

    // Store bare `key=val;` statements (ID equalities) as graph-level attributes.
    for ideq in &graph.ideqs {
        dep.attrs.insert(unquote(&ideq.lhs), unquote(&ideq.rhs));
    }

    // dot-parser uses HashMap internally so iteration order is non-deterministic.
    // Sort by node ID to ensure deterministic output.
    let mut sorted_nodes: Vec<_> = graph.nodes.set.iter().collect();
    sorted_nodes.sort_by_key(|(id, _)| *id);

    for (id, node) in sorted_nodes {
        let id = unquote(id);

        let mut info = NodeInfo::default();
        for (k, v) in &node.attr.elems {
            let k: String = k.clone().into();
            let v: String = v.clone().into();
            let key = unquote(&k);
            let value = unquote(&v);
            if key == "label" {
                info.label = Some(value);
            } else {
                info.attrs.insert(key, value);
            }
        }
        dep.nodes.insert(id, info);
    }

    for edge in &graph.edges.set {
        let from = unquote(&edge.from);
        let to = unquote(&edge.to);

        let mut label = None;
        let mut attrs = indexmap::IndexMap::new();
        for (k, v) in &edge.attr {
            let k: String = k.clone().into();
            let v: String = v.clone().into();
            let key = unquote(&k);
            let value = unquote(&v);
            if key == "label" {
                label = Some(value);
            } else {
                attrs.insert(key, value);
            }
        }
        // Ensure both endpoints exist as nodes (DOT can define nodes implicitly via edges).
        for endpoint in [&from, &to] {
            dep.nodes
                .entry(endpoint.clone())
                .or_insert_with(NodeInfo::default);
        }
        dep.edges.push(Edge {
            from,
            to,
            label,
            attrs,
        });
    }

    Ok(dep)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_parser_behavior() {
        let ast = ast::Graph::try_from(r#"digraph { a [label="My Label"]; "quoted node" -> a; }"#)
            .unwrap();
        let graph = canonical::Graph::from(ast);

        // Attribute value: parser strips outer quotes, gives bare text.
        let (_, node_a) = graph.nodes.set.iter().find(|(id, _)| *id == "a").unwrap();
        let (_, val) = &node_a.attr.elems[0];
        let val_str: String = val.clone().into();
        assert_eq!(val_str, "My Label");

        // Node ID: parser preserves outer quotes on quoted identifiers.
        assert!(graph.nodes.set.contains_key("\"quoted node\""));
        assert!(!graph.nodes.set.contains_key("quoted node"));

        // Edge endpoint: quotes also preserved.
        assert_eq!(graph.edges.set[0].from, "\"quoted node\"");
    }

    #[test]
    fn dot_parser_preserves_escape_sequences() {
        let ast =
            ast::Graph::try_from(r#"digraph { a [label="say \"hi\"", tooltip="path\\here"]; }"#)
                .unwrap();
        let graph = canonical::Graph::from(ast);

        let (_, node) = graph.nodes.set.iter().find(|(id, _)| *id == "a").unwrap();
        let vals: Vec<(String, String)> = node
            .attr
            .elems
            .iter()
            .map(|(k, v)| {
                let k: String = k.clone().into();
                let v: String = v.clone().into();
                (k, v)
            })
            .collect();

        // Escaped quotes: parser gives us the raw escape sequence, NOT unescaped.
        let label_val = &vals.iter().find(|(k, _)| k == "label").unwrap().1;
        assert_eq!(
            label_val, r#"say \"hi\""#,
            "dot-parser preserves \\\" as-is (does not unescape)"
        );

        // Escaped backslash: parser gives us the raw escape sequence.
        let tooltip_val = &vals.iter().find(|(k, _)| k == "tooltip").unwrap().1;
        assert_eq!(
            tooltip_val, r"path\\here",
            "dot-parser preserves \\\\ as-is (does not unescape)"
        );
    }

    #[test]
    fn unquote_strips_outer_quotes() {
        assert_eq!(unquote(r#""hello""#), "hello");
    }

    #[test]
    fn unquote_bare_id_unchanged() {
        assert_eq!(unquote("hello"), "hello");
    }

    #[test]
    fn unquote_escaped_quote_with_outer_quotes() {
        // Node ID case: outer quotes present + escape sequences.
        assert_eq!(unquote(r#""say \"hi\"""#), r#"say "hi""#);
    }

    #[test]
    fn unquote_escaped_quote_without_outer_quotes() {
        // Attribute value case: dot-parser already stripped outer quotes,
        // but escape sequences remain. Must still unescape.
        assert_eq!(unquote(r#"say \"hi\""#), r#"say "hi""#);
    }

    #[test]
    fn unquote_backslash_preserved() {
        // Backslashes are preserved verbatim (DOT formatting directives).
        assert_eq!(unquote(r#""a\\b""#), r"a\\b");
        assert_eq!(unquote(r"a\\b"), r"a\\b");
    }

    #[test]
    fn unquote_formatting_directives_preserved() {
        // DOT \n, \l, \r are label formatting directives and must survive.
        assert_eq!(unquote(r#""line1\nline2""#), r"line1\nline2");
        assert_eq!(unquote(r"line1\nline2"), r"line1\nline2");
        assert_eq!(unquote(r"line1\lline2"), r"line1\lline2");
        assert_eq!(unquote(r"line1\rline2"), r"line1\rline2");
    }

    #[test]
    fn unquote_escaped_backslash_before_quote() {
        // DOT \\\" = escaped backslash + escaped quote.
        // We preserve \\ but decode \" → ", so \\\" → \\"
        assert_eq!(unquote(r#""a\\\"b""#), r#"a\\"b"#);
        assert_eq!(unquote(r#"a\\\"b"#), r#"a\\"b"#);
    }

    #[test]
    fn empty_digraph() {
        let graph = parse("digraph {}").unwrap();
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn named_digraph() {
        let graph = parse("digraph deps {}").unwrap();
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn simple_edge() {
        let graph = parse("digraph { a -> b; }").unwrap();
        assert_eq!(graph.nodes.len(), 2);
        assert!(graph.nodes.contains_key("a"));
        assert!(graph.nodes.contains_key("b"));
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].from, "a");
        assert_eq!(graph.edges[0].to, "b");
        assert_eq!(graph.edges[0].label, None);
    }

    #[test]
    fn node_labels() {
        let graph = parse(r#"digraph { a [label="Alpha"]; b [label="Bravo"]; a -> b; }"#).unwrap();
        assert_eq!(graph.nodes["a"].label.as_deref(), Some("Alpha"));
        assert_eq!(graph.nodes["b"].label.as_deref(), Some("Bravo"));
    }

    #[test]
    fn edge_labels() {
        let graph =
            parse(r#"digraph { a -> b [label="depends"]; a -> c [label="uses"]; }"#).unwrap();
        assert_eq!(graph.edges.len(), 2);
        assert_eq!(graph.edges[0].label.as_deref(), Some("depends"));
        assert_eq!(graph.edges[1].label.as_deref(), Some("uses"));
    }

    #[test]
    fn type_attr_in_attrs() {
        let graph = parse(r#"digraph { a [type="lib"]; }"#).unwrap();
        assert_eq!(
            graph.nodes["a"].attrs.get("type").map(|s| s.as_str()),
            Some("lib")
        );
    }

    #[test]
    fn shape_attr_in_attrs() {
        let graph = parse(r#"digraph { a [shape=box]; }"#).unwrap();
        assert_eq!(
            graph.nodes["a"].attrs.get("shape").map(|s| s.as_str()),
            Some("box")
        );
    }

    #[test]
    fn type_and_shape_coexist_in_attrs() {
        let graph = parse(r#"digraph { a [shape=box, type="lib"]; }"#).unwrap();
        assert_eq!(
            graph.nodes["a"].attrs.get("type").map(|s| s.as_str()),
            Some("lib")
        );
        assert_eq!(
            graph.nodes["a"].attrs.get("shape").map(|s| s.as_str()),
            Some("box")
        );
    }

    #[test]
    fn extra_attrs_preserved() {
        let graph = parse(r#"digraph { a [label="A", color="red", style="bold"]; }"#).unwrap();
        assert_eq!(graph.nodes["a"].label.as_deref(), Some("A"));
        assert_eq!(
            graph.nodes["a"].attrs.get("color").map(|s| s.as_str()),
            Some("red")
        );
        assert_eq!(
            graph.nodes["a"].attrs.get("style").map(|s| s.as_str()),
            Some("bold")
        );
    }

    #[test]
    fn implicit_nodes_from_edges() {
        // Nodes only defined implicitly by edges should still appear in the graph.
        let graph = parse("digraph { a -> b -> c; }").unwrap();
        assert_eq!(graph.nodes.len(), 3);
        assert!(graph.nodes.contains_key("a"));
        assert!(graph.nodes.contains_key("b"));
        assert!(graph.nodes.contains_key("c"));
    }

    #[test]
    fn graph_attrs_captured() {
        let graph = parse(r#"digraph { rankdir=LR; a -> b; }"#).unwrap();
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.attrs.get("rankdir").map(|s| s.as_str()), Some("LR"));
    }

    #[test]
    fn graph_name_captured() {
        let graph = parse("digraph deps { a -> b; }").unwrap();
        assert_eq!(graph.attrs.get("name").map(|s| s.as_str()), Some("deps"));
    }

    #[test]
    fn quoted_ids() {
        let graph =
            parse(r#"digraph { "my node" [label="My Node"]; "my node" -> "other"; }"#).unwrap();
        assert!(graph.nodes.contains_key("my node"));
        assert_eq!(graph.nodes["my node"].label.as_deref(), Some("My Node"));
    }

    #[test]
    fn edge_attrs_captured() {
        let graph = parse(r#"digraph { a -> b [style="dashed", color="red"]; }"#).unwrap();
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(
            graph.edges[0].attrs.get("style").map(|s| s.as_str()),
            Some("dashed")
        );
        assert_eq!(
            graph.edges[0].attrs.get("color").map(|s| s.as_str()),
            Some("red")
        );
    }

    #[test]
    fn edge_label_and_attrs() {
        let graph = parse(r#"digraph { a -> b [label="uses", style="bold"]; }"#).unwrap();
        assert_eq!(graph.edges[0].label.as_deref(), Some("uses"));
        assert_eq!(
            graph.edges[0].attrs.get("style").map(|s| s.as_str()),
            Some("bold")
        );
    }

    #[test]
    fn escaped_quotes_in_label() {
        // This exercises the bug path: dot-parser strips outer quotes from
        // attribute values but preserves \" escape sequences. Our unquote
        // must decode them so they don't get double-escaped by quote().
        let graph = parse(r#"digraph { a [label="say \"hi\""]; }"#).unwrap();
        assert_eq!(graph.nodes["a"].label.as_deref(), Some(r#"say "hi""#));
    }

    #[test]
    fn escaped_quotes_in_label_roundtrip() {
        // Full parse→emit round-trip with escaped quotes.
        let input = r#"digraph { a [label="say \"hi\""]; }"#;
        let graph = parse(input).unwrap();
        let mut buf = Vec::new();
        crate::emit::dot::emit(&graph, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "digraph {\n    a [label=\"say \\\"hi\\\"\"];\n}\n");
        // And parse the output again to verify it's valid.
        let graph2 = parse(&output).unwrap();
        assert_eq!(graph2.nodes["a"].label.as_deref(), Some(r#"say "hi""#));
    }

    #[test]
    fn fixture_small_dot() {
        let input = include_str!("../../../../data/depconv/small.dot");
        let graph = parse(input).unwrap();
        assert_eq!(graph.nodes.len(), 3);
        assert!(graph.nodes.contains_key("myapp"));
        assert!(graph.nodes.contains_key("libfoo"));
        assert!(graph.nodes.contains_key("libbar"));
        assert_eq!(
            graph.nodes["myapp"].label.as_deref(),
            Some("My Application")
        );
        // shape=box stored in attrs
        assert_eq!(
            graph.nodes["myapp"].attrs.get("shape").map(|s| s.as_str()),
            Some("box")
        );
        assert_eq!(graph.edges.len(), 3);
        // Graph name and rankdir captured
        assert_eq!(graph.attrs.get("name").map(|s| s.as_str()), Some("deps"));
        assert_eq!(graph.attrs.get("rankdir").map(|s| s.as_str()), Some("LR"));
    }
}
