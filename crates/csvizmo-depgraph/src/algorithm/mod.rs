pub mod between;
pub mod cycles;
pub mod filter;
pub mod reverse;
pub mod select;
pub mod shorten;
pub mod simplify;

use globset::{Glob, GlobSet, GlobSetBuilder};

#[derive(Debug, Default, Clone, Copy, clap::ValueEnum)]
pub enum MatchKey {
    Id,
    #[default]
    Label,
}

impl std::fmt::Display for MatchKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use clap::ValueEnum;

        f.write_str(self.to_possible_value().unwrap().get_name())
    }
}

fn build_globset(patterns: &[String]) -> eyre::Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern)?);
    }
    Ok(builder.build()?)
}
