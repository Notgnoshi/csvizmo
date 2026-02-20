use std::collections::HashSet;
use std::path::PathBuf;

use csvizmo_minpath::PathTransforms;
use indexmap::IndexMap;

use crate::{DepGraph, Edge, NodeInfo};

/// Which fields to shorten.
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum ShortenKey {
    Id,
    Label,
    #[default]
    Both,
}

impl std::fmt::Display for ShortenKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use clap::ValueEnum;

        f.write_str(self.to_possible_value().unwrap().get_name())
    }
}

/// Shorten node IDs and/or labels using minpath transforms.
pub fn shorten(
    graph: &DepGraph,
    separator: &str,
    key: ShortenKey,
    transforms: &PathTransforms,
) -> DepGraph {
    let all_nodes = graph.all_nodes();

    let shorten_ids = matches!(key, ShortenKey::Id | ShortenKey::Both);
    let shorten_labels = matches!(key, ShortenKey::Label | ShortenKey::Both);

    // Build ID mapping.
    let id_map: IndexMap<String, String> = if shorten_ids {
        build_mapping(all_nodes.keys().map(|s| s.as_str()), separator, transforms)
    } else {
        IndexMap::new()
    };

    // Build label mapping.
    let label_map: IndexMap<String, String> = if shorten_labels {
        let labels: Vec<&str> = all_nodes.values().map(|n| n.label.as_str()).collect();
        build_mapping(labels.into_iter(), separator, transforms)
    } else {
        IndexMap::new()
    };

    remap_graph(graph, &id_map, &label_map)
}

/// Convert strings to path form, apply minpath transforms, convert back.
fn build_mapping<'a>(
    values: impl Iterator<Item = &'a str>,
    separator: &str,
    transforms: &PathTransforms,
) -> IndexMap<String, String> {
    let originals: Vec<&str> = values.collect();

    // Convert to path form by replacing separator with '/'.
    let paths: Vec<String> = originals
        .iter()
        .map(|s| s.replace(separator, "/"))
        .collect();

    let shortened = transforms.build(&paths);

    let mut mapping = IndexMap::new();
    for (i, original) in originals.iter().enumerate() {
        let short = shortened
            .shorten(&paths[i])
            .to_string_lossy()
            .replace('/', separator);
        if short != *original {
            mapping.insert(original.to_string(), short);
        }
    }
    mapping
}

/// Build a [`PathTransforms`] from CLI arguments.
///
/// If no transform flags are set, defaults to `strip_common_prefix + minimal_unique_suffix`.
pub fn build_transforms(args: &ShortenArgs) -> PathTransforms {
    let any_set = args.home_dir
        || args.resolve_relative
        || args.relative_to.is_some()
        || !args.strip_prefix.is_empty()
        || args.smart_abbreviate
        || args.strip_common_prefix
        || args.minimal_unique_suffix
        || args.single_letter;

    if any_set {
        PathTransforms::new()
            .strip_prefix(args.strip_prefix.clone())
            .home_dir(args.home_dir)
            .resolve_relative(args.resolve_relative)
            .relative_to(args.relative_to.as_ref())
            .smart_abbreviate(args.smart_abbreviate)
            .strip_common_prefix(args.strip_common_prefix)
            .minimal_unique_suffix(args.minimal_unique_suffix)
            .single_letter(args.single_letter)
    } else {
        PathTransforms::new()
            .strip_common_prefix(true)
            .minimal_unique_suffix(true)
    }
}

/// CLI arguments for the shorten subcommand.
#[derive(Clone, Debug, Default, clap::Parser)]
pub struct ShortenArgs {
    /// Character used to split node IDs into path components
    #[clap(long, default_value = "/")]
    pub separator: String,

    /// Which fields to shorten: id, label, or both
    #[clap(long, default_value_t = ShortenKey::default())]
    pub key: ShortenKey,

    /// Replace `/home/$USER` with `~`
    #[clap(long)]
    pub home_dir: bool,

    /// Normalize . and .. path components
    #[clap(long)]
    pub resolve_relative: bool,

    /// Make paths relative to a base path
    #[clap(long)]
    pub relative_to: Option<PathBuf>,

    /// Remove the given prefix (can be repeated)
    #[clap(long)]
    pub strip_prefix: Vec<PathBuf>,

    /// Abbreviate common directory names (Documents -> docs, etc.)
    #[clap(long)]
    pub smart_abbreviate: bool,

    /// Remove the prefix shared by all paths
    #[clap(long)]
    pub strip_common_prefix: bool,

    /// Shorten to the minimal unique suffix
    #[clap(long)]
    pub minimal_unique_suffix: bool,

    /// Abbreviate directory components to single letters
    #[clap(long)]
    pub single_letter: bool,
}

fn remap_graph(
    graph: &DepGraph,
    id_map: &IndexMap<String, String>,
    label_map: &IndexMap<String, String>,
) -> DepGraph {
    let mut placed = HashSet::new();
    remap_inner(graph, id_map, label_map, &mut placed)
}

fn remap_inner(
    graph: &DepGraph,
    id_map: &IndexMap<String, String>,
    label_map: &IndexMap<String, String>,
    placed: &mut HashSet<String>,
) -> DepGraph {
    let nodes: IndexMap<String, NodeInfo> = graph
        .nodes
        .iter()
        .filter_map(|(id, info)| {
            let new_id = id_map.get(id).unwrap_or(id).clone();
            if !placed.insert(new_id.clone()) {
                return None; // already placed (ID collision from shortening)
            }
            let mut info = info.clone();
            if let Some(new_label) = label_map.get(&info.label) {
                info.label = new_label.clone();
            }
            Some((new_id, info))
        })
        .collect();

    let subgraphs: Vec<DepGraph> = graph
        .subgraphs
        .iter()
        .map(|sg| remap_inner(sg, id_map, label_map, placed))
        .filter(|sg| !sg.nodes.is_empty() || !sg.subgraphs.is_empty())
        .collect();

    let mut seen_edges = HashSet::new();
    let edges: Vec<Edge> = graph
        .edges
        .iter()
        .filter_map(|e| {
            let from = id_map.get(&e.from).unwrap_or(&e.from).clone();
            let to = id_map.get(&e.to).unwrap_or(&e.to).clone();
            if from == to {
                return None; // self-loop from collision
            }
            let key = (from.clone(), to.clone(), e.label.clone());
            if !seen_edges.insert(key) {
                return None; // duplicate
            }
            Some(Edge {
                from,
                to,
                label: e.label.clone(),
                attrs: e.attrs.clone(),
            })
        })
        .collect();

    DepGraph {
        id: graph.id.clone(),
        attrs: graph.attrs.clone(),
        nodes,
        edges,
        subgraphs,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn node_ids(graph: &DepGraph) -> Vec<&str> {
        graph.nodes.keys().map(|s| s.as_str()).collect()
    }

    fn node_labels(graph: &DepGraph) -> Vec<&str> {
        graph.nodes.values().map(|n| n.label.as_str()).collect()
    }

    fn default_transforms() -> PathTransforms {
        PathTransforms::new()
            .strip_common_prefix(true)
            .minimal_unique_suffix(true)
    }

    #[test]
    fn shorten_strips_common_prefix() {
        let g = make_graph(
            &[
                ("src/foo/bar.rs", "src/foo/bar.rs"),
                ("src/foo/baz.rs", "src/foo/baz.rs"),
            ],
            &[("src/foo/bar.rs", "src/foo/baz.rs")],
        );
        let result = shorten(&g, "/", ShortenKey::Both, &default_transforms());
        assert_eq!(node_ids(&result), vec!["bar.rs", "baz.rs"]);
        assert_eq!(node_labels(&result), vec!["bar.rs", "baz.rs"]);
    }

    #[test]
    fn shorten_with_dot_separator() {
        let g = make_graph(
            &[
                ("com.example.foo", "com.example.foo"),
                ("com.example.bar", "com.example.bar"),
            ],
            &[],
        );
        let result = shorten(&g, ".", ShortenKey::Both, &default_transforms());
        assert_eq!(node_ids(&result), vec!["foo", "bar"]);
    }

    #[test]
    fn shorten_id_only() {
        let g = make_graph(
            &[
                ("src/foo/bar.rs", "Original Label 1"),
                ("src/foo/baz.rs", "Original Label 2"),
            ],
            &[],
        );
        let result = shorten(&g, "/", ShortenKey::Id, &default_transforms());
        assert_eq!(node_ids(&result), vec!["bar.rs", "baz.rs"]);
        // Labels unchanged
        assert_eq!(
            node_labels(&result),
            vec!["Original Label 1", "Original Label 2"]
        );
    }

    #[test]
    fn shorten_label_only() {
        let g = make_graph(&[("id1", "src/foo/bar.rs"), ("id2", "src/foo/baz.rs")], &[]);
        let result = shorten(&g, "/", ShortenKey::Label, &default_transforms());
        // IDs unchanged
        assert_eq!(node_ids(&result), vec!["id1", "id2"]);
        assert_eq!(node_labels(&result), vec!["bar.rs", "baz.rs"]);
    }

    #[test]
    fn shorten_updates_edge_endpoints() {
        let g = make_graph(
            &[("src/foo/bar.rs", "bar"), ("src/foo/baz.rs", "baz")],
            &[("src/foo/bar.rs", "src/foo/baz.rs")],
        );
        let result = shorten(&g, "/", ShortenKey::Id, &default_transforms());
        assert_eq!(result.edges[0].from, "bar.rs");
        assert_eq!(result.edges[0].to, "baz.rs");
    }

    #[test]
    fn shorten_empty_graph() {
        let g = DepGraph::default();
        let result = shorten(&g, "/", ShortenKey::Both, &default_transforms());
        assert!(result.nodes.is_empty());
    }

    #[test]
    fn build_transforms_defaults() {
        // No flags set -> strip_common_prefix + minimal_unique_suffix
        let args = ShortenArgs::default();
        let transforms = build_transforms(&args);
        let g = make_graph(
            &[
                ("src/foo/bar.rs", "src/foo/bar.rs"),
                ("src/foo/baz.rs", "src/foo/baz.rs"),
            ],
            &[],
        );
        let result = shorten(&g, "/", ShortenKey::Id, &transforms);
        assert_eq!(node_ids(&result), vec!["bar.rs", "baz.rs"]);
    }

    #[test]
    fn build_transforms_explicit_single_letter() {
        let args = ShortenArgs {
            single_letter: true,
            ..Default::default()
        };
        let transforms = build_transforms(&args);
        // single_letter only, no strip_common_prefix or minimal_unique_suffix
        let g = make_graph(&[("src/foo/bar.rs", "src/foo/bar.rs")], &[]);
        let result = shorten(&g, "/", ShortenKey::Id, &transforms);
        assert_eq!(node_ids(&result), vec!["s/f/bar.rs"]);
    }

    fn colliding_transforms() -> PathTransforms {
        PathTransforms::new().single_letter(true)
    }

    #[test]
    fn collision_removes_self_loops() {
        // src/utils/parse.rs -> s/u/parse.rs
        // src/uber/parse.rs  -> s/u/parse.rs  (collision)
        // Edge between them becomes a self-loop and should be removed.
        let g = make_graph(
            &[
                ("src/utils/parse.rs", "src/utils/parse.rs"),
                ("src/uber/parse.rs", "src/uber/parse.rs"),
            ],
            &[("src/utils/parse.rs", "src/uber/parse.rs")],
        );
        let result = shorten(&g, "/", ShortenKey::Id, &colliding_transforms());
        assert_eq!(node_ids(&result), vec!["s/u/parse.rs"]);
        assert!(result.edges.is_empty());
    }

    #[test]
    fn collision_deduplicates_edges() {
        // Both a.x and a.y collide to the same ID, both have edges to b.
        // After remapping, only one edge should remain.
        let g = make_graph(
            &[
                ("src/utils/parse.rs", "src/utils/parse.rs"),
                ("src/uber/parse.rs", "src/uber/parse.rs"),
                ("lib/out.rs", "lib/out.rs"),
            ],
            &[
                ("src/utils/parse.rs", "lib/out.rs"),
                ("src/uber/parse.rs", "lib/out.rs"),
            ],
        );
        let result = shorten(&g, "/", ShortenKey::Id, &colliding_transforms());
        assert_eq!(node_ids(&result), vec!["s/u/parse.rs", "l/out.rs"]);
        assert_eq!(result.edges.len(), 1);
        assert_eq!(result.edges[0].from, "s/u/parse.rs");
        assert_eq!(result.edges[0].to, "l/out.rs");
    }
}
