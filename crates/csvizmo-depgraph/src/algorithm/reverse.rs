use crate::DepGraph;

/// Reverse the direction of all edges in the graph.
pub fn reverse(graph: &DepGraph) -> DepGraph {
    reverse_inner(graph)
}

fn reverse_inner(graph: &DepGraph) -> DepGraph {
    DepGraph {
        id: graph.id.clone(),
        attrs: graph.attrs.clone(),
        nodes: graph.nodes.clone(),
        edges: graph
            .edges
            .iter()
            .map(|e| crate::Edge {
                from: e.to.clone(),
                to: e.from.clone(),
                label: e.label.clone(),
                attrs: e.attrs.clone(),
            })
            .collect(),
        subgraphs: graph.subgraphs.iter().map(reverse_inner).collect(),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Edge, NodeInfo};

    fn make_graph(
        nodes: &[(&str, &str)],
        edges: &[(&str, &str)],
        subgraphs: Vec<DepGraph>,
    ) -> DepGraph {
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
            subgraphs,
            ..Default::default()
        }
    }

    fn edge_pairs(graph: &DepGraph) -> Vec<(&str, &str)> {
        graph
            .edges
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str()))
            .collect()
    }

    #[test]
    fn reverses_edges() {
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c")],
            vec![],
        );
        let result = reverse(&g);
        assert_eq!(edge_pairs(&result), vec![("b", "a"), ("c", "b")]);
    }

    #[test]
    fn preserves_nodes() {
        let g = make_graph(&[("a", "A"), ("b", "B")], &[("a", "b")], vec![]);
        let result = reverse(&g);
        assert_eq!(result.nodes.len(), 2);
        assert_eq!(result.nodes["a"].label, "A");
        assert_eq!(result.nodes["b"].label, "B");
    }

    #[test]
    fn reverses_subgraph_edges() {
        let sub = make_graph(&[("c", "C")], &[("c", "d")], vec![]);
        let g = make_graph(&[("a", "A"), ("d", "D")], &[("a", "d")], vec![sub]);
        let result = reverse(&g);
        assert_eq!(edge_pairs(&result), vec![("d", "a")]);
        assert_eq!(edge_pairs(&result.subgraphs[0]), vec![("d", "c")]);
    }

    #[test]
    fn empty_graph() {
        let g = DepGraph::default();
        let result = reverse(&g);
        assert!(result.nodes.is_empty());
        assert!(result.edges.is_empty());
    }

    #[test]
    fn preserves_edge_attrs() {
        let mut g = make_graph(&[("a", "A"), ("b", "B")], &[], vec![]);
        g.edges.push(Edge {
            from: "a".to_string(),
            to: "b".to_string(),
            label: Some("dep".to_string()),
            ..Default::default()
        });
        let result = reverse(&g);
        assert_eq!(result.edges[0].from, "b");
        assert_eq!(result.edges[0].to, "a");
        assert_eq!(result.edges[0].label.as_deref(), Some("dep"));
    }
}
