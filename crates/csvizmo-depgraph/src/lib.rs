use std::fmt;
use std::path::Path;

use clap::ValueEnum;
use indexmap::IndexMap;

pub mod detect;
pub mod emit;
pub mod parse;

/// Variant order defines content-detection priority (most specific first).
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum InputFormat {
    CargoMetadata,
    Mermaid,
    Dot,
    Tgf,
    Depfile,
    CargoTree,
    Tree,
    Pathlist,
}

impl fmt::Display for InputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.to_possible_value().unwrap().get_name())
    }
}

impl TryFrom<&Path> for InputFormat {
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
            "json" => Ok(Self::CargoMetadata),
            _ => eyre::bail!("unrecognized dependency graph file extension: .{ext}"),
        }
    }
}

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

#[derive(Clone, Debug, Default)]
pub struct DepGraph {
    pub nodes: IndexMap<String, NodeInfo>,
    pub edges: Vec<Edge>,
}

#[derive(Clone, Debug, Default)]
pub struct NodeInfo {
    pub label: Option<String>,
    /// Node type (e.g. "lib", "bin", "proc-macro", "build").
    /// Semantics are format-dependent on input; preserved where the output format supports it.
    pub node_type: Option<String>,
    /// Arbitrary extra attributes. Parsers populate these from format-specific features;
    /// emitters carry them through where the output format allows.
    pub attrs: IndexMap<String, String>,
}

#[derive(Clone, Debug, Default)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
}
