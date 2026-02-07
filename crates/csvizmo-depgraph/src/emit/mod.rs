mod depfile;
pub(crate) mod dot;
mod tgf;

use std::io::Write;

use crate::{DepGraph, OutputFormat};

pub fn emit(format: OutputFormat, graph: &DepGraph, writer: &mut dyn Write) -> eyre::Result<()> {
    match format {
        OutputFormat::Dot => dot::emit(graph, writer),
        OutputFormat::Tgf => tgf::emit(graph, writer),
        OutputFormat::Depfile => depfile::emit(graph, writer),
        _ => eyre::bail!("{format:?} emitting not yet implemented"),
    }
}

#[cfg(test)]
pub(crate) mod fixtures {
    use indexmap::IndexMap;

    use crate::{DepGraph, Edge, NodeInfo};

    /// A small graph for testing: a -> b -> c, a -> c
    pub fn sample_graph() -> DepGraph {
        let mut nodes = IndexMap::new();
        nodes.insert(
            "a".into(),
            NodeInfo {
                label: Some("alpha".into()),
                ..Default::default()
            },
        );
        nodes.insert(
            "b".into(),
            NodeInfo {
                label: Some("bravo".into()),
                ..Default::default()
            },
        );
        nodes.insert("c".into(), NodeInfo::default());

        DepGraph {
            nodes,
            edges: vec![
                Edge {
                    from: "a".into(),
                    to: "b".into(),
                    label: Some("depends".into()),
                    ..Default::default()
                },
                Edge {
                    from: "b".into(),
                    to: "c".into(),
                    ..Default::default()
                },
                Edge {
                    from: "a".into(),
                    to: "c".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }
}
