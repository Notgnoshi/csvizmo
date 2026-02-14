mod cargo_metadata;
mod cargo_tree;
mod depfile;
#[cfg(feature = "dot")]
pub(crate) mod dot;
mod mermaid;
mod pathlist;
mod tgf;
mod tree;

use crate::{DepGraph, InputFormat};

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

    crate::style::apply_default_styles(&mut graph);

    Ok(graph)
}
