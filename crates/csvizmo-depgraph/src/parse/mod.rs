mod tgf;

use crate::{DepGraph, InputFormat};

pub fn parse(format: InputFormat, input: &str) -> eyre::Result<DepGraph> {
    match format {
        InputFormat::Tgf => tgf::parse(input),
        _ => eyre::bail!("{format:?} parsing not yet implemented"),
    }
}
