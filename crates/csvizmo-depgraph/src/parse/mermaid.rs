use indexmap::IndexMap;
use mermaid_rs_renderer::ir::{Direction, EdgeStyle, NodeShape};
use mermaid_rs_renderer::parse_mermaid;

use crate::{DepGraph, Edge, NodeInfo};

fn map_direction(d: Direction) -> &'static str {
    match d {
        Direction::TopDown => "TD",
        Direction::LeftRight => "LR",
        Direction::BottomTop => "BT",
        Direction::RightLeft => "RL",
    }
}

fn map_shape(s: NodeShape) -> Option<&'static str> {
    match s {
        NodeShape::Rectangle
        | NodeShape::ForkJoin
        | NodeShape::ActorBox
        | NodeShape::MindmapDefault => None,
        NodeShape::RoundRect => Some("rounded"),
        NodeShape::Stadium => Some("stadium"),
        NodeShape::Subroutine => Some("subroutine"),
        NodeShape::Cylinder => Some("cylinder"),
        NodeShape::Circle => Some("circle"),
        NodeShape::DoubleCircle => Some("doublecircle"),
        NodeShape::Diamond => Some("diamond"),
        NodeShape::Hexagon => Some("hexagon"),
        NodeShape::Parallelogram => Some("parallelogram"),
        NodeShape::ParallelogramAlt => Some("parallelogram-alt"),
        NodeShape::Trapezoid => Some("trapezoid"),
        NodeShape::TrapezoidAlt => Some("trapezoid-alt"),
        NodeShape::Asymmetric => Some("asymmetric"),
        NodeShape::Text => Some("plaintext"),
    }
}

fn map_edge_style(s: EdgeStyle) -> Option<&'static str> {
    match s {
        EdgeStyle::Solid => None,
        EdgeStyle::Dotted => Some("dotted"),
        EdgeStyle::Thick => Some("thick"),
    }
}

pub fn parse(input: &str) -> eyre::Result<DepGraph> {
    let parsed = parse_mermaid(input).map_err(|e| eyre::eyre!("mermaid parse error: {e}"))?;
    let graph = &parsed.graph;

    let mut result = DepGraph::default();
    result.attrs.insert(
        "direction".to_string(),
        map_direction(graph.direction).to_string(),
    );

    // Sort nodes by their insertion order (node_order) for deterministic output.
    let mut ordered_ids: Vec<&String> = graph.nodes.keys().collect();
    ordered_ids.sort_by_key(|id| {
        graph
            .node_order
            .get(id.as_str())
            .copied()
            .unwrap_or(usize::MAX)
    });

    // Collect which nodes belong to subgraphs so we can partition them.
    let mut subgraph_node_ids = std::collections::HashSet::new();
    for sg in &graph.subgraphs {
        for node_id in &sg.nodes {
            subgraph_node_ids.insert(node_id.as_str());
        }
    }

    // Build subgraphs first, moving matching nodes into them.
    for sg in &graph.subgraphs {
        let mut sub = DepGraph {
            id: sg.id.clone(),
            ..Default::default()
        };
        if sg.label != sg.id.as_deref().unwrap_or("") {
            sub.attrs.insert("label".to_string(), sg.label.clone());
        }
        if let Some(dir) = sg.direction {
            sub.attrs
                .insert("direction".to_string(), map_direction(dir).to_string());
        }
        for node_id in &sg.nodes {
            if let Some(node) = graph.nodes.get(node_id) {
                sub.nodes.insert(node.id.clone(), convert_node(node));
            }
        }
        result.subgraphs.push(sub);
    }

    // Add top-level nodes (those not in any subgraph).
    for id in &ordered_ids {
        if !subgraph_node_ids.contains(id.as_str())
            && let Some(node) = graph.nodes.get(id.as_str())
        {
            result.nodes.insert(node.id.clone(), convert_node(node));
        }
    }

    // Convert edges. All edges stay at top level.
    for edge in &graph.edges {
        let mut e = Edge {
            from: edge.from.clone(),
            to: edge.to.clone(),
            label: edge.label.clone(),
            ..Default::default()
        };
        if let Some(style) = map_edge_style(edge.style) {
            e.attrs.insert("style".to_string(), style.to_string());
        }
        result.edges.push(e);
    }

    Ok(result)
}

fn convert_node(node: &mermaid_rs_renderer::ir::Node) -> NodeInfo {
    let label = if node.label != node.id {
        node.label.clone()
    } else {
        node.id.clone()
    };
    let mut attrs = IndexMap::new();
    if let Some(shape) = map_shape(node.shape) {
        attrs.insert("shape".to_string(), shape.to_string());
    }
    NodeInfo {
        label,
        node_type: None,
        attrs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_flowchart() {
        let graph = parse("flowchart LR\n").unwrap();
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
        assert_eq!(graph.attrs.get("direction").unwrap(), "LR");
    }

    #[test]
    fn simple_nodes_and_edges() {
        let input = include_str!("../../../../data/depconv/flowchart.mmd");
        let graph = parse(input).unwrap();

        assert_eq!(graph.attrs.get("direction").unwrap(), "LR");
        assert_eq!(graph.nodes.len(), 3);

        assert_eq!(graph.nodes["A"].label.as_str(), "myapp");
        assert_eq!(graph.nodes["B"].label.as_str(), "libfoo");
        assert_eq!(graph.nodes["C"].label.as_str(), "libbar");

        assert_eq!(graph.edges.len(), 3);
        assert_eq!(graph.edges[0].from, "A");
        assert_eq!(graph.edges[0].to, "B");
        assert_eq!(graph.edges[0].label.as_deref(), Some("static"));
        assert_eq!(graph.edges[1].from, "A");
        assert_eq!(graph.edges[1].to, "C");
        assert_eq!(graph.edges[1].label.as_deref(), Some("dynamic"));
        assert_eq!(graph.edges[2].from, "B");
        assert_eq!(graph.edges[2].to, "C");
        assert_eq!(graph.edges[2].label, None);
    }

    #[test]
    fn subgraphs() {
        let input = include_str!("../../../../data/depconv/subgraph.mmd");
        let graph = parse(input).unwrap();

        assert_eq!(graph.attrs.get("direction").unwrap(), "TD");
        assert_eq!(graph.subgraphs.len(), 2);

        let backend = &graph.subgraphs[0];
        assert_eq!(backend.id.as_deref(), Some("backend"));
        assert_eq!(backend.nodes.len(), 3);
        assert!(backend.nodes.contains_key("api"));
        assert!(backend.nodes.contains_key("db"));
        assert!(backend.nodes.contains_key("cache"));
        assert_eq!(backend.nodes["api"].label.as_str(), "API Server");

        let frontend = &graph.subgraphs[1];
        assert_eq!(frontend.id.as_deref(), Some("frontend"));
        assert_eq!(frontend.nodes.len(), 2);
        assert!(frontend.nodes.contains_key("web"));
        assert!(frontend.nodes.contains_key("mobile"));

        // Nodes in subgraphs should not be at top level
        assert!(graph.nodes.is_empty());

        // All edges remain at top level
        assert_eq!(graph.edges.len(), 4);
    }

    #[test]
    fn node_shapes() {
        // (( )) = doublecircle, { } = diamond, {{ }} = hexagon, [ ] = rectangle
        let input = "flowchart LR\n    A((dcircle))\n    B{diamond}\n    C{{hexagon}}\n    D[rectangle]\n    E([stadium])\n";
        let graph = parse(input).unwrap();

        assert_eq!(graph.nodes["A"].attrs.get("shape").unwrap(), "doublecircle");
        assert_eq!(graph.nodes["B"].attrs.get("shape").unwrap(), "diamond");
        assert_eq!(graph.nodes["C"].attrs.get("shape").unwrap(), "hexagon");
        // Rectangle is default -- no shape attr
        assert!(graph.nodes["D"].attrs.get("shape").is_none());
        assert_eq!(graph.nodes["E"].attrs.get("shape").unwrap(), "stadium");
    }

    #[test]
    fn edge_labels() {
        let input = "flowchart LR\n    A -->|uses| B\n    A --> C\n";
        let graph = parse(input).unwrap();

        assert_eq!(graph.edges[0].label.as_deref(), Some("uses"));
        assert_eq!(graph.edges[1].label, None);
    }

    #[test]
    fn edge_styles() {
        let input = "flowchart LR\n    A --> B\n    A -.-> C\n    A ==> D\n";
        let graph = parse(input).unwrap();

        assert!(graph.edges[0].attrs.get("style").is_none());
        assert_eq!(graph.edges[1].attrs.get("style").unwrap(), "dotted");
        assert_eq!(graph.edges[2].attrs.get("style").unwrap(), "thick");
    }

    #[test]
    fn direction_variants() {
        for (input_dir, expected) in [
            ("TD", "TD"),
            ("TB", "TD"), // TB and TD both map to TopDown
            ("LR", "LR"),
            ("RL", "RL"),
            ("BT", "BT"),
        ] {
            let input = format!("flowchart {input_dir}\n    A --> B\n");
            let graph = parse(&input).unwrap();
            assert_eq!(
                graph.attrs.get("direction").unwrap(),
                expected,
                "direction mismatch for {input_dir}"
            );
        }
    }
}
