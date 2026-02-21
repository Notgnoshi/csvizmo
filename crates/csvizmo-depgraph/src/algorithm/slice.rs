use std::collections::HashMap;

use clap::Parser;

use crate::{DepGraph, Edge, NodeInfo};

/// Cut edges between subgraphs, isolating each subgraph.
#[derive(Clone, Debug, Default, Parser)]
pub struct SliceArgs {
    /// Remove nodes that are not inside any subgraph.
    #[clap(long)]
    pub drop_orphans: bool,

    /// Also slice within nested subgraphs recursively.
    #[clap(short, long)]
    pub recursive: bool,
}

/// Cut edges that cross subgraph boundaries, isolating each subgraph into a
/// disconnected component.
///
/// In the default (top-level) mode, every node is assigned to the top-level
/// subgraph it belongs to (even if nested deeper). Edges are kept only when
/// both endpoints share the same top-level group. With `--drop-orphans`,
/// root-level nodes (not inside any subgraph) and their edges are also removed.
///
/// In `--recursive` mode, the same logic is applied independently at each level
/// of the subgraph hierarchy: edges at a given level are kept only when both
/// endpoints belong to the same immediate child subgraph (or are both
/// root-level at that scope).
pub fn slice(graph: &DepGraph, args: &SliceArgs) -> eyre::Result<DepGraph> {
    if args.recursive {
        Ok(slice_recursive(graph, args.drop_orphans))
    } else {
        Ok(slice_toplevel(graph, args.drop_orphans))
    }
}

/// Recursively assign all nodes in `sg` (and its nested subgraphs) to `group`.
fn assign_group(sg: &DepGraph, map: &mut HashMap<String, usize>, group: usize) {
    for id in sg.nodes.keys() {
        map.insert(id.clone(), group);
    }
    for child in &sg.subgraphs {
        assign_group(child, map, group);
    }
}

/// Check if an edge should be kept based on the group map.
///
/// An edge is kept when both endpoints belong to the same group. Endpoints not
/// present in the map are considered root-level; two root-level endpoints are
/// kept unless `drop_orphans` is true.
fn edge_allowed(edge: &Edge, group_map: &HashMap<String, usize>, drop_orphans: bool) -> bool {
    match (group_map.get(&edge.from), group_map.get(&edge.to)) {
        (Some(f), Some(t)) => f == t,
        (None, None) => !drop_orphans,
        _ => false,
    }
}

/// Filter nodes at one level: drop root-level nodes when `drop_orphans` is set.
fn filter_nodes(
    graph: &DepGraph,
    group_map: &HashMap<String, usize>,
    drop_orphans: bool,
) -> indexmap::IndexMap<String, NodeInfo> {
    if drop_orphans {
        graph
            .nodes
            .iter()
            .filter(|(id, _)| group_map.contains_key(id.as_str()))
            .map(|(id, info)| (id.clone(), info.clone()))
            .collect()
    } else {
        graph.nodes.clone()
    }
}

fn slice_toplevel(graph: &DepGraph, drop_orphans: bool) -> DepGraph {
    let mut group_map = HashMap::new();
    for (i, sg) in graph.subgraphs.iter().enumerate() {
        assign_group(sg, &mut group_map, i);
    }
    rebuild(graph, &group_map, drop_orphans)
}

/// Rebuild the graph tree, filtering edges using a global group map.
fn rebuild(graph: &DepGraph, group_map: &HashMap<String, usize>, drop_orphans: bool) -> DepGraph {
    let nodes = filter_nodes(graph, group_map, drop_orphans);
    let edges: Vec<Edge> = graph
        .edges
        .iter()
        .filter(|e| edge_allowed(e, group_map, drop_orphans))
        .cloned()
        .collect();
    let subgraphs = graph
        .subgraphs
        .iter()
        .map(|sg| rebuild(sg, group_map, drop_orphans))
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

fn slice_recursive(graph: &DepGraph, drop_orphans: bool) -> DepGraph {
    // Nothing to slice at a leaf level (no subgraph boundaries).
    if graph.subgraphs.is_empty() {
        return DepGraph {
            id: graph.id.clone(),
            attrs: graph.attrs.clone(),
            nodes: graph.nodes.clone(),
            edges: graph.edges.clone(),
            subgraphs: vec![],
            ..Default::default()
        };
    }

    let mut group_map = HashMap::new();
    for (i, sg) in graph.subgraphs.iter().enumerate() {
        assign_group(sg, &mut group_map, i);
    }

    let nodes = filter_nodes(graph, &group_map, drop_orphans);
    let edges: Vec<Edge> = graph
        .edges
        .iter()
        .filter(|e| edge_allowed(e, &group_map, drop_orphans))
        .cloned()
        .collect();
    let subgraphs = graph
        .subgraphs
        .iter()
        .map(|sg| slice_recursive(sg, drop_orphans))
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

    fn make_graph(nodes: &[&str], edges: &[(&str, &str)], subgraphs: Vec<DepGraph>) -> DepGraph {
        DepGraph {
            nodes: nodes
                .iter()
                .map(|id| (id.to_string(), NodeInfo::new(*id)))
                .collect(),
            edges: edges
                .iter()
                .map(|(from, to)| Edge {
                    from: from.to_string(),
                    to: to.to_string(),
                    ..Default::default()
                })
                .collect(),
            subgraphs,
            ..Default::default()
        }
    }

    fn node_ids(graph: &DepGraph) -> Vec<&str> {
        graph.nodes.keys().map(|s| s.as_str()).collect()
    }

    fn edge_pairs(graph: &DepGraph) -> Vec<(&str, &str)> {
        graph
            .edges
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str()))
            .collect()
    }

    #[test]
    fn empty_graph() {
        let g = DepGraph::default();
        let result = slice(&g, &SliceArgs::default()).unwrap();
        assert!(result.nodes.is_empty());
        assert!(result.edges.is_empty());
        assert!(result.subgraphs.is_empty());
    }

    #[test]
    fn no_subgraphs_preserves_all() {
        let g = make_graph(&["a", "b"], &[("a", "b")], vec![]);
        let result = slice(&g, &SliceArgs::default()).unwrap();
        assert_eq!(node_ids(&result), vec!["a", "b"]);
        assert_eq!(edge_pairs(&result), vec![("a", "b")]);
    }

    #[test]
    fn no_subgraphs_drop_orphans_removes_all() {
        let g = make_graph(&["a", "b"], &[("a", "b")], vec![]);
        let args = SliceArgs {
            drop_orphans: true,
            ..Default::default()
        };
        let result = slice(&g, &args).unwrap();
        assert!(result.nodes.is_empty());
        assert!(result.edges.is_empty());
    }

    #[test]
    fn cross_subgraph_edges_removed() {
        let sg0 = make_graph(&["a"], &[], vec![]);
        let sg1 = make_graph(&["b"], &[], vec![]);
        let g = make_graph(&[], &[("a", "b")], vec![sg0, sg1]);
        let result = slice(&g, &SliceArgs::default()).unwrap();
        assert!(result.edges.is_empty());
    }

    #[test]
    fn intra_subgraph_edges_preserved() {
        let sg = make_graph(&["a", "b"], &[], vec![]);
        let g = make_graph(&[], &[("a", "b")], vec![sg]);
        let result = slice(&g, &SliceArgs::default()).unwrap();
        assert_eq!(edge_pairs(&result), vec![("a", "b")]);
    }

    #[test]
    fn root_to_subgraph_edges_removed() {
        let sg = make_graph(&["b"], &[], vec![]);
        let g = make_graph(&["a"], &[("a", "b")], vec![sg]);
        let result = slice(&g, &SliceArgs::default()).unwrap();
        assert!(result.edges.is_empty());
        assert_eq!(node_ids(&result), vec!["a"]);
    }

    #[test]
    fn root_nodes_preserved_by_default() {
        let sg = make_graph(&["b"], &[], vec![]);
        let g = make_graph(&["a"], &[], vec![sg]);
        let result = slice(&g, &SliceArgs::default()).unwrap();
        assert_eq!(node_ids(&result), vec!["a"]);
    }

    #[test]
    fn drop_orphans_removes_root_nodes_and_edges() {
        let sg = make_graph(&["b", "c"], &[], vec![]);
        let g = make_graph(&["a"], &[("a", "b"), ("b", "c")], vec![sg]);
        let args = SliceArgs {
            drop_orphans: true,
            ..Default::default()
        };
        let result = slice(&g, &args).unwrap();
        assert!(result.nodes.is_empty());
        // b->c kept (both in same group), a->b dropped (cross-boundary)
        assert_eq!(edge_pairs(&result), vec![("b", "c")]);
    }

    #[test]
    fn nested_subgraph_toplevel_groups_under_outermost() {
        let inner = make_graph(&["c"], &[], vec![]);
        let outer = make_graph(&["a"], &[], vec![inner]);
        let g = make_graph(&[], &[("a", "c")], vec![outer]);
        let result = slice(&g, &SliceArgs::default()).unwrap();
        // Both a and c belong to the same top-level group
        assert_eq!(edge_pairs(&result), vec![("a", "c")]);
    }

    #[test]
    fn recursive_cuts_within_nested_subgraphs() {
        let inner = make_graph(&["b"], &[], vec![]);
        let outer = make_graph(&["a"], &[("a", "b")], vec![inner]);
        let g = make_graph(&[], &[], vec![outer]);

        // Top-level: a and b both in group 0, edge preserved inside the subgraph
        let result_toplevel = slice(&g, &SliceArgs::default()).unwrap();
        assert_eq!(edge_pairs(&result_toplevel.subgraphs[0]), vec![("a", "b")]);

        // Recursive: at the outer subgraph level, a is root, b is in inner -> cut
        let args = SliceArgs {
            recursive: true,
            ..Default::default()
        };
        let result_recursive = slice(&g, &args).unwrap();
        assert!(result_recursive.subgraphs[0].edges.is_empty());
    }

    #[test]
    fn recursive_drop_orphans_at_each_level() {
        let inner = make_graph(&["b"], &[], vec![]);
        let outer = make_graph(&["a"], &[("a", "b")], vec![inner]);
        let g = make_graph(&[], &[], vec![outer]);

        let args = SliceArgs {
            recursive: true,
            drop_orphans: true,
        };
        let result = slice(&g, &args).unwrap();
        // a is an orphan at the outer subgraph level -> dropped
        assert!(result.subgraphs[0].nodes.is_empty());
        assert!(result.subgraphs[0].edges.is_empty());
        // b is in the inner subgraph -> preserved
        assert_eq!(node_ids(&result.subgraphs[0].subgraphs[0]), vec!["b"]);
    }

    #[test]
    fn subgraph_attrs_and_id_preserved() {
        let mut sg = make_graph(&["a"], &[], vec![]);
        sg.id = Some("cluster_0".to_string());
        sg.attrs.insert("color".to_string(), "blue".to_string());
        let g = make_graph(&[], &[], vec![sg]);

        let result = slice(&g, &SliceArgs::default()).unwrap();
        assert_eq!(result.subgraphs[0].id.as_deref(), Some("cluster_0"));
        assert_eq!(
            result.subgraphs[0].attrs.get("color").map(String::as_str),
            Some("blue")
        );
    }
}
