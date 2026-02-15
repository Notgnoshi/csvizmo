use std::io::Write;

use crate::DepGraph;

/// Replace whitespace in a node ID with underscores so it survives TGF roundtripping.
///
/// TGF uses whitespace to separate tokens, so bare whitespace inside an ID
/// would be mis-parsed on re-read. Tabs and spaces are both replaced.
fn sanitize_id(id: &str) -> String {
    if id.contains(|c: char| c.is_ascii_whitespace()) {
        id.chars()
            .map(|c| if c.is_ascii_whitespace() { '_' } else { c })
            .collect()
    } else {
        id.to_string()
    }
}

/// Emit a [`DepGraph`] as TGF (Trivial Graph Format).
///
/// Preserves node IDs, node labels, edge endpoints, and edge labels.
/// Graph-level attrs, node attrs, and edge attrs are silently dropped
/// (TGF has no syntax for them). Whitespace in node IDs is replaced
/// with underscores to ensure the output can be parsed back.
pub fn emit(graph: &DepGraph, writer: &mut dyn Write) -> eyre::Result<()> {
    for (id, info) in graph.all_nodes() {
        let id = sanitize_id(id);
        if info.label != *id {
            writeln!(writer, "{id}\t{}", info.label)?;
        } else {
            writeln!(writer, "{id}")?;
        }
    }

    writeln!(writer, "#")?;

    for edge in graph.all_edges() {
        let from = sanitize_id(&edge.from);
        let to = sanitize_id(&edge.to);
        match &edge.label {
            Some(label) => writeln!(writer, "{from}\t{to}\t{label}")?,
            None => writeln!(writer, "{from}\t{to}")?,
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;

    use super::*;
    use crate::emit::fixtures::sample_graph;
    use crate::{Edge, NodeInfo};

    #[test]
    fn emit_sample() {
        let mut buf = Vec::new();
        emit(&sample_graph(), &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(
            output,
            "a\talpha\nb\tbravo\nc\n#\na\tb\tdepends\nb\tc\na\tc\n"
        );
    }

    #[test]
    fn emit_empty() {
        let mut buf = Vec::new();
        emit(&DepGraph::default(), &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "#\n");
    }

    #[test]
    fn emit_nodes_only() {
        let mut nodes = IndexMap::new();
        nodes.insert("x".into(), NodeInfo::new("xray"));
        let graph = DepGraph {
            nodes,
            edges: vec![],
            ..Default::default()
        };
        let mut buf = Vec::new();
        emit(&graph, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "x\txray\n#\n");
    }

    #[test]
    fn rich_graph_drops_attrs() {
        let mut nodes = IndexMap::new();
        nodes.insert(
            "a".into(),
            NodeInfo {
                label: "Alpha".into(),
                node_type: None,
                attrs: IndexMap::from([
                    ("shape".into(), "box".into()),
                    ("color".into(), "red".into()),
                ]),
            },
        );
        nodes.insert("b".into(), NodeInfo::new("b"));
        let graph = DepGraph {
            attrs: IndexMap::from([
                ("name".into(), "deps".into()),
                ("rankdir".into(), "LR".into()),
            ]),
            nodes,
            edges: vec![crate::Edge {
                from: "a".into(),
                to: "b".into(),
                label: Some("uses".into()),
                attrs: IndexMap::from([("style".into(), "dashed".into())]),
            }],
            ..Default::default()
        };
        let mut buf = Vec::new();
        emit(&graph, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        // TGF only preserves IDs, labels, and edge labels -- all attrs are dropped.
        assert_eq!(output, "a\tAlpha\nb\n#\na\tb\tuses\n");
    }

    #[test]
    fn whitespace_in_ids_replaced_with_underscores() {
        let mut nodes = IndexMap::new();
        nodes.insert("my app v1.0".into(), NodeInfo::new("my app"));
        nodes.insert("lib foo v2.0".into(), NodeInfo::new("lib foo"));
        let graph = DepGraph {
            nodes,
            edges: vec![Edge {
                from: "my app v1.0".into(),
                to: "lib foo v2.0".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut buf = Vec::new();
        emit(&graph, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(
            output,
            "my_app_v1.0\tmy app\nlib_foo_v2.0\tlib foo\n#\nmy_app_v1.0\tlib_foo_v2.0\n"
        );
    }

    #[test]
    fn subgraph_nodes_and_edges_included() {
        let graph = DepGraph {
            nodes: IndexMap::from([("top".into(), NodeInfo::new("top"))]),
            edges: vec![Edge {
                from: "top".into(),
                to: "a".into(),
                ..Default::default()
            }],
            subgraphs: vec![DepGraph {
                nodes: IndexMap::from([
                    ("a".into(), NodeInfo::new("Alpha")),
                    ("b".into(), NodeInfo::new("b")),
                ]),
                edges: vec![Edge {
                    from: "a".into(),
                    to: "b".into(),
                    label: Some("uses".into()),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut buf = Vec::new();
        emit(&graph, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "top\na\tAlpha\nb\n#\ntop\ta\na\tb\tuses\n");
    }
}
