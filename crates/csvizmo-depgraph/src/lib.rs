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
    /// Graph or subgraph identifier (e.g. DOT `digraph <id>` / `subgraph <id>`).
    pub id: Option<String>,
    /// Graph-level attributes (e.g. DOT `rankdir`, `label`, `color`).
    pub attrs: IndexMap<String, String>,
    pub nodes: IndexMap<String, NodeInfo>,
    pub edges: Vec<Edge>,
    /// Nested subgraphs, each owning its own nodes and edges.
    pub subgraphs: Vec<DepGraph>,
}

impl DepGraph {
    /// Collect all nodes from this graph and all nested subgraphs.
    ///
    /// This function recurses over subgraphs to aggregate the results. If you're doing repeated
    /// lookups, consider caching the results.
    pub fn all_nodes(&self) -> IndexMap<&str, &NodeInfo> {
        let mut result = IndexMap::new();
        // Recurse over the subgraphs in DFS order to collect nodes from each
        self.collect_nodes(&mut result);
        result
    }

    fn collect_nodes<'a>(&'a self, result: &mut IndexMap<&'a str, &'a NodeInfo>) {
        for (id, info) in &self.nodes {
            result.insert(id.as_str(), info);
        }
        for sg in &self.subgraphs {
            sg.collect_nodes(result);
        }
    }

    /// Collect all edges from this graph and all nested subgraphs.
    ///
    /// This function recurses over subgraphs to aggregate the results. If you're doing repeated
    /// lookups, consider caching the results.
    pub fn all_edges(&self) -> Vec<&Edge> {
        let mut result = Vec::new();
        // Recurse over the subgraphs in DFS order to collect edges from each
        self.collect_edges(&mut result);
        result
    }

    fn collect_edges<'a>(&'a self, result: &mut Vec<&'a Edge>) {
        result.extend(&self.edges);
        for sg in &self.subgraphs {
            sg.collect_edges(result);
        }
    }

    /// Build an adjacency list from all edges across all subgraphs.
    ///
    /// This function recurses over subgraphs to aggregate the results. If you're doing repeated
    /// lookups, consider caching the results.
    pub fn adjacency_list(&self) -> IndexMap<&str, Vec<&str>> {
        let mut adj = IndexMap::new();
        for edge in self.all_edges() {
            adj.entry(edge.from.as_str())
                .or_insert_with(Vec::new)
                .push(edge.to.as_str());
        }
        adj
    }
}

/// Normalize a node type string to a canonical form.
///
/// Converts format-specific type names to standardized equivalents:
/// - `"custom-build"` -> `"build-script"`
/// - `"rlib"`, `"cdylib"`, `"dylib"`, `"staticlib"` -> `"lib"`
/// - Already canonical types (`"proc-macro"`, `"bin"`, `"test"`, etc.) pass through
///
/// # Examples
///
/// ```
/// use csvizmo_depgraph::normalize_node_type;
/// assert_eq!(normalize_node_type("custom-build"), "build-script");
/// assert_eq!(normalize_node_type("proc-macro"), "proc-macro");
/// assert_eq!(normalize_node_type("rlib"), "lib");
/// assert_eq!(normalize_node_type("cdylib"), "lib");
/// ```
pub fn normalize_node_type(raw: &str) -> String {
    match raw {
        "custom-build" => "build-script".to_string(),
        "rlib" | "cdylib" | "dylib" | "staticlib" => "lib".to_string(),
        _ => raw.to_string(),
    }
}

#[derive(Clone, Debug, Default)]
pub struct NodeInfo {
    pub label: Option<String>,
    /// Node type/kind (e.g. "lib", "bin", "proc-macro", "build-script").
    /// Semantics are format-specific on input; normalized to canonical names where possible.
    /// Formats that don't support types leave this as None.
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
    /// Arbitrary extra attributes (e.g. DOT `style`, `color`).
    pub attrs: IndexMap<String, String>,
}
