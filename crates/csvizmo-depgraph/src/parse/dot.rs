use dot_parser::{ast, canonical};

use crate::{DepGraph, Edge, NodeInfo};

/// Strip surrounding double-quotes and unescape `\"` and `\\`.
/// The `dot-parser` crate preserves quotes in its String output.
fn unquote(s: &str) -> String {
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        s[1..s.len() - 1]
            .replace("\\\"", "\"")
            .replace("\\\\", "\\")
    } else {
        s.to_string()
    }
}

pub fn parse(input: &str) -> eyre::Result<DepGraph> {
    let ast_graph = ast::Graph::try_from(input).map_err(|e| eyre::eyre!("DOT parse error: {e}"))?;
    let graph = canonical::Graph::from(ast_graph);

    let mut dep = DepGraph::default();

    // dot-parser uses HashMap internally so iteration order is non-deterministic.
    // Sort by node ID to ensure deterministic output.
    let mut sorted_nodes: Vec<_> = graph.nodes.set.iter().collect();
    sorted_nodes.sort_by_key(|(id, _)| *id);

    for (id, node) in sorted_nodes {
        let id = unquote(id);

        // Collect all attributes first so we can resolve type vs shape priority.
        let attrs: Vec<(String, String)> = node
            .attr
            .elems
            .iter()
            .map(|(k, v)| {
                let k: String = k.clone().into();
                let v: String = v.clone().into();
                (unquote(&k), unquote(&v))
            })
            .collect();

        let mut info = NodeInfo::default();
        let has_type = attrs.iter().any(|(k, _)| k == "type");

        for (key, value) in attrs {
            match key.as_str() {
                "label" => info.label = Some(value),
                "type" => info.node_type = Some(value),
                // shape is a fallback for node_type only when no explicit type attr exists.
                "shape" if !has_type => info.node_type = Some(value),
                _ => {
                    info.attrs.insert(key, value);
                }
            }
        }
        dep.nodes.insert(id, info);
    }

    for edge in &graph.edges.set {
        let from = unquote(&edge.from);
        let to = unquote(&edge.to);

        let mut label = None;
        for (key, value) in &edge.attr {
            let key: String = key.clone().into();
            if unquote(&key) == "label" {
                let v: String = value.clone().into();
                label = Some(unquote(&v));
            }
        }
        // Ensure both endpoints exist as nodes (DOT can define nodes implicitly via edges).
        for endpoint in [&from, &to] {
            dep.nodes
                .entry(endpoint.clone())
                .or_insert_with(NodeInfo::default);
        }
        dep.edges.push(Edge { from, to, label });
    }

    Ok(dep)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_digraph() {
        let graph = parse("digraph {}").unwrap();
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn named_digraph() {
        let graph = parse("digraph deps {}").unwrap();
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn simple_edge() {
        let graph = parse("digraph { a -> b; }").unwrap();
        assert_eq!(graph.nodes.len(), 2);
        assert!(graph.nodes.contains_key("a"));
        assert!(graph.nodes.contains_key("b"));
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].from, "a");
        assert_eq!(graph.edges[0].to, "b");
        assert_eq!(graph.edges[0].label, None);
    }

    #[test]
    fn node_labels() {
        let graph = parse(r#"digraph { a [label="Alpha"]; b [label="Bravo"]; a -> b; }"#).unwrap();
        assert_eq!(graph.nodes["a"].label.as_deref(), Some("Alpha"));
        assert_eq!(graph.nodes["b"].label.as_deref(), Some("Bravo"));
    }

    #[test]
    fn edge_labels() {
        let graph =
            parse(r#"digraph { a -> b [label="depends"]; a -> c [label="uses"]; }"#).unwrap();
        assert_eq!(graph.edges.len(), 2);
        assert_eq!(graph.edges[0].label.as_deref(), Some("depends"));
        assert_eq!(graph.edges[1].label.as_deref(), Some("uses"));
    }

    #[test]
    fn node_type_from_type_attr() {
        let graph = parse(r#"digraph { a [type="lib"]; }"#).unwrap();
        assert_eq!(graph.nodes["a"].node_type.as_deref(), Some("lib"));
    }

    #[test]
    fn node_type_from_shape_fallback() {
        let graph = parse(r#"digraph { a [shape=box]; }"#).unwrap();
        assert_eq!(graph.nodes["a"].node_type.as_deref(), Some("box"));
    }

    #[test]
    fn type_takes_priority_over_shape() {
        let graph = parse(r#"digraph { a [shape=box, type="lib"]; }"#).unwrap();
        assert_eq!(graph.nodes["a"].node_type.as_deref(), Some("lib"));
        // shape goes to attrs since type was used for node_type
        assert_eq!(
            graph.nodes["a"].attrs.get("shape").map(|s| s.as_str()),
            Some("box")
        );
    }

    #[test]
    fn extra_attrs_preserved() {
        let graph = parse(r#"digraph { a [label="A", color="red", style="bold"]; }"#).unwrap();
        assert_eq!(graph.nodes["a"].label.as_deref(), Some("A"));
        assert_eq!(
            graph.nodes["a"].attrs.get("color").map(|s| s.as_str()),
            Some("red")
        );
        assert_eq!(
            graph.nodes["a"].attrs.get("style").map(|s| s.as_str()),
            Some("bold")
        );
    }

    #[test]
    fn implicit_nodes_from_edges() {
        // Nodes only defined implicitly by edges should still appear in the graph.
        let graph = parse("digraph { a -> b -> c; }").unwrap();
        assert_eq!(graph.nodes.len(), 3);
        assert!(graph.nodes.contains_key("a"));
        assert!(graph.nodes.contains_key("b"));
        assert!(graph.nodes.contains_key("c"));
    }

    #[test]
    fn graph_attrs_ignored() {
        // Graph-level attributes like rankdir should not cause errors.
        let graph = parse(r#"digraph { rankdir=LR; a -> b; }"#).unwrap();
        assert_eq!(graph.edges.len(), 1);
    }

    #[test]
    fn quoted_ids() {
        let graph =
            parse(r#"digraph { "my node" [label="My Node"]; "my node" -> "other"; }"#).unwrap();
        assert!(graph.nodes.contains_key("my node"));
        assert_eq!(graph.nodes["my node"].label.as_deref(), Some("My Node"));
    }

    #[test]
    fn fixture_small_dot() {
        let input = include_str!("../../../../data/depconv/small.dot");
        let graph = parse(input).unwrap();
        assert_eq!(graph.nodes.len(), 3);
        assert!(graph.nodes.contains_key("myapp"));
        assert!(graph.nodes.contains_key("libfoo"));
        assert!(graph.nodes.contains_key("libbar"));
        assert_eq!(
            graph.nodes["myapp"].label.as_deref(),
            Some("My Application")
        );
        // shape=box => node_type
        assert_eq!(graph.nodes["myapp"].node_type.as_deref(), Some("box"));
        assert_eq!(graph.edges.len(), 3);
    }
}
