use indexmap::IndexMap;

use crate::{DepGraph, Edge, NodeInfo};

/// Merge multiple dependency graphs into one.
///
/// Nodes are unioned by ID (later graphs overwrite on collision).
/// Edges are deduplicated by (from, to): the first label wins, and
/// attributes are merged with earlier values taking precedence.
/// Named subgraphs with the same ID are recursively merged;
/// unnamed subgraphs are kept as-is.
pub fn merge(graphs: &[DepGraph]) -> DepGraph {
    let mut nodes: IndexMap<String, NodeInfo> = IndexMap::new();
    let mut edge_map: IndexMap<(String, String), Edge> = IndexMap::new();
    let mut named_subgraphs: IndexMap<String, Vec<DepGraph>> = IndexMap::new();
    let mut unnamed_subgraphs = Vec::new();

    for graph in graphs {
        for (id, info) in &graph.nodes {
            nodes.insert(id.clone(), info.clone());
        }
        for edge in &graph.edges {
            let key = (edge.from.clone(), edge.to.clone());
            match edge_map.get_mut(&key) {
                Some(existing) => {
                    if existing.label.is_none() {
                        existing.label.clone_from(&edge.label);
                    }
                    for (k, v) in &edge.attrs {
                        existing.attrs.entry(k.clone()).or_insert_with(|| v.clone());
                    }
                }
                None => {
                    edge_map.insert(key, edge.clone());
                }
            }
        }
        for sg in &graph.subgraphs {
            match &sg.id {
                Some(id) => named_subgraphs
                    .entry(id.clone())
                    .or_default()
                    .push(sg.clone()),
                None => unnamed_subgraphs.push(sg.clone()),
            }
        }
    }

    let mut subgraphs = Vec::new();
    for (id, sgs) in named_subgraphs {
        let mut merged = merge(&sgs);
        merged.id = Some(id);
        subgraphs.push(merged);
    }
    subgraphs.extend(unnamed_subgraphs);

    DepGraph {
        nodes,
        edges: edge_map.into_values().collect(),
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

    fn edge_pairs(graph: &DepGraph) -> Vec<(&str, &str)> {
        graph
            .edges
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str()))
            .collect()
    }

    #[test]
    fn merge_disjoint() {
        let g1 = make_graph(&[("a", "A")], &[]);
        let g2 = make_graph(&[("b", "B")], &[]);
        let result = merge(&[g1, g2]);
        assert_eq!(node_ids(&result), vec!["a", "b"]);
    }

    #[test]
    fn merge_overlapping_nodes() {
        let g1 = make_graph(&[("a", "A1")], &[]);
        let g2 = make_graph(&[("a", "A2")], &[]);
        let result = merge(&[g1, g2]);
        assert_eq!(node_ids(&result), vec!["a"]);
        // Later graph overwrites
        assert_eq!(result.nodes["a"].label, "A2");
    }

    #[test]
    fn merge_edges() {
        let g1 = make_graph(&[("a", "A"), ("b", "B")], &[("a", "b")]);
        let g2 = make_graph(&[("b", "B"), ("c", "C")], &[("b", "c")]);
        let result = merge(&[g1, g2]);
        assert_eq!(edge_pairs(&result), vec![("a", "b"), ("b", "c")]);
    }

    #[test]
    fn merge_deduplicates_edges() {
        let g1 = make_graph(&[("a", "A"), ("b", "B")], &[("a", "b")]);
        let g2 = make_graph(&[("a", "A"), ("b", "B")], &[("a", "b")]);
        let result = merge(&[g1, g2]);
        assert_eq!(edge_pairs(&result), vec![("a", "b")]);
    }

    #[test]
    fn merge_edges_first_label_wins() {
        let mut g1 = make_graph(&[("a", "A"), ("b", "B")], &[]);
        g1.edges.push(Edge {
            from: "a".to_string(),
            to: "b".to_string(),
            label: Some("uses".to_string()),
            ..Default::default()
        });
        let mut g2 = make_graph(&[("a", "A"), ("b", "B")], &[]);
        g2.edges.push(Edge {
            from: "a".to_string(),
            to: "b".to_string(),
            label: Some("depends_on".to_string()),
            ..Default::default()
        });
        let result = merge(&[g1, g2]);
        assert_eq!(result.edges.len(), 1);
        assert_eq!(result.edges[0].label.as_deref(), Some("uses"));
    }

    #[test]
    fn merge_edges_attrs_merged() {
        let mut g1 = make_graph(&[("a", "A"), ("b", "B")], &[]);
        let mut e1 = Edge {
            from: "a".to_string(),
            to: "b".to_string(),
            ..Default::default()
        };
        e1.attrs.insert("color".to_string(), "red".to_string());
        g1.edges.push(e1);

        let mut g2 = make_graph(&[("a", "A"), ("b", "B")], &[]);
        let mut e2 = Edge {
            from: "a".to_string(),
            to: "b".to_string(),
            ..Default::default()
        };
        e2.attrs.insert("color".to_string(), "blue".to_string());
        e2.attrs.insert("style".to_string(), "dashed".to_string());
        g2.edges.push(e2);

        let result = merge(&[g1, g2]);
        assert_eq!(result.edges.len(), 1);
        // First wins for conflicting attrs
        assert_eq!(result.edges[0].attrs["color"], "red");
        // New attrs from later graph are added
        assert_eq!(result.edges[0].attrs["style"], "dashed");
    }

    #[test]
    fn merge_empty() {
        let result = merge(&[DepGraph::default(), DepGraph::default()]);
        assert!(result.nodes.is_empty());
        assert!(result.edges.is_empty());
    }

    fn make_subgraph(id: &str, nodes: &[(&str, &str)], edges: &[(&str, &str)]) -> DepGraph {
        DepGraph {
            id: Some(id.to_string()),
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

    #[test]
    fn merge_preserves_subgraphs() {
        let g1 = DepGraph {
            nodes: [("a", "A")]
                .into_iter()
                .map(|(id, label)| (id.to_string(), NodeInfo::new(label)))
                .collect(),
            subgraphs: vec![make_subgraph("cluster_0", &[("c", "C")], &[])],
            ..Default::default()
        };
        let g2 = make_graph(&[("b", "B")], &[]);
        let result = merge(&[g1, g2]);
        assert_eq!(node_ids(&result), vec!["a", "b"]);
        assert_eq!(result.subgraphs.len(), 1);
        assert_eq!(result.subgraphs[0].id.as_deref(), Some("cluster_0"));
        assert_eq!(node_ids(&result.subgraphs[0]), vec!["c"]);
    }

    #[test]
    fn merge_named_subgraphs_by_id() {
        let g1 = DepGraph {
            subgraphs: vec![make_subgraph("cluster_0", &[("a", "A")], &[])],
            ..Default::default()
        };
        let g2 = DepGraph {
            subgraphs: vec![make_subgraph("cluster_0", &[("b", "B")], &[])],
            ..Default::default()
        };
        let result = merge(&[g1, g2]);
        assert_eq!(result.subgraphs.len(), 1);
        assert_eq!(result.subgraphs[0].id.as_deref(), Some("cluster_0"));
        assert_eq!(node_ids(&result.subgraphs[0]), vec!["a", "b"]);
    }

    #[test]
    fn merge_disjoint_subgraphs() {
        let g1 = DepGraph {
            subgraphs: vec![make_subgraph("cluster_a", &[("a", "A")], &[])],
            ..Default::default()
        };
        let g2 = DepGraph {
            subgraphs: vec![make_subgraph("cluster_b", &[("b", "B")], &[])],
            ..Default::default()
        };
        let result = merge(&[g1, g2]);
        assert_eq!(result.subgraphs.len(), 2);
    }
}
