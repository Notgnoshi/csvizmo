use std::collections::HashSet;
use std::io::Write;

use indexmap::IndexMap;

use crate::{DepGraph, Edge, NodeInfo};

/// Status of a node or edge in a graph diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffStatus {
    Added,
    Removed,
    Changed,
    Moved,
    Unchanged,
}

/// A node with its diff status.
#[derive(Debug)]
pub struct DiffNode {
    pub status: DiffStatus,
    pub info: NodeInfo,
}

/// An edge with its diff status.
#[derive(Debug)]
pub struct DiffEdge {
    pub status: DiffStatus,
    pub edge: Edge,
}

/// Result of diffing two dependency graphs.
#[derive(Debug)]
pub struct GraphDiff {
    pub nodes: IndexMap<String, DiffNode>,
    pub edges: Vec<DiffEdge>,
}

impl GraphDiff {
    /// Returns true if any node or edge has a status other than Unchanged.
    pub fn has_changes(&self) -> bool {
        self.nodes
            .values()
            .any(|n| n.status != DiffStatus::Unchanged)
            || self.edges.iter().any(|e| e.status != DiffStatus::Unchanged)
    }
}

fn node_eq(a: &NodeInfo, b: &NodeInfo) -> bool {
    a.label == b.label && a.node_type == b.node_type && a.attrs == b.attrs
}

fn edge_eq(a: &Edge, b: &Edge) -> bool {
    a.label == b.label && a.attrs == b.attrs
}

fn build_incoming(edges: &[Edge]) -> IndexMap<String, Vec<String>> {
    let mut incoming: IndexMap<String, Vec<String>> = IndexMap::new();
    for edge in edges {
        incoming
            .entry(edge.to.clone())
            .or_default()
            .push(edge.from.clone());
    }
    incoming
}

/// Compute the difference between two dependency graphs.
///
/// Nodes are matched by ID. Edges are matched by (from, to) tuple.
/// Content equality for nodes compares label, node_type, and attrs.
/// Content equality for edges compares label and attrs.
/// Nodes that are unchanged in content but have a single parent that
/// changed are marked as Moved.
pub fn diff(before: &DepGraph, after: &DepGraph) -> GraphDiff {
    let before_nodes = before.all_nodes();
    let after_nodes = after.all_nodes();
    let before_edges = before.all_edges();
    let after_edges = after.all_edges();

    let mut nodes = IndexMap::new();

    // After-graph nodes: Added, Changed, or Unchanged
    for (id, after_info) in after_nodes {
        let status = match before_nodes.get(id) {
            Some(before_info) => {
                if node_eq(before_info, after_info) {
                    DiffStatus::Unchanged
                } else {
                    DiffStatus::Changed
                }
            }
            None => DiffStatus::Added,
        };
        nodes.insert(
            id.clone(),
            DiffNode {
                status,
                info: after_info.clone(),
            },
        );
    }

    // Before-only nodes: Removed
    for (id, before_info) in before_nodes {
        if !after_nodes.contains_key(id) {
            nodes.insert(
                id.clone(),
                DiffNode {
                    status: DiffStatus::Removed,
                    info: before_info.clone(),
                },
            );
        }
    }

    // Build before-edge lookup grouped by (from, to), consuming matched entries as we go
    let mut before_edge_map: IndexMap<(String, String), Vec<Edge>> = IndexMap::new();
    for edge in before_edges {
        let key = (edge.from.clone(), edge.to.clone());
        before_edge_map.entry(key).or_default().push(edge.clone());
    }

    let mut edges = Vec::new();

    for edge in after_edges {
        let key = (edge.from.clone(), edge.to.clone());
        let status = match before_edge_map.get_mut(&key) {
            Some(before_edges) => {
                if let Some(pos) = before_edges.iter().position(|be| edge_eq(be, edge)) {
                    before_edges.swap_remove(pos);
                    DiffStatus::Unchanged
                } else if !before_edges.is_empty() {
                    before_edges.swap_remove(0);
                    DiffStatus::Changed
                } else {
                    DiffStatus::Added
                }
            }
            None => DiffStatus::Added,
        };
        edges.push(DiffEdge {
            status,
            edge: edge.clone(),
        });
    }

    // Remaining before edges are Removed
    for (_, remaining) in before_edge_map {
        for edge in remaining {
            edges.push(DiffEdge {
                status: DiffStatus::Removed,
                edge,
            });
        }
    }

    // Move detection: upgrade Unchanged nodes whose single parent changed
    let before_incoming = build_incoming(before_edges);
    let after_incoming = build_incoming(after_edges);

    for (id, diff_node) in &mut nodes {
        if diff_node.status != DiffStatus::Unchanged {
            continue;
        }
        let before_parents = before_incoming.get(id.as_str());
        let after_parents = after_incoming.get(id.as_str());
        match (before_parents, after_parents) {
            (Some(bp), Some(ap)) if bp.len() == 1 && ap.len() == 1 && bp[0] != ap[0] => {
                diff_node.status = DiffStatus::Moved;
            }
            _ => {}
        }
    }

    GraphDiff { nodes, edges }
}

/// Build an annotated graph combining both inputs with visual diff styling.
///
/// Added nodes/edges are green, removed are red, changed are orange,
/// moved are blue. Each element gets a `diff` attribute for programmatic
/// filtering. The after-graph's subgraph structure is preserved: nodes
/// appear in their original subgraph positions. Removed nodes (only in
/// the before-graph) are placed at root level, or into a `cluster_removed`
/// subgraph when `cluster` is true.
pub fn annotate_graph(diff: &GraphDiff, after: &DepGraph, cluster: bool) -> DepGraph {
    fn annotate_node(diff_node: &DiffNode) -> NodeInfo {
        let mut info = diff_node.info.clone();
        match diff_node.status {
            DiffStatus::Added => {
                info.label = format!("+ {}", info.label);
                info.attrs.insert("color".into(), "green".into());
                info.attrs.insert("fontcolor".into(), "green".into());
                info.attrs.insert("diff".into(), "added".into());
            }
            DiffStatus::Removed => {
                info.label = format!("- {}", info.label);
                info.attrs.insert("color".into(), "red".into());
                info.attrs.insert("fontcolor".into(), "red".into());
                info.attrs.insert("diff".into(), "removed".into());
            }
            DiffStatus::Changed => {
                info.label = format!("~ {}", info.label);
                info.attrs.insert("color".into(), "orange".into());
                info.attrs.insert("fontcolor".into(), "orange".into());
                info.attrs.insert("diff".into(), "changed".into());
            }
            DiffStatus::Moved => {
                info.label = format!("> {}", info.label);
                info.attrs.insert("color".into(), "blue".into());
                info.attrs.insert("fontcolor".into(), "blue".into());
                info.attrs.insert("diff".into(), "moved".into());
            }
            DiffStatus::Unchanged => {
                info.attrs.insert("diff".into(), "unchanged".into());
            }
        }
        info
    }

    fn annotate_subgraph(diff: &GraphDiff, subgraph: &DepGraph) -> DepGraph {
        let nodes: IndexMap<String, NodeInfo> = subgraph
            .nodes
            .keys()
            .filter_map(|id| {
                let diff_node = diff.nodes.get(id)?;
                Some((id.clone(), annotate_node(diff_node)))
            })
            .collect();

        let subgraphs: Vec<DepGraph> = subgraph
            .subgraphs
            .iter()
            .map(|sg| annotate_subgraph(diff, sg))
            .filter(|sg| !sg.nodes.is_empty() || !sg.subgraphs.is_empty())
            .collect();

        DepGraph {
            id: subgraph.id.clone(),
            attrs: subgraph.attrs.clone(),
            nodes,
            subgraphs,
            ..Default::default()
        }
    }

    // Walk the after-graph tree to place nodes in their original positions.
    let mut root = annotate_subgraph(diff, after);

    // Removed nodes are not in the after-graph; collect them separately.
    let removed_nodes: IndexMap<String, NodeInfo> = diff
        .nodes
        .iter()
        .filter(|(_, n)| n.status == DiffStatus::Removed)
        .map(|(id, n)| (id.clone(), annotate_node(n)))
        .collect();

    if !removed_nodes.is_empty() {
        if cluster {
            root.subgraphs.push(DepGraph {
                id: Some("cluster_removed".into()),
                nodes: removed_nodes,
                ..Default::default()
            });
        } else {
            root.nodes.extend(removed_nodes);
        }
    }

    // Edges stay at root level.
    for diff_edge in &diff.edges {
        let mut edge = diff_edge.edge.clone();
        match diff_edge.status {
            DiffStatus::Added => {
                edge.attrs.insert("color".into(), "green".into());
                edge.attrs.insert("diff".into(), "added".into());
            }
            DiffStatus::Removed => {
                edge.attrs.insert("color".into(), "red".into());
                edge.attrs.insert("diff".into(), "removed".into());
            }
            DiffStatus::Changed => {
                edge.attrs.insert("color".into(), "orange".into());
                edge.attrs.insert("diff".into(), "changed".into());
            }
            DiffStatus::Moved => {
                edge.attrs.insert("color".into(), "blue".into());
                edge.attrs.insert("diff".into(), "moved".into());
            }
            DiffStatus::Unchanged => {
                edge.attrs.insert("diff".into(), "unchanged".into());
            }
        }
        root.edges.push(edge);
    }

    root
}

/// Build a graph containing only nodes exclusive to the "before" graph.
///
/// The before-graph's subgraph structure is preserved. Edges are included
/// only when both endpoints are removed nodes. Empty subgraphs are dropped.
pub fn subtract_graph(diff: &GraphDiff, before: &DepGraph) -> DepGraph {
    let removed_ids: HashSet<&str> = diff
        .nodes
        .iter()
        .filter(|(_, n)| n.status == DiffStatus::Removed)
        .map(|(id, _)| id.as_str())
        .collect();

    fn filter_subgraph(graph: &DepGraph, keep: &HashSet<&str>) -> DepGraph {
        DepGraph {
            id: graph.id.clone(),
            attrs: graph.attrs.clone(),
            nodes: graph
                .nodes
                .iter()
                .filter(|(id, _)| keep.contains(id.as_str()))
                .map(|(id, info)| (id.clone(), info.clone()))
                .collect(),
            edges: graph
                .edges
                .iter()
                .filter(|e| keep.contains(e.from.as_str()) && keep.contains(e.to.as_str()))
                .cloned()
                .collect(),
            subgraphs: graph
                .subgraphs
                .iter()
                .map(|sg| filter_subgraph(sg, keep))
                .filter(|sg| !sg.nodes.is_empty() || !sg.subgraphs.is_empty())
                .collect(),
            ..Default::default()
        }
    }

    filter_subgraph(before, &removed_ids)
}

/// Write a tab-delimited listing of changed nodes and edges.
///
/// Unchanged items are omitted. Node format: `<status>\t<id>\t<label>`
/// (label column omitted when it equals the ID). Edge format:
/// `<status>\t<from>\t<to>`.
pub fn write_list(diff: &GraphDiff, writer: &mut dyn Write) -> eyre::Result<()> {
    for (id, diff_node) in &diff.nodes {
        let prefix = match diff_node.status {
            DiffStatus::Added => "+",
            DiffStatus::Removed => "-",
            DiffStatus::Changed => "~",
            DiffStatus::Moved => ">",
            DiffStatus::Unchanged => continue,
        };
        if diff_node.info.label == *id {
            writeln!(writer, "{prefix}\t{id}")?;
        } else {
            writeln!(writer, "{prefix}\t{id}\t{}", diff_node.info.label)?;
        }
    }
    for diff_edge in &diff.edges {
        let prefix = match diff_edge.status {
            DiffStatus::Added => "+",
            DiffStatus::Removed => "-",
            DiffStatus::Changed => "~",
            DiffStatus::Moved => ">",
            DiffStatus::Unchanged => continue,
        };
        writeln!(
            writer,
            "{prefix}\t{}\t{}",
            diff_edge.edge.from, diff_edge.edge.to
        )?;
    }
    Ok(())
}

/// Write a tab-delimited summary of diff counts.
pub fn write_summary(diff: &GraphDiff, writer: &mut dyn Write) -> eyre::Result<()> {
    let (mut added_n, mut removed_n, mut changed_n, mut moved_n, mut unchanged_n) =
        (0usize, 0, 0, 0, 0);
    for n in diff.nodes.values() {
        match n.status {
            DiffStatus::Added => added_n += 1,
            DiffStatus::Removed => removed_n += 1,
            DiffStatus::Changed => changed_n += 1,
            DiffStatus::Moved => moved_n += 1,
            DiffStatus::Unchanged => unchanged_n += 1,
        }
    }

    let (mut added_e, mut removed_e, mut changed_e, mut unchanged_e) = (0usize, 0, 0, 0);
    for e in &diff.edges {
        match e.status {
            DiffStatus::Added => added_e += 1,
            DiffStatus::Removed => removed_e += 1,
            DiffStatus::Changed => changed_e += 1,
            DiffStatus::Moved | DiffStatus::Unchanged => unchanged_e += 1,
        }
    }

    writeln!(writer, "added_nodes\t{added_n}")?;
    writeln!(writer, "removed_nodes\t{removed_n}")?;
    writeln!(writer, "changed_nodes\t{changed_n}")?;
    writeln!(writer, "moved_nodes\t{moved_n}")?;
    writeln!(writer, "unchanged_nodes\t{unchanged_n}")?;
    writeln!(writer, "added_edges\t{added_e}")?;
    writeln!(writer, "removed_edges\t{removed_e}")?;
    writeln!(writer, "changed_edges\t{changed_e}")?;
    writeln!(writer, "unchanged_edges\t{unchanged_e}")?;
    Ok(())
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

    // -- diff --

    #[test]
    fn diff_identical() {
        let g = make_graph(&[("a", "A"), ("b", "B")], &[("a", "b")]);
        let result = diff(&g, &g);
        assert!(
            result
                .nodes
                .values()
                .all(|n| n.status == DiffStatus::Unchanged)
        );
        assert!(
            result
                .edges
                .iter()
                .all(|e| e.status == DiffStatus::Unchanged)
        );
    }

    #[test]
    fn diff_disjoint() {
        let before = make_graph(&[("a", "A")], &[]);
        let after = make_graph(&[("b", "B")], &[]);
        let result = diff(&before, &after);
        assert_eq!(result.nodes["b"].status, DiffStatus::Added);
        assert_eq!(result.nodes["a"].status, DiffStatus::Removed);
    }

    #[test]
    fn diff_mixed() {
        let before = make_graph(&[("a", "A"), ("b", "B")], &[("a", "b")]);
        let after = make_graph(&[("b", "B"), ("c", "C")], &[("b", "c")]);
        let result = diff(&before, &after);
        assert_eq!(result.nodes["b"].status, DiffStatus::Unchanged);
        assert_eq!(result.nodes["c"].status, DiffStatus::Added);
        assert_eq!(result.nodes["a"].status, DiffStatus::Removed);
    }

    #[test]
    fn diff_changed_label() {
        let before = make_graph(&[("a", "Alpha")], &[]);
        let after = make_graph(&[("a", "Aleph")], &[]);
        let result = diff(&before, &after);
        assert_eq!(result.nodes["a"].status, DiffStatus::Changed);
        assert_eq!(result.nodes["a"].info.label, "Aleph");
    }

    #[test]
    fn diff_changed_node_type() {
        let mut before = make_graph(&[("a", "A")], &[]);
        before.nodes.get_mut("a").unwrap().node_type = Some("lib".to_string());
        let mut after = make_graph(&[("a", "A")], &[]);
        after.nodes.get_mut("a").unwrap().node_type = Some("bin".to_string());
        let result = diff(&before, &after);
        assert_eq!(result.nodes["a"].status, DiffStatus::Changed);
    }

    #[test]
    fn diff_changed_attrs() {
        let mut before = make_graph(&[("a", "A")], &[]);
        before
            .nodes
            .get_mut("a")
            .unwrap()
            .attrs
            .insert("color".to_string(), "red".to_string());
        let mut after = make_graph(&[("a", "A")], &[]);
        after
            .nodes
            .get_mut("a")
            .unwrap()
            .attrs
            .insert("color".to_string(), "blue".to_string());
        let result = diff(&before, &after);
        assert_eq!(result.nodes["a"].status, DiffStatus::Changed);
    }

    #[test]
    fn diff_changed_edge() {
        let mut before = make_graph(&[("a", "A"), ("b", "B")], &[]);
        before.edges.push(Edge {
            from: "a".to_string(),
            to: "b".to_string(),
            label: Some("uses".to_string()),
            ..Default::default()
        });
        let mut after = make_graph(&[("a", "A"), ("b", "B")], &[]);
        after.edges.push(Edge {
            from: "a".to_string(),
            to: "b".to_string(),
            label: Some("depends".to_string()),
            ..Default::default()
        });
        let result = diff(&before, &after);
        assert_eq!(result.edges[0].status, DiffStatus::Changed);
    }

    #[test]
    fn diff_duplicate_edges() {
        let mut before = make_graph(&[("a", "A"), ("b", "B")], &[]);
        before.edges.push(Edge {
            from: "a".to_string(),
            to: "b".to_string(),
            label: Some("uses".to_string()),
            ..Default::default()
        });
        before.edges.push(Edge {
            from: "a".to_string(),
            to: "b".to_string(),
            label: Some("dev".to_string()),
            ..Default::default()
        });
        let mut after = make_graph(&[("a", "A"), ("b", "B")], &[]);
        after.edges.push(Edge {
            from: "a".to_string(),
            to: "b".to_string(),
            label: Some("uses".to_string()),
            ..Default::default()
        });
        after.edges.push(Edge {
            from: "a".to_string(),
            to: "b".to_string(),
            label: Some("test".to_string()),
            ..Default::default()
        });
        let result = diff(&before, &after);
        // "uses" matched unchanged, "test" paired with "dev" as changed
        assert_eq!(result.edges.len(), 2);
        assert_eq!(result.edges[0].status, DiffStatus::Unchanged);
        assert_eq!(result.edges[0].edge.label.as_deref(), Some("uses"));
        assert_eq!(result.edges[1].status, DiffStatus::Changed);
        assert_eq!(result.edges[1].edge.label.as_deref(), Some("test"));
    }

    #[test]
    fn diff_empty() {
        let result = diff(&DepGraph::default(), &DepGraph::default());
        assert!(result.nodes.is_empty());
        assert!(result.edges.is_empty());
    }

    // -- has_changes --

    #[test]
    fn has_changes_identical() {
        let g = make_graph(&[("a", "A")], &[]);
        let d = diff(&g, &g);
        assert!(!d.has_changes());
    }

    #[test]
    fn has_changes_different() {
        let before = make_graph(&[("a", "A")], &[]);
        let after = make_graph(&[("b", "B")], &[]);
        let d = diff(&before, &after);
        assert!(d.has_changes());
    }

    #[test]
    fn has_changes_empty() {
        let d = diff(&DepGraph::default(), &DepGraph::default());
        assert!(!d.has_changes());
    }

    #[test]
    fn diff_moved_node() {
        // c has same info but different single parent
        let before = make_graph(&[("p1", "P1"), ("c", "C")], &[("p1", "c")]);
        let after = make_graph(&[("p2", "P2"), ("c", "C")], &[("p2", "c")]);
        let result = diff(&before, &after);
        assert_eq!(result.nodes["c"].status, DiffStatus::Moved);
    }

    #[test]
    fn diff_multi_parent_not_moved() {
        // c has multiple parents in both graphs -- not considered moved
        let before = make_graph(
            &[("p1", "P1"), ("p2", "P2"), ("c", "C")],
            &[("p1", "c"), ("p2", "c")],
        );
        let after = make_graph(
            &[("p1", "P1"), ("p3", "P3"), ("c", "C")],
            &[("p1", "c"), ("p3", "c")],
        );
        let result = diff(&before, &after);
        assert_eq!(result.nodes["c"].status, DiffStatus::Unchanged);
    }

    #[test]
    fn diff_changed_and_moved_stays_changed() {
        // c has different label AND different parent -- stays Changed
        let before = make_graph(&[("p1", "P1"), ("c", "C")], &[("p1", "c")]);
        let after = make_graph(&[("p2", "P2"), ("c", "C-new")], &[("p2", "c")]);
        let result = diff(&before, &after);
        assert_eq!(result.nodes["c"].status, DiffStatus::Changed);
    }

    // -- annotate_graph --

    #[test]
    fn annotate_identical() {
        let g = make_graph(&[("a", "A")], &[]);
        let d = diff(&g, &g);
        let annotated = annotate_graph(&d, &g, false);
        assert_eq!(annotated.nodes["a"].attrs["diff"], "unchanged");
        assert_eq!(annotated.nodes["a"].label, "A");
    }

    #[test]
    fn annotate_added_removed() {
        let before = make_graph(&[("a", "A")], &[]);
        let after = make_graph(&[("b", "B")], &[]);
        let d = diff(&before, &after);
        let annotated = annotate_graph(&d, &after, false);
        assert_eq!(annotated.nodes["b"].label, "+ B");
        assert_eq!(annotated.nodes["b"].attrs["color"], "green");
        assert_eq!(annotated.nodes["b"].attrs["diff"], "added");
        assert_eq!(annotated.nodes["a"].label, "- A");
        assert_eq!(annotated.nodes["a"].attrs["color"], "red");
        assert_eq!(annotated.nodes["a"].attrs["diff"], "removed");
    }

    #[test]
    fn annotate_cluster() {
        let before = make_graph(&[("a", "A")], &[]);
        let after = make_graph(&[("b", "B")], &[]);
        let d = diff(&before, &after);
        let annotated = annotate_graph(&d, &after, true);
        // Added nodes stay in their natural position (root of after-graph).
        assert!(annotated.nodes.contains_key("b"));
        // Only cluster_removed is created.
        assert_eq!(annotated.subgraphs.len(), 1);
        assert_eq!(
            annotated.subgraphs[0].id.as_deref(),
            Some("cluster_removed")
        );
        assert!(annotated.subgraphs[0].nodes.contains_key("a"));
    }

    #[test]
    fn annotate_preserves_subgraphs() {
        let sub = DepGraph {
            id: Some("cluster_0".into()),
            nodes: [("b".into(), NodeInfo::new("B"))].into_iter().collect(),
            ..Default::default()
        };
        let after = DepGraph {
            nodes: [("a".into(), NodeInfo::new("A"))].into_iter().collect(),
            subgraphs: vec![sub],
            ..Default::default()
        };
        let before = make_graph(&[("a", "A")], &[]);
        let d = diff(&before, &after);
        let annotated = annotate_graph(&d, &after, false);
        // "a" unchanged at root, "b" added inside subgraph
        assert!(annotated.nodes.contains_key("a"));
        assert_eq!(annotated.subgraphs.len(), 1);
        assert_eq!(annotated.subgraphs[0].id.as_deref(), Some("cluster_0"));
        assert!(annotated.subgraphs[0].nodes.contains_key("b"));
        assert_eq!(annotated.subgraphs[0].nodes["b"].attrs["diff"], "added");
    }

    // -- subtract_graph --

    #[test]
    fn subtract_only_removed() {
        let before = make_graph(&[("a", "A"), ("b", "B")], &[("a", "b")]);
        let after = make_graph(&[("b", "B")], &[]);
        let d = diff(&before, &after);
        let subtracted = subtract_graph(&d, &before);
        assert_eq!(subtracted.nodes.len(), 1);
        assert!(subtracted.nodes.contains_key("a"));
        assert!(subtracted.edges.is_empty());
    }

    #[test]
    fn subtract_with_edges() {
        let before = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c")],
        );
        let after = make_graph(&[("c", "C")], &[]);
        let d = diff(&before, &after);
        let subtracted = subtract_graph(&d, &before);
        assert_eq!(subtracted.nodes.len(), 2);
        assert!(subtracted.nodes.contains_key("a"));
        assert!(subtracted.nodes.contains_key("b"));
        assert_eq!(subtracted.edges.len(), 1);
        assert_eq!(subtracted.edges[0].from, "a");
        assert_eq!(subtracted.edges[0].to, "b");
    }

    #[test]
    fn subtract_preserves_subgraphs() {
        let sub = DepGraph {
            id: Some("cluster_0".into()),
            nodes: [("b".into(), NodeInfo::new("B"))].into_iter().collect(),
            ..Default::default()
        };
        let before = DepGraph {
            nodes: [("a".into(), NodeInfo::new("A"))].into_iter().collect(),
            subgraphs: vec![sub],
            ..Default::default()
        };
        // after has only "a", so "b" is removed
        let after = make_graph(&[("a", "A")], &[]);
        let d = diff(&before, &after);
        let subtracted = subtract_graph(&d, &before);
        // "b" should be in the subgraph, not at root
        assert!(subtracted.nodes.is_empty());
        assert_eq!(subtracted.subgraphs.len(), 1);
        assert_eq!(subtracted.subgraphs[0].id.as_deref(), Some("cluster_0"));
        assert!(subtracted.subgraphs[0].nodes.contains_key("b"));
    }

    // -- write_list --

    #[test]
    fn list_label_equals_id() {
        let before = make_graph(&[("a", "a")], &[]);
        let after = make_graph(&[("b", "b")], &[]);
        let d = diff(&before, &after);
        let mut buf = Vec::new();
        write_list(&d, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "+\tb\n-\ta\n");
    }

    #[test]
    fn list_with_labels() {
        let before = make_graph(&[("a", "Alpha")], &[]);
        let after = make_graph(&[("b", "Beta")], &[]);
        let d = diff(&before, &after);
        let mut buf = Vec::new();
        write_list(&d, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "+\tb\tBeta\n-\ta\tAlpha\n");
    }

    #[test]
    fn list_empty_diff() {
        let g = make_graph(&[("a", "A")], &[]);
        let d = diff(&g, &g);
        let mut buf = Vec::new();
        write_list(&d, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "");
    }

    #[test]
    fn list_edges() {
        let before = make_graph(&[("a", "a"), ("b", "b")], &[("a", "b")]);
        let after = make_graph(&[("a", "a"), ("b", "b")], &[]);
        let d = diff(&before, &after);
        let mut buf = Vec::new();
        write_list(&d, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "-\ta\tb\n");
    }

    // -- write_summary --

    #[test]
    fn summary_counts() {
        let before = make_graph(&[("a", "A"), ("b", "B")], &[("a", "b")]);
        let after = make_graph(&[("b", "B-new"), ("c", "C")], &[("b", "c")]);
        let d = diff(&before, &after);
        let mut buf = Vec::new();
        write_summary(&d, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(
            output,
            "\
added_nodes\t1
removed_nodes\t1
changed_nodes\t1
moved_nodes\t0
unchanged_nodes\t0
added_edges\t1
removed_edges\t1
changed_edges\t0
unchanged_edges\t0
"
        );
    }

    #[test]
    fn summary_all_unchanged() {
        let g = make_graph(&[("a", "A"), ("b", "B")], &[("a", "b")]);
        let d = diff(&g, &g);
        let mut buf = Vec::new();
        write_summary(&d, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(
            output,
            "\
added_nodes\t0
removed_nodes\t0
changed_nodes\t0
moved_nodes\t0
unchanged_nodes\t2
added_edges\t0
removed_edges\t0
changed_edges\t0
unchanged_edges\t1
"
        );
    }
}
