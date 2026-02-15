mod cargo_metadata;
mod cargo_tree;
mod depfile;
#[cfg(feature = "dot")]
pub(crate) mod dot;
mod mermaid;
mod pathlist;
mod style;
mod tgf;
mod tree;

use std::fmt;
use std::path::Path;

use clap::ValueEnum;

use crate::DepGraph;

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

/// Normalize a node type string to a canonical form.
///
/// Converts format-specific type names to standardized equivalents:
/// - `"custom-build"` -> `"build-script"`
/// - `"rlib"`, `"cdylib"`, `"dylib"`, `"staticlib"` -> `"lib"`
/// - Already canonical types (`"proc-macro"`, `"bin"`, `"test"`, etc.) pass through
fn normalize_node_type(raw: &str) -> String {
    match raw {
        "custom-build" => "build-script".to_string(),
        "rlib" | "cdylib" | "dylib" | "staticlib" => "lib".to_string(),
        _ => raw.to_string(),
    }
}

/// Resolve input format using explicit flag, file extension, or content detection.
///
/// Resolution order:
/// 1. Explicit flag if provided
/// 2. File extension if path is available
/// 3. Content detection from input string
///
/// Returns an error if format cannot be determined.
pub fn resolve_input_format(
    flag: Option<InputFormat>,
    path: Option<&Path>,
    input: &str,
) -> eyre::Result<InputFormat> {
    if let Some(f) = flag {
        return Ok(f);
    }
    let ext_err = match path.map(InputFormat::try_from) {
        Some(Ok(f)) => {
            tracing::info!("Detected input format: {f:?} from file extension");
            return Ok(f);
        }
        Some(Err(e)) => Some(e),
        None => None,
    };
    if let Some(f) = crate::detect::detect(input) {
        tracing::info!("Detected input format: {f:?} from content");
        return Ok(f);
    }
    match ext_err {
        Some(e) => Err(e.wrap_err("cannot detect input format; use --input-format")),
        None => eyre::bail!("cannot detect input format; use --input-format"),
    }
}

pub fn parse(format: InputFormat, input: &str) -> eyre::Result<DepGraph> {
    let mut graph = match format {
        #[cfg(feature = "dot")]
        InputFormat::Dot => dot::parse(input),
        #[cfg(not(feature = "dot"))]
        InputFormat::Dot => eyre::bail!("'dot' feature not enabled to maintain MIT license"),
        InputFormat::Tgf => tgf::parse(input),
        InputFormat::Depfile => depfile::parse(input),
        InputFormat::Pathlist => pathlist::parse(input),
        InputFormat::Tree => tree::parse(input),
        InputFormat::CargoTree => cargo_tree::parse(input),
        InputFormat::CargoMetadata => cargo_metadata::parse(input),
        InputFormat::Mermaid => mermaid::parse(input),
    }?;

    style::apply_default_styles(&mut graph);

    Ok(graph)
}
