use std::collections::HashSet;

use clap::Parser;

use super::{MatchKey, build_globset};
use crate::{DepGraph, FlatGraphView};

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
    let globset = build_globset(&args.pattern)?;
    let view = FlatGraphView::new(graph);

    let mut keep = HashSet::new();
    for (id, info) in graph.all_nodes() {
        let text = match args.key {
            MatchKey::Id => id,
            MatchKey::Label => info.label.as_str(),
        };

        let matched = if args.and {
            globset.matches(text).len() == args.pattern.len()
        } else {
            globset.is_match(text)
        };

        if matched && let Some(&idx) = view.id_to_idx.get(id) {
            keep.insert(idx);
        }
    }

    Ok(view.filter(&keep))
}
