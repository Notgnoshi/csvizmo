mod depfile;
pub(crate) mod dot;
mod mermaid;
mod pathlist;
mod tgf;
mod tree;
mod walk;

use std::io::Write;
use std::path::Path;

use clap::ValueEnum;

use crate::DepGraph;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum OutputFormat {
    Dot,
    Mermaid,
    Tgf,
    Depfile,
    Tree,
    Pathlist,
}

impl TryFrom<&Path> for OutputFormat {
    type Error = eyre::Report;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| eyre::eyre!("no file extension: {}", path.display()))?;
        match ext {
            "dot" | "gv" => Ok(Self::Dot),
            "mmd" | "mermaid" => Ok(Self::Mermaid),
            "tgf" => Ok(Self::Tgf),
            "d" => Ok(Self::Depfile),
            _ => eyre::bail!("unrecognized dependency graph file extension: .{ext}"),
        }
    }
}

/// Resolve output format using explicit flag, file extension, or default to DOT.
///
/// Resolution order:
/// 1. Explicit flag if provided
/// 2. File extension if path is available
/// 3. Default to DOT format
///
/// Returns an error if file extension is present but unrecognized.
pub fn resolve_output_format(
    flag: Option<OutputFormat>,
    path: Option<&Path>,
) -> eyre::Result<OutputFormat> {
    if let Some(f) = flag {
        return Ok(f);
    }
    match path.map(OutputFormat::try_from) {
        Some(Ok(f)) => {
            tracing::info!("Detected output format: {f:?} from file extension");
            Ok(f)
        }
        Some(Err(e)) => Err(
            e.wrap_err("Failed to detect output format from file extension; use --output-format")
        ),
        None => Ok(OutputFormat::Dot),
    }
}

/// Emit a [`DepGraph`] in the given output format.
///
/// Not every format can represent all graph features. The table below
/// summarises what each emitter preserves:
///
/// | Format   | Graph attrs | Node label | Node attrs | Edge label | Edge attrs |
/// |----------|-------------|------------|------------|------------|------------|
/// | DOT      | yes         | yes        | yes        | yes        | yes        |
/// | Mermaid  | direction   | yes        | shapes     | yes        | dropped    |
/// | TGF      | dropped     | yes        | dropped    | yes        | dropped    |
/// | Tree     | dropped     | yes        | dropped    | dropped    | dropped    |
/// | Pathlist | dropped     | yes        | dropped    | dropped    | dropped    |
/// | Depfile  | dropped     | dropped    | dropped    | dropped    | dropped    |
///
/// Features marked "dropped" are silently discarded. Converting from a
/// rich format (e.g. DOT) to a lossy one (e.g. Depfile) is intentionally
/// non-destructive to the source data -- the information simply isn't
/// written to the output.
pub fn emit(format: OutputFormat, graph: &DepGraph, writer: &mut dyn Write) -> eyre::Result<()> {
    match format {
        OutputFormat::Dot => dot::emit(graph, writer),
        OutputFormat::Mermaid => mermaid::emit(graph, writer),
        OutputFormat::Tgf => tgf::emit(graph, writer),
        OutputFormat::Depfile => depfile::emit(graph, writer),
        OutputFormat::Pathlist => pathlist::emit(graph, writer),
        OutputFormat::Tree => tree::emit(graph, writer),
    }
}

#[cfg(test)]
pub(crate) mod fixtures {
    use indexmap::IndexMap;

    use crate::{DepGraph, Edge, NodeInfo};

    /// A small graph for testing: a -> b -> c, a -> c
    pub fn sample_graph() -> DepGraph {
        let mut nodes = IndexMap::new();
        nodes.insert("a".into(), NodeInfo::new("alpha"));
        nodes.insert("b".into(), NodeInfo::new("bravo"));
        nodes.insert("c".into(), NodeInfo::new("c"));

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
