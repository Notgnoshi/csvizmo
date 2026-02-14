use std::io::Write;

use crate::DepGraph;

/// Emit a [`DepGraph`] as TGF (Trivial Graph Format).
///
/// Preserves node IDs, node labels, edge endpoints, and edge labels.
/// Graph-level attrs, node attrs, and edge attrs are silently dropped
/// (TGF has no syntax for them).
pub fn emit(graph: &DepGraph, writer: &mut dyn Write) -> eyre::Result<()> {
    for (id, info) in graph.all_nodes() {
        if info.label != id {
            writeln!(writer, "{id}\t{}", info.label)?;
        } else {
            writeln!(writer, "{id}")?;
        }
    }

    writeln!(writer, "#")?;

    for edge in graph.all_edges() {
        match &edge.label {
            Some(label) => writeln!(writer, "{}\t{}\t{label}", edge.from, edge.to)?,
            None => writeln!(writer, "{}\t{}", edge.from, edge.to)?,
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
