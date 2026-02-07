use std::io::Write;

use crate::DepGraph;

/// DOT double-quoted string: wrap in `"…"` and escape `\` and `"`.
fn quote(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// Returns true if `s` is a bare DOT identifier (alphanumeric + underscore, not
/// digit-leading, and not a DOT reserved keyword).
fn is_bare_id(s: &str) -> bool {
    !s.is_empty()
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !s.starts_with(|c: char| c.is_ascii_digit())
        && !is_dot_keyword(s)
}

fn is_dot_keyword(s: &str) -> bool {
    matches!(
        s,
        "node" | "edge" | "graph" | "digraph" | "subgraph" | "strict"
    )
}

/// Quote a DOT ID: bare identifiers pass through, everything else gets double-quoted.
fn quote_id(s: &str) -> String {
    if is_bare_id(s) {
        s.to_string()
    } else {
        quote(s)
    }
}

pub fn emit(graph: &DepGraph, writer: &mut dyn Write) -> eyre::Result<()> {
    writeln!(writer, "digraph {{")?;

    for (id, info) in &graph.nodes {
        let mut attrs = Vec::new();
        if let Some(label) = &info.label {
            attrs.push(format!("label={}", quote(label)));
        }
        if let Some(node_type) = &info.node_type {
            attrs.push(format!("type={}", quote(node_type)));
        }
        for (k, v) in &info.attrs {
            attrs.push(format!("{}={}", quote_id(k), quote(v)));
        }

        if attrs.is_empty() {
            writeln!(writer, "    {};", quote_id(id))?;
        } else {
            writeln!(writer, "    {} [{}];", quote_id(id), attrs.join(", "))?;
        }
    }

    for edge in &graph.edges {
        if let Some(label) = &edge.label {
            writeln!(
                writer,
                "    {} -> {} [label={}];",
                quote_id(&edge.from),
                quote_id(&edge.to),
                quote(label)
            )?;
        } else {
            writeln!(
                writer,
                "    {} -> {};",
                quote_id(&edge.from),
                quote_id(&edge.to)
            )?;
        }
    }

    writeln!(writer, "}}")?;
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
    fn empty_graph() {
        let output = emit_to_string(&DepGraph::default());
        assert_eq!(output, "digraph {\n}\n");
    }

    #[test]
    fn sample() {
        let output = emit_to_string(&sample_graph());
        assert_eq!(
            output,
            "\
digraph {
    a [label=\"alpha\"];
    b [label=\"bravo\"];
    c;
    a -> b [label=\"depends\"];
    b -> c;
    a -> c;
}
"
        );
    }

    #[test]
    fn nodes_only() {
        let mut nodes = IndexMap::new();
        nodes.insert(
            "x".into(),
            NodeInfo {
                label: Some("X Node".into()),
                ..Default::default()
            },
        );
        nodes.insert("y".into(), NodeInfo::default());
        let graph = DepGraph {
            nodes,
            edges: vec![],
        };
        let output = emit_to_string(&graph);
        assert_eq!(
            output,
            "\
digraph {
    x [label=\"X Node\"];
    y;
}
"
        );
    }

    #[test]
    fn node_type_and_attrs() {
        let mut nodes = IndexMap::new();
        nodes.insert(
            "mylib".into(),
            NodeInfo {
                label: Some("My Library".into()),
                node_type: Some("lib".into()),
                attrs: IndexMap::from([("version".into(), "1.0".into())]),
            },
        );
        let graph = DepGraph {
            nodes,
            edges: vec![],
        };
        let output = emit_to_string(&graph);
        assert_eq!(
            output,
            "\
digraph {
    mylib [label=\"My Library\", type=\"lib\", version=\"1.0\"];
}
"
        );
    }

    #[test]
    fn special_chars_in_ids() {
        let mut nodes = IndexMap::new();
        nodes.insert("my node".into(), NodeInfo::default());
        nodes.insert(
            "has\"quotes".into(),
            NodeInfo {
                label: Some("a \"label\"".into()),
                ..Default::default()
            },
        );
        let graph = DepGraph {
            nodes,
            edges: vec![Edge {
                from: "my node".into(),
                to: "has\"quotes".into(),
                label: None,
            }],
        };
        let output = emit_to_string(&graph);
        assert_eq!(
            output,
            "\
digraph {
    \"my node\";
    \"has\\\"quotes\" [label=\"a \\\"label\\\"\"];
    \"my node\" -> \"has\\\"quotes\";
}
"
        );
    }

    #[test]
    fn bare_ids_not_quoted() {
        let mut nodes = IndexMap::new();
        nodes.insert("foo_bar".into(), NodeInfo::default());
        nodes.insert("Baz123".into(), NodeInfo::default());
        let graph = DepGraph {
            nodes,
            edges: vec![Edge {
                from: "foo_bar".into(),
                to: "Baz123".into(),
                label: None,
            }],
        };
        let output = emit_to_string(&graph);
        assert_eq!(
            output,
            "\
digraph {
    foo_bar;
    Baz123;
    foo_bar -> Baz123;
}
"
        );
    }

    #[test]
    fn dot_keyword_ids_are_quoted() {
        let mut nodes = IndexMap::new();
        nodes.insert("node".into(), NodeInfo::default());
        nodes.insert("edge".into(), NodeInfo::default());
        let graph = DepGraph {
            nodes,
            edges: vec![Edge {
                from: "node".into(),
                to: "edge".into(),
                label: None,
            }],
        };
        let output = emit_to_string(&graph);
        assert_eq!(
            output,
            "\
digraph {
    \"node\";
    \"edge\";
    \"node\" -> \"edge\";
}
"
        );
    }

    #[test]
    fn digit_leading_id_is_quoted() {
        let mut nodes = IndexMap::new();
        nodes.insert("1abc".into(), NodeInfo::default());
        let graph = DepGraph {
            nodes,
            edges: vec![],
        };
        let output = emit_to_string(&graph);
        assert_eq!(
            output,
            "\
digraph {
    \"1abc\";
}
"
        );
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
                },
                Edge {
                    from: "a".into(),
                    to: "c".into(),
                    label: Some("has space".into()),
                },
            ],
        };
        let output = emit_to_string(&graph);
        assert_eq!(
            output,
            "\
digraph {
    a -> b [label=\"uses\"];
    a -> c [label=\"has space\"];
}
"
        );
    }
}
