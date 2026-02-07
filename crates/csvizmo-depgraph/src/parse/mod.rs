#[cfg(feature = "dot")]
pub(crate) mod dot;
mod tgf;

use crate::{DepGraph, InputFormat};

pub fn parse(format: InputFormat, input: &str) -> eyre::Result<DepGraph> {
    match format {
        #[cfg(feature = "dot")]
        InputFormat::Dot => dot::parse(input),
        #[cfg(not(feature = "dot"))]
        InputFormat::Dot => eyre::bail!("'dot' feature not enabled to maintain MIT license"),
        InputFormat::Tgf => tgf::parse(input),
        _ => eyre::bail!("{format:?} parsing not yet implemented"),
    }
}
