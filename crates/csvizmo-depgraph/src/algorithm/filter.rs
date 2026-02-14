use clap::Parser;

use super::{MatchKey, build_globset};
use crate::DepGraph;

#[derive(Clone, Debug, Default, Parser)]
pub struct FilterArgs {
    /// Glob pattern to remove nodes (can be repeated)
    #[clap(short, long)]
    pub pattern: Vec<String>,

    /// Combine multiple patterns with AND instead of OR
    #[clap(long)]
    pub and: bool,

    /// Match patterns against 'id' or 'label'
    #[clap(long, default_value_t = MatchKey::default())]
    pub key: MatchKey,

    /// Also remove all dependencies of matched nodes (cascade)
    #[clap(long)]
    pub deps: bool,

    /// Also remove all ancestors of matched nodes (cascade)
    #[clap(long)]
    pub ancestors: bool,

    /// Preserve graph connectivity when removing nodes
    /// (creates direct edges, no self-loops or parallel edges)
    #[clap(long)]
    pub preserve_connectivity: bool,
}

impl FilterArgs {
    pub fn pattern(mut self, p: impl Into<String>) -> Self {
        self.pattern.push(p.into());
        self
    }

    pub fn and(mut self) -> Self {
        self.and = true;
        self
    }

    pub fn key(mut self, k: MatchKey) -> Self {
        self.key = k;
        self
    }

    pub fn deps(mut self) -> Self {
        self.deps = true;
        self
    }

    pub fn ancestors(mut self) -> Self {
        self.ancestors = true;
        self
    }

    pub fn preserve_connectivity(mut self) -> Self {
        self.preserve_connectivity = true;
        self
    }
}

pub fn filter(graph: &DepGraph, args: &FilterArgs) -> eyre::Result<DepGraph> {
    let _globset = build_globset(&args.pattern)?;
    // TODO: implement filter logic
    Ok(graph.clone())
}
