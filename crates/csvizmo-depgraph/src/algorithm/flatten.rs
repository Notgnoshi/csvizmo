use crate::DepGraph;

/// Flatten a graph by moving all nodes and edges from subgraphs to the top level.
pub fn flatten(graph: &DepGraph) -> DepGraph {
    DepGraph {
        id: graph.id.clone(),
        attrs: graph.attrs.clone(),
        nodes: graph.all_nodes().clone(),
        edges: graph.all_edges().clone(),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Edge, NodeInfo};

    #[test]
    fn flat_graph_unchanged() {
        let g = DepGraph {
            nodes: [("a", "A"), ("b", "B")]
                .into_iter()
                .map(|(id, label)| (id.to_string(), NodeInfo::new(label)))
                .collect(),
            edges: vec![Edge {
                from: "a".to_string(),
                to: "b".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let result = flatten(&g);
        assert_eq!(result.nodes.len(), 2);
        assert_eq!(result.edges.len(), 1);
        assert!(result.subgraphs.is_empty());
    }

    #[test]
    fn flattens_subgraph_nodes_and_edges() {
        let sub = DepGraph {
            nodes: [("c", "C")]
                .into_iter()
                .map(|(id, label)| (id.to_string(), NodeInfo::new(label)))
                .collect(),
            edges: vec![Edge {
                from: "b".to_string(),
                to: "c".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let g = DepGraph {
            nodes: [("a", "A"), ("b", "B")]
                .into_iter()
                .map(|(id, label)| (id.to_string(), NodeInfo::new(label)))
                .collect(),
            edges: vec![Edge {
                from: "a".to_string(),
                to: "b".to_string(),
                ..Default::default()
            }],
            subgraphs: vec![sub],
            ..Default::default()
        };
        let result = flatten(&g);
        assert_eq!(result.nodes.len(), 3);
        assert_eq!(result.edges.len(), 2);
        assert!(result.subgraphs.is_empty());
    }
}
