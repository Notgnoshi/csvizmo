use clap::Parser;

use super::{MatchKey, build_globset};
use crate::DepGraph;

#[derive(Clone, Debug, Default, Parser)]
pub struct SelectArgs {
    /// Glob pattern to select nodes (can be repeated)
    #[clap(short, long)]
    pub pattern: Vec<String>,

    /// Combine multiple patterns with AND instead of OR
    #[clap(long)]
    pub and: bool,

    /// Match patterns against 'id' or 'label'
    #[clap(long, default_value_t = MatchKey::default())]
    pub key: MatchKey,

    /// Include all dependencies of selected nodes
    #[clap(long)]
    pub deps: bool,

    /// Include all ancestors of selected nodes
    #[clap(long)]
    pub ancestors: bool,

    /// Traverse up to N layers (implies --deps if no direction given)
    #[clap(long)]
    pub depth: Option<usize>,
}

impl SelectArgs {
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

    pub fn depth(mut self, n: usize) -> Self {
        self.depth = Some(n);
        self
    }
}

pub fn select(graph: &DepGraph, args: &SelectArgs) -> eyre::Result<DepGraph> {
    let _globset = build_globset(&args.pattern)?;
    // TODO: implement select logic
    Ok(graph.clone())
}
