use std::io::Write;

use crate::DepGraph;

pub fn emit(graph: &DepGraph, writer: &mut dyn Write) -> eyre::Result<()> {
    for (id, info) in &graph.nodes {
        match &info.label {
            Some(label) => writeln!(writer, "{id}\t{label}")?,
            None => writeln!(writer, "{id}")?,
        }
    }

    writeln!(writer, "#")?;

    for edge in &graph.edges {
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
    use crate::NodeInfo;
    use crate::emit::fixtures::sample_graph;

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
        nodes.insert(
            "x".into(),
            NodeInfo {
                label: Some("xray".into()),
                ..Default::default()
            },
        );
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
                label: Some("Alpha".into()),
                attrs: IndexMap::from([
                    ("shape".into(), "box".into()),
                    ("color".into(), "red".into()),
                ]),
            },
        );
        nodes.insert("b".into(), NodeInfo::default());
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
        };
        let mut buf = Vec::new();
        emit(&graph, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        // TGF only preserves IDs, labels, and edge labels — all attrs are dropped.
        assert_eq!(output, "a\tAlpha\nb\n#\na\tb\tuses\n");
    }
}
