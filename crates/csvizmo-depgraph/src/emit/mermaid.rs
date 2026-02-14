use std::io::Write;

use crate::DepGraph;

/// Sanitize a node ID to be a valid Mermaid identifier.
///
/// Mermaid IDs support alphanumeric, underscore, dash, and dot characters,
/// and must not start with a digit. Spaces are replaced with underscores.
/// Other characters are replaced with `_XX` hex encoding.
fn sanitize_id(id: &str) -> String {
    if is_bare_id(id) {
        return id.to_string();
    }
    let mut result = String::new();
    for c in id.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
            result.push(c);
        } else if c == ' ' {
            result.push('_');
        } else {
            for b in c.to_string().as_bytes() {
                result.push_str(&format!("_{b:02x}"));
            }
        }
    }
    if result.starts_with(|c: char| c.is_ascii_digit()) {
        result.insert(0, '_');
    }
    result
}

fn is_bare_id(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        && !s.starts_with(|c: char| c.is_ascii_digit())
}

/// Escape a label string for use inside a quoted Mermaid label (`"..."`).
///
/// Labels are always wrapped in double quotes inside shape brackets, so
/// Mermaid syntax characters like `[]{}()|` are safe. Only `"` and `&`
/// need escaping via HTML named entities.
fn escape_label(s: &str) -> String {
    s.replace('&', "&amp;").replace('"', "&quot;")
}

/// Map a DOT shape attribute to a Mermaid shape bracket.
///
/// Returns None if the shape doesn't have a good Mermaid equivalent.
fn dot_shape_to_mermaid(shape: &str, label: &str) -> Option<String> {
    match shape {
        "box" => Some(format!("[\"{label}\"]")),
        "circle" => Some(format!("((\"{label}\"))")),
        "ellipse" => Some(format!("([\"{label}\"])")),
        "diamond" => Some(format!("{{\"{label}\"}}")),
        "hexagon" => Some(format!("{{{{\"{label}\"}}}}")),
        "note" => Some(format!("[/\"{label}\"/]")),
        "cylinder" => Some(format!("[(\"{label}\")]")),
        _ => None,
    }
}

/// Choose node shape brackets based on shape attr.
///
/// Mermaid supports various shapes via bracket syntax:
/// - `[label]` - rectangle (default)
/// - `([label])` - stadium/pill shape
/// - `((label))` - circle
/// - `{label}` - rhombus/diamond
/// - `{{label}}` - hexagon
/// - `[/label/]` - parallelogram
/// - `[(label)]` - cylindrical (database)
///
/// The `shape` attr is expected to be populated by `apply_default_styles()`
/// before emitting, so semantic types like "proc-macro" are already mapped
/// to DOT shape names (e.g. "diamond") by the time this runs.
fn node_shape(info: &crate::NodeInfo, label: &str) -> String {
    if let Some(shape) = info.attrs.get("shape")
        && let Some(mermaid_shape) = dot_shape_to_mermaid(shape, label)
    {
        return mermaid_shape;
    }

    // Default to rectangle
    format!("[\"{label}\"]")
}

/// Emit a [`DepGraph`] as a Mermaid flowchart.
///
/// Preserves:
/// - Graph direction from `rankdir` attr (LR, RL, TB, BT, TD)
/// - Node labels (escaped for Mermaid syntax)
/// - Node types as shape hints (lossy mapping to Mermaid shapes)
/// - Edge labels
/// - Subgraphs as nested `subgraph ... end` blocks
///
/// Drops:
/// - Graph-level attrs (except rankdir for direction)
/// - Arbitrary node attrs (no Mermaid syntax for them)
/// - Arbitrary edge attrs (no Mermaid syntax for them)
pub fn emit(graph: &DepGraph, writer: &mut dyn Write) -> eyre::Result<()> {
    let direction = graph
        .attrs
        .get("rankdir")
        .or(graph.attrs.get("direction"))
        .map(|s| s.as_str())
        .unwrap_or("LR");

    writeln!(writer, "flowchart {direction}")?;
    emit_body(graph, writer, 1)?;
    Ok(())
}

/// Emit the body of a graph or subgraph: subgraphs, nodes, edges.
fn emit_body(graph: &DepGraph, writer: &mut dyn Write, depth: usize) -> eyre::Result<()> {
    let indent = "    ".repeat(depth);

    // Emit subgraphs before nodes/edges.
    for sg in &graph.subgraphs {
        emit_subgraph(sg, writer, depth)?;
    }

    // Emit nodes (only if they have labels or types that affect shape).
    // Mermaid doesn't require explicit node declarations if they appear
    // in edges, but we emit them to show labels and shapes.
    for (id, info) in &graph.nodes {
        let sanitized = sanitize_id(id);
        let label = &info.label;
        let escaped = escape_label(label);
        let shape = node_shape(info, &escaped);
        writeln!(writer, "{indent}{sanitized}{shape}")?;
    }

    // Emit edges.
    for edge in &graph.edges {
        emit_edge(edge, writer, depth)?;
    }

    Ok(())
}

fn emit_edge(edge: &crate::Edge, writer: &mut dyn Write, depth: usize) -> eyre::Result<()> {
    let indent = "    ".repeat(depth);
    let from = sanitize_id(&edge.from);
    let to = sanitize_id(&edge.to);

    if let Some(label) = &edge.label {
        let escaped = escape_label(label);
        writeln!(writer, "{indent}{from} -->|\"{escaped}\"| {to}")?;
    } else {
        writeln!(writer, "{indent}{from} --> {to}")?;
    }

    Ok(())
}

fn emit_subgraph(sg: &DepGraph, writer: &mut dyn Write, depth: usize) -> eyre::Result<()> {
    let indent = "    ".repeat(depth);

    if let Some(id) = &sg.id {
        let sanitized = sanitize_id(id);
        writeln!(writer, "{indent}subgraph {sanitized}")?;
    } else {
        // Anonymous subgraphs not well-supported in Mermaid; use a generic name
        writeln!(writer, "{indent}subgraph sg{depth}")?;
    }

    emit_body(sg, writer, depth + 1)?;

    writeln!(writer, "{indent}end")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;

    use super::*;
    use crate::emit::fixtures::sample_graph;
    use crate::{Edge, NodeInfo};

    fn emit_to_string(graph: &DepGraph) -> String {
        let mut buf = Vec::new();
        emit(graph, &mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn sanitize_bare_id() {
        assert_eq!(sanitize_id("foo"), "foo");
        assert_eq!(sanitize_id("foo_bar"), "foo_bar");
        assert_eq!(sanitize_id("foo-bar"), "foo-bar");
        assert_eq!(sanitize_id("Foo123"), "Foo123");
    }

    #[test]
    fn sanitize_non_bare_id() {
        assert_eq!(sanitize_id("my node"), "my_node");
        assert_eq!(sanitize_id("has\"quotes"), "has_22quotes");
        assert_eq!(sanitize_id("123abc"), "_123abc");
        assert_eq!(sanitize_id("main.o"), "main.o");
        assert_eq!(sanitize_id("myapp v0.1.0"), "myapp_v0.1.0");
    }

    #[test]
    fn escape_label_basic() {
        assert_eq!(escape_label("hello"), "hello");
    }

    #[test]
    fn escape_label_brackets_preserved() {
        assert_eq!(escape_label("foo[bar]"), "foo[bar]");
    }

    #[test]
    fn escape_label_quotes() {
        assert_eq!(escape_label("say \"hi\""), "say &quot;hi&quot;");
    }

    #[test]
    fn escape_label_ampersand() {
        assert_eq!(escape_label("A & B"), "A &amp; B");
    }

    #[test]
    fn empty_graph() {
        let output = emit_to_string(&DepGraph::default());
        assert_eq!(output, "flowchart LR\n");
    }

    #[test]
    fn sample() {
        let output = emit_to_string(&sample_graph());
        assert_eq!(
            output,
            "\
flowchart LR
    a[\"alpha\"]
    b[\"bravo\"]
    c[\"c\"]
    a -->|\"depends\"| b
    b --> c
    a --> c
"
        );
    }

    #[test]
    fn nodes_only() {
        let mut nodes = IndexMap::new();
        nodes.insert("x".into(), NodeInfo::new("X Node"));
        nodes.insert("y".into(), NodeInfo::new("y"));
        let graph = DepGraph {
            nodes,
            edges: vec![],
            ..Default::default()
        };
        let output = emit_to_string(&graph);
        assert_eq!(
            output,
            "\
flowchart LR
    x[\"X Node\"]
    y[\"y\"]
"
        );
    }

    #[test]
    fn shape_attrs_from_node_types() {
        let mut nodes = IndexMap::new();
        nodes.insert(
            "lib1".into(),
            NodeInfo {
                label: "Library".into(),
                node_type: None,
                attrs: IndexMap::from([("shape".into(), "ellipse".into())]),
            },
        );
        nodes.insert(
            "bin1".into(),
            NodeInfo {
                label: "Binary".into(),
                node_type: None,
                attrs: IndexMap::from([("shape".into(), "box".into())]),
            },
        );
        nodes.insert(
            "pm1".into(),
            NodeInfo {
                label: "Proc Macro".into(),
                node_type: None,
                attrs: IndexMap::from([("shape".into(), "diamond".into())]),
            },
        );
        nodes.insert(
            "bs1".into(),
            NodeInfo {
                label: "Build Script".into(),
                node_type: None,
                attrs: IndexMap::from([("shape".into(), "note".into())]),
            },
        );
        nodes.insert(
            "test1".into(),
            NodeInfo {
                label: "Test".into(),
                node_type: None,
                attrs: IndexMap::from([("shape".into(), "hexagon".into())]),
            },
        );
        let graph = DepGraph {
            nodes,
            ..Default::default()
        };
        let output = emit_to_string(&graph);
        assert!(output.contains("lib1([\"Library\"])"));
        assert!(output.contains("bin1[\"Binary\"]"));
        assert!(output.contains("pm1{\"Proc Macro\"}"));
        assert!(output.contains("bs1[/\"Build Script\"/]"));
        assert!(output.contains("test1{{\"Test\"}}"));
    }

    #[test]
    fn edge_labels() {
        let graph = DepGraph {
            nodes: IndexMap::new(),
            edges: vec![
                Edge {
                    from: "a".into(),
                    to: "b".into(),
                    label: Some("uses".into()),
                    ..Default::default()
                },
                Edge {
                    from: "a".into(),
                    to: "c".into(),
                    label: Some("has space".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let output = emit_to_string(&graph);
        assert!(output.contains("a -->|\"uses\"| b"));
        assert!(output.contains("a -->|\"has space\"| c"));
    }

    #[test]
    fn direction_from_rankdir() {
        let graph = DepGraph {
            attrs: IndexMap::from([("rankdir".into(), "TB".into())]),
            nodes: IndexMap::from([("a".into(), NodeInfo::new("a"))]),
            ..Default::default()
        };
        let output = emit_to_string(&graph);
        assert!(output.starts_with("flowchart TB\n"));
    }

    #[test]
    fn direction_from_direction_attr() {
        let graph = DepGraph {
            attrs: IndexMap::from([("direction".into(), "RL".into())]),
            nodes: IndexMap::from([("a".into(), NodeInfo::new("a"))]),
            ..Default::default()
        };
        let output = emit_to_string(&graph);
        assert!(output.starts_with("flowchart RL\n"));
    }

    #[test]
    fn subgraph_emitted() {
        let graph = DepGraph {
            nodes: IndexMap::from([("top".into(), NodeInfo::new("top"))]),
            subgraphs: vec![DepGraph {
                id: Some("backend".into()),
                nodes: IndexMap::from([
                    ("api".into(), NodeInfo::new("API Server")),
                    ("db".into(), NodeInfo::new("Database")),
                ]),
                edges: vec![Edge {
                    from: "api".into(),
                    to: "db".into(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            edges: vec![Edge {
                from: "top".into(),
                to: "api".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let output = emit_to_string(&graph);
        assert_eq!(
            output,
            "\
flowchart LR
    subgraph backend
        api[\"API Server\"]
        db[\"Database\"]
        api --> db
    end
    top[\"top\"]
    top --> api
"
        );
    }

    #[test]
    fn special_chars_in_ids() {
        let mut nodes = IndexMap::new();
        nodes.insert("my node".into(), NodeInfo::new("my node"));
        nodes.insert("has\"quotes".into(), NodeInfo::new("a \"label\""));
        let graph = DepGraph {
            nodes,
            edges: vec![Edge {
                from: "my node".into(),
                to: "has\"quotes".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let output = emit_to_string(&graph);
        assert!(output.contains("my_node[\"my node\"]"));
        assert!(output.contains("has_22quotes[\"a &quot;label&quot;\"]"));
        assert!(output.contains("my_node --> has_22quotes"));
    }

    #[test]
    fn node_attrs_dropped() {
        let mut nodes = IndexMap::new();
        nodes.insert(
            "a".into(),
            NodeInfo {
                label: "Alpha".into(),
                node_type: None,
                attrs: IndexMap::from([
                    ("shape".into(), "box".into()),
                    ("color".into(), "red".into()),
                ]),
            },
        );
        let graph = DepGraph {
            nodes,
            ..Default::default()
        };
        let output = emit_to_string(&graph);
        // Mermaid doesn't support arbitrary attrs in basic syntax, so they're dropped
        assert!(output.contains("a[\"Alpha\"]"));
        assert!(!output.contains("shape"));
        assert!(!output.contains("color"));
    }

    #[test]
    fn edge_attrs_dropped() {
        let graph = DepGraph {
            nodes: IndexMap::new(),
            edges: vec![Edge {
                from: "a".into(),
                to: "b".into(),
                label: Some("uses".into()),
                attrs: IndexMap::from([("style".into(), "dashed".into())]),
            }],
            ..Default::default()
        };
        let output = emit_to_string(&graph);
        assert!(output.contains("a -->|\"uses\"| b"));
        assert!(!output.contains("dashed"));
    }

    #[test]
    fn shape_attrs_mapped_to_mermaid() {
        let mut nodes = IndexMap::new();
        nodes.insert(
            "n1".into(),
            NodeInfo {
                label: "Circle".into(),
                node_type: None,
                attrs: IndexMap::from([("shape".into(), "circle".into())]),
            },
        );
        nodes.insert(
            "n2".into(),
            NodeInfo {
                label: "Diamond".into(),
                node_type: None,
                attrs: IndexMap::from([("shape".into(), "diamond".into())]),
            },
        );
        nodes.insert(
            "n3".into(),
            NodeInfo {
                label: "Hexagon".into(),
                node_type: None,
                attrs: IndexMap::from([("shape".into(), "hexagon".into())]),
            },
        );
        nodes.insert(
            "n4".into(),
            NodeInfo {
                label: "Ellipse".into(),
                node_type: None,
                attrs: IndexMap::from([("shape".into(), "ellipse".into())]),
            },
        );
        nodes.insert(
            "n5".into(),
            NodeInfo {
                label: "Cylinder".into(),
                node_type: None,
                attrs: IndexMap::from([("shape".into(), "cylinder".into())]),
            },
        );
        let graph = DepGraph {
            nodes,
            ..Default::default()
        };
        let output = emit_to_string(&graph);
        assert!(output.contains("n1((\"Circle\"))"));
        assert!(output.contains("n2{\"Diamond\"}"));
        assert!(output.contains("n3{{\"Hexagon\"}}"));
        assert!(output.contains("n4([\"Ellipse\"])"));
        assert!(output.contains("n5[(\"Cylinder\")]"));
    }

    #[test]
    fn explicit_shape_attr_used() {
        let mut nodes = IndexMap::new();
        nodes.insert(
            "lib1".into(),
            NodeInfo {
                label: "Library".into(),
                node_type: None,
                attrs: IndexMap::from([("shape".into(), "box".into())]),
            },
        );
        let graph = DepGraph {
            nodes,
            ..Default::default()
        };
        let output = emit_to_string(&graph);
        // shape=box maps to rectangle brackets
        assert!(output.contains("lib1[\"Library\"]"));
    }

    #[test]
    fn unknown_shape_falls_back_to_rectangle() {
        let mut nodes = IndexMap::new();
        nodes.insert(
            "n1".into(),
            NodeInfo {
                label: "Unknown".into(),
                node_type: None,
                attrs: IndexMap::from([("shape".into(), "trapezium".into())]),
            },
        );
        let graph = DepGraph {
            nodes,
            ..Default::default()
        };
        let output = emit_to_string(&graph);
        // Unknown shape should fall back to rectangle
        assert!(output.contains("n1[\"Unknown\"]"));
    }
}
