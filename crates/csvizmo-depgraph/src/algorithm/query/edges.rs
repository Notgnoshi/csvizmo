use clap::Parser;

use super::OutputFields;
use crate::DepGraph;
use crate::algorithm::{MatchKey, build_globset};

#[derive(Debug, Default, Clone, Copy, clap::ValueEnum)]
pub enum EdgeSort {
    #[default]
    None,
    Source,
    Target,
}

impl std::fmt::Display for EdgeSort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use clap::ValueEnum;

        f.write_str(self.to_possible_value().unwrap().get_name())
    }
}

#[derive(Clone, Debug, Default, Parser)]
pub struct EdgesArgs {
    /// Include edges where source OR target matches (repeatable, OR by default)
    #[clap(short = 'g', long)]
    pub include: Vec<String>,

    /// Exclude edges where source OR target matches (repeatable, OR)
    #[clap(short = 'x', long)]
    pub exclude: Vec<String>,

    /// Combine include patterns with AND instead of OR
    #[clap(long)]
    pub and: bool,

    /// What patterns match against
    #[clap(long, default_value_t = MatchKey::default())]
    pub key: MatchKey,

    /// Sort order
    #[clap(long, default_value_t = EdgeSort::None)]
    pub sort: EdgeSort,

    /// Reverse the sort order
    #[clap(short = 'r', long)]
    pub reverse: bool,

    /// Show only first N results (applied after sort)
    #[clap(short = 'n', long)]
    pub limit: Option<usize>,

    /// What to print for endpoints
    #[clap(long, default_value_t = OutputFields::Label)]
    pub format: OutputFields,
}

/// Returns (source_display, target_display, edge_label) tuples.
pub fn edges(
    graph: &DepGraph,
    args: &EdgesArgs,
) -> eyre::Result<Vec<(String, String, Option<String>)>> {
    let all_nodes = graph.all_nodes();
    let all_edges = graph.all_edges();

    let include_set = if !args.include.is_empty() {
        Some(build_globset(&args.include)?)
    } else {
        None
    };

    let exclude_set = if !args.exclude.is_empty() {
        Some(build_globset(&args.exclude)?)
    } else {
        None
    };

    let mut result: Vec<(String, String, Option<String>)> = Vec::new();

    for edge in all_edges {
        let from_info = all_nodes.get(&edge.from);
        let to_info = all_nodes.get(&edge.to);

        // Skip edges with dangling endpoints
        let (from_info, to_info) = match (from_info, to_info) {
            (Some(f), Some(t)) => (f, t),
            _ => continue,
        };

        let from_text = match args.key {
            MatchKey::Id => edge.from.as_str(),
            MatchKey::Label => from_info.label.as_str(),
        };
        let to_text = match args.key {
            MatchKey::Id => edge.to.as_str(),
            MatchKey::Label => to_info.label.as_str(),
        };

        // Include filter: edge included if source OR target matches
        if let Some(ref include) = include_set {
            let source_match = if args.and {
                include.matches(from_text).len() == args.include.len()
            } else {
                include.is_match(from_text)
            };
            let target_match = if args.and {
                include.matches(to_text).len() == args.include.len()
            } else {
                include.is_match(to_text)
            };
            if !source_match && !target_match {
                continue;
            }
        }

        // Exclude filter: edge excluded if source OR target matches
        if let Some(ref exclude) = exclude_set
            && (exclude.is_match(from_text) || exclude.is_match(to_text))
        {
            continue;
        }

        let source_display = match args.format {
            OutputFields::Id => edge.from.clone(),
            OutputFields::Label => from_info.label.clone(),
        };
        let target_display = match args.format {
            OutputFields::Id => edge.to.clone(),
            OutputFields::Label => to_info.label.clone(),
        };

        result.push((source_display, target_display, edge.label.clone()));
    }

    // Sort
    match args.sort {
        EdgeSort::None => {
            if args.reverse {
                result.reverse();
            }
        }
        EdgeSort::Source => {
            result.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
            if args.reverse {
                result.reverse();
            }
        }
        EdgeSort::Target => {
            result.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
            if args.reverse {
                result.reverse();
            }
        }
    }

    // Limit
    if let Some(limit) = args.limit {
        result.truncate(limit);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Edge, NodeInfo};

    fn make_graph(nodes: &[(&str, &str)], edges: &[(&str, &str)]) -> DepGraph {
        DepGraph {
            nodes: nodes
                .iter()
                .map(|(id, label)| (id.to_string(), NodeInfo::new(*label)))
                .collect(),
            edges: edges
                .iter()
                .map(|(from, to)| Edge {
                    from: from.to_string(),
                    to: to.to_string(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }

    fn make_graph_with_labels(
        nodes: &[(&str, &str)],
        edge_specs: &[(&str, &str, Option<&str>)],
    ) -> DepGraph {
        DepGraph {
            nodes: nodes
                .iter()
                .map(|(id, label)| (id.to_string(), NodeInfo::new(*label)))
                .collect(),
            edges: edge_specs
                .iter()
                .map(|(from, to, label)| Edge {
                    from: from.to_string(),
                    to: to.to_string(),
                    label: label.map(|l| l.to_string()),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }

    fn pairs(result: &[(String, String, Option<String>)]) -> Vec<(&str, &str)> {
        result
            .iter()
            .map(|(s, t, _)| (s.as_str(), t.as_str()))
            .collect()
    }

    #[test]
    fn all_edges_default() {
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c")],
        );
        let result = edges(&g, &EdgesArgs::default()).unwrap();
        assert_eq!(pairs(&result), vec![("A", "B"), ("B", "C")]);
    }

    #[test]
    fn include_filter() {
        let g = make_graph(
            &[("a", "alpha"), ("b", "beta"), ("c", "gamma")],
            &[("a", "b"), ("b", "c"), ("a", "c")],
        );
        let args = EdgesArgs {
            include: vec!["alpha".to_string()],
            ..Default::default()
        };
        let result = edges(&g, &args).unwrap();
        // edges where source or target is "alpha": a->b, a->c
        assert_eq!(pairs(&result), vec![("alpha", "beta"), ("alpha", "gamma")]);
    }

    #[test]
    fn exclude_filter() {
        let g = make_graph(
            &[("a", "alpha"), ("b", "beta"), ("c", "gamma")],
            &[("a", "b"), ("b", "c"), ("a", "c")],
        );
        let args = EdgesArgs {
            exclude: vec!["beta".to_string()],
            ..Default::default()
        };
        let result = edges(&g, &args).unwrap();
        // exclude edges touching beta: a->b and b->c removed, only a->c remains
        assert_eq!(pairs(&result), vec![("alpha", "gamma")]);
    }

    #[test]
    fn sort_by_source() {
        let g = make_graph(
            &[("a", "C"), ("b", "A"), ("c", "B")],
            &[("a", "b"), ("c", "a"), ("b", "c")],
        );
        let args = EdgesArgs {
            sort: EdgeSort::Source,
            ..Default::default()
        };
        let result = edges(&g, &args).unwrap();
        assert_eq!(pairs(&result), vec![("A", "B"), ("B", "C"), ("C", "A")]);
    }

    #[test]
    fn sort_by_target() {
        let g = make_graph(
            &[("a", "C"), ("b", "A"), ("c", "B")],
            &[("a", "b"), ("c", "a"), ("b", "c")],
        );
        let args = EdgesArgs {
            sort: EdgeSort::Target,
            ..Default::default()
        };
        let result = edges(&g, &args).unwrap();
        assert_eq!(pairs(&result), vec![("C", "A"), ("A", "B"), ("B", "C")]);
    }

    #[test]
    fn format_id() {
        let g = make_graph(&[("a", "Alpha"), ("b", "Beta")], &[("a", "b")]);
        let args = EdgesArgs {
            format: OutputFields::Id,
            ..Default::default()
        };
        let result = edges(&g, &args).unwrap();
        assert_eq!(pairs(&result), vec![("a", "b")]);
    }

    #[test]
    fn edge_label_included() {
        let g = make_graph_with_labels(&[("a", "A"), ("b", "B")], &[("a", "b", Some("depends"))]);
        let result = edges(&g, &EdgesArgs::default()).unwrap();
        assert_eq!(result[0].2.as_deref(), Some("depends"));
    }

    #[test]
    fn limit_edges() {
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c"), ("a", "c")],
        );
        let args = EdgesArgs {
            limit: Some(2),
            ..Default::default()
        };
        let result = edges(&g, &args).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn reverse_edges() {
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c")],
        );
        let args = EdgesArgs {
            reverse: true,
            ..Default::default()
        };
        let result = edges(&g, &args).unwrap();
        assert_eq!(pairs(&result), vec![("B", "C"), ("A", "B")]);
    }

    #[test]
    fn skips_dangling_edges() {
        let g = DepGraph {
            nodes: [("a".to_string(), NodeInfo::new("A"))]
                .into_iter()
                .collect(),
            edges: vec![Edge {
                from: "a".to_string(),
                to: "missing".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let result = edges(&g, &EdgesArgs::default()).unwrap();
        assert!(result.is_empty());
    }
}
