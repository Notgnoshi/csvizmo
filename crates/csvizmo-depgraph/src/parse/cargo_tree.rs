use indexmap::IndexMap;

use crate::{DepGraph, Edge, NodeInfo};

/// Parse a line of `cargo tree` output into (depth, text), or None for blank lines
/// and section headers like `[dev-dependencies]`.
fn parse_line(line: &str) -> Option<(usize, &str)> {
    let mut depth = 0;
    let mut rest = line;

    loop {
        // Unicode branch markers (terminal)
        if rest.starts_with("├── ") || rest.starts_with("└── ") {
            rest = &rest[10..];
            depth += 1;
            break;
        }
        // Unicode continuation
        if rest.starts_with("│   ") {
            rest = &rest[6..];
            depth += 1;
            continue;
        }
        // ASCII branch markers (terminal)
        if rest.starts_with("|-- ") || rest.starts_with("`-- ") || rest.starts_with("\\-- ") {
            rest = &rest[4..];
            depth += 1;
            break;
        }
        // ASCII continuation (pipe + 3 spaces) or blank continuation (last-child ancestor)
        if rest.starts_with("|   ") || rest.starts_with("    ") {
            rest = &rest[4..];
            depth += 1;
            continue;
        }
        break;
    }

    let text = rest.trim_end();
    if text.is_empty() || text.starts_with('[') {
        None
    } else {
        Some((depth, text))
    }
}

/// Parse the text portion of a cargo tree line, returning (id, label, is_dup, attrs).
///
/// The node ID is the crate name (without version). The version is stored in
/// `attrs["version"]`. Strips trailing `(*)` duplicate markers and parenthesized
/// annotations like `(proc-macro)`, `(build)`, `(dev)`, and local paths.
fn parse_node_text(text: &str) -> (&str, bool, IndexMap<String, String>) {
    let mut rest = text;
    let mut is_dup = false;
    let mut attrs = IndexMap::new();

    // Strip trailing (*) duplicate marker
    if let Some(stripped) = rest.strip_suffix("(*)") {
        rest = stripped.trim_end();
        is_dup = true;
    }

    // Strip parenthesized annotations from the end
    while rest.ends_with(')') {
        let Some(open) = rest.rfind('(') else { break };
        let annotation = &rest[open + 1..rest.len() - 1];
        let before = rest[..open].trim_end();

        match annotation {
            "proc-macro" | "build" | "dev" => {
                attrs.insert("kind".into(), annotation.into());
            }
            _ if annotation.contains('/') || annotation.starts_with('.') => {
                attrs.insert("path".into(), annotation.into());
            }
            _ => break,
        }
        rest = before;
    }

    // Extract version into attrs, but keep "name v1.2.3" as the full ID
    // for disambiguation (multiple versions of the same crate can coexist)
    if let Some((_, version)) = split_name_version(rest) {
        attrs.insert("version".into(), version.into());
    }

    (rest, is_dup, attrs)
}

/// Split `"name v1.2.3"` into `("name", "v1.2.3")`, or None if no version token.
fn split_name_version(text: &str) -> Option<(&str, &str)> {
    for (i, _) in text.match_indices(" v") {
        let version = &text[i + 1..];
        if version[1..].starts_with(|c: char| c.is_ascii_digit()) {
            return Some((&text[..i], version));
        }
    }
    None
}

pub fn parse(input: &str) -> eyre::Result<DepGraph> {
    let mut graph = DepGraph::default();
    // stack[i] = node ID at depth i
    let mut stack: Vec<String> = Vec::new();

    for raw_line in input.lines() {
        // Normalize NO-BREAK SPACE (U+00A0) to ASCII space
        let owned;
        let line = if raw_line.contains('\u{a0}') {
            owned = raw_line.replace('\u{a0}', " ");
            owned.as_str()
        } else {
            raw_line
        };

        let (depth, text) = match parse_line(line) {
            Some(pair) => pair,
            None => continue,
        };

        if depth > stack.len() {
            eyre::bail!("unexpected depth jump at line: {text:?}");
        }

        let (name, _is_dup, attrs) = parse_node_text(text);
        let id = name.to_string();
        let label = match split_name_version(name) {
            Some((crate_name, _)) => crate_name.to_string(),
            None => name.to_string(),
        };

        stack.truncate(depth);

        // Insert node if not already present (handles duplicates and repeated leaves)
        if !graph.nodes.contains_key(&id) {
            graph.nodes.insert(
                id.clone(),
                NodeInfo {
                    label: Some(label),
                    attrs,
                },
            );
        }

        // Add edge from parent
        if let Some(parent) = stack.last() {
            graph.edges.push(Edge {
                from: parent.clone(),
                to: id.clone(),
                ..Default::default()
            });
        }

        stack.push(id);
    }

    Ok(graph)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input() {
        let graph = parse("").unwrap();
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn root_only() {
        let graph = parse("myapp v1.0.0\n").unwrap();
        assert_eq!(graph.nodes.len(), 1);
        let node = &graph.nodes["myapp v1.0.0"];
        assert_eq!(node.label.as_deref(), Some("myapp"));
        assert_eq!(
            node.attrs.get("version").map(|s| s.as_str()),
            Some("v1.0.0")
        );
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn simple_tree() {
        let input = "\
myapp v1.0.0
├── libfoo v0.2.1
└── libbar v0.1.0
";
        let graph = parse(input).unwrap();
        assert_eq!(graph.nodes.len(), 3);
        assert!(graph.nodes.contains_key("myapp v1.0.0"));
        assert!(graph.nodes.contains_key("libfoo v0.2.1"));
        assert!(graph.nodes.contains_key("libbar v0.1.0"));
        assert_eq!(graph.edges.len(), 2);
        assert_eq!(graph.edges[0].from, "myapp v1.0.0");
        assert_eq!(graph.edges[0].to, "libfoo v0.2.1");
        assert_eq!(graph.edges[1].from, "myapp v1.0.0");
        assert_eq!(graph.edges[1].to, "libbar v0.1.0");
    }

    #[test]
    fn nested_tree() {
        let input = "\
myapp v1.0.0
├── libfoo v0.2.1
│   └── libbar v0.1.0
└── libbaz v0.3.0
";
        let graph = parse(input).unwrap();
        assert_eq!(graph.nodes.len(), 4);
        assert_eq!(graph.edges.len(), 3);
        assert_eq!(graph.edges[0].from, "myapp v1.0.0");
        assert_eq!(graph.edges[0].to, "libfoo v0.2.1");
        assert_eq!(graph.edges[1].from, "libfoo v0.2.1");
        assert_eq!(graph.edges[1].to, "libbar v0.1.0");
        assert_eq!(graph.edges[2].from, "myapp v1.0.0");
        assert_eq!(graph.edges[2].to, "libbaz v0.3.0");
    }

    #[test]
    fn duplicate_star_marker() {
        let input = "\
myapp v1.0.0
├── libfoo v0.2.1
│   └── shared v1.0.0
└── libbar v0.1.0
    └── shared v1.0.0 (*)
";
        let graph = parse(input).unwrap();
        // "shared v1.0.0" should appear only once as a node
        assert_eq!(graph.nodes.len(), 4);
        assert!(graph.nodes.contains_key("shared v1.0.0"));
        // But there should be two edges pointing to it
        let shared_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.to == "shared v1.0.0")
            .collect();
        assert_eq!(shared_edges.len(), 2);
        assert_eq!(shared_edges[0].from, "libfoo v0.2.1");
        assert_eq!(shared_edges[1].from, "libbar v0.1.0");
    }

    #[test]
    fn repeated_leaf_no_star() {
        // Leaf nodes can appear multiple times without (*) since they have no subtree
        let input = "\
myapp v1.0.0
├── libfoo v0.2.1
│   └── leaf v1.0.0
└── libbar v0.1.0
    └── leaf v1.0.0
";
        let graph = parse(input).unwrap();
        assert_eq!(graph.nodes.len(), 4);
        let leaf_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.to == "leaf v1.0.0")
            .collect();
        assert_eq!(leaf_edges.len(), 2);
    }

    #[test]
    fn proc_macro_kind() {
        let input = "\
myapp v1.0.0
└── derive-thing v0.5.0 (proc-macro)
";
        let graph = parse(input).unwrap();
        assert_eq!(graph.nodes.len(), 2);
        let node = &graph.nodes["derive-thing v0.5.0"];
        assert_eq!(
            node.attrs.get("kind").map(|s| s.as_str()),
            Some("proc-macro")
        );
        assert_eq!(node.label.as_deref(), Some("derive-thing"));
        assert_eq!(
            node.attrs.get("version").map(|s| s.as_str()),
            Some("v0.5.0")
        );
    }

    #[test]
    fn local_path_attr() {
        let input = "\
myapp v1.0.0 (my/workspace/path)
└── mylib v0.1.0 (my/workspace/lib)
";
        let graph = parse(input).unwrap();
        assert_eq!(
            graph.nodes["myapp v1.0.0"]
                .attrs
                .get("path")
                .map(|s| s.as_str()),
            Some("my/workspace/path")
        );
        assert_eq!(
            graph.nodes["mylib v0.1.0"]
                .attrs
                .get("path")
                .map(|s| s.as_str()),
            Some("my/workspace/lib")
        );
    }

    #[test]
    fn proc_macro_with_path() {
        let input = "\
myapp v1.0.0
└── mymacro v0.1.0 (my/path) (proc-macro)
";
        let graph = parse(input).unwrap();
        let node = &graph.nodes["mymacro v0.1.0"];
        assert_eq!(
            node.attrs.get("kind").map(|s| s.as_str()),
            Some("proc-macro")
        );
        assert_eq!(node.attrs.get("path").map(|s| s.as_str()), Some("my/path"));
    }

    #[test]
    fn dev_dependencies_section() {
        let input = "\
myapp v1.0.0
└── libfoo v0.2.1
[dev-dependencies]
└── testlib v1.0.0
";
        let graph = parse(input).unwrap();
        assert_eq!(graph.nodes.len(), 3);
        // Both libfoo and testlib are children of myapp
        assert_eq!(graph.edges.len(), 2);
        assert_eq!(graph.edges[0].from, "myapp v1.0.0");
        assert_eq!(graph.edges[0].to, "libfoo v0.2.1");
        assert_eq!(graph.edges[1].from, "myapp v1.0.0");
        assert_eq!(graph.edges[1].to, "testlib v1.0.0");
    }

    #[test]
    fn build_dependencies_section() {
        let input = "\
myapp v1.0.0
└── libfoo v0.2.1
[build-dependencies]
└── buildlib v1.0.0
";
        let graph = parse(input).unwrap();
        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.edges.len(), 2);
        assert_eq!(graph.edges[1].from, "myapp v1.0.0");
        assert_eq!(graph.edges[1].to, "buildlib v1.0.0");
    }

    #[test]
    fn feature_entries() {
        let input = "\
myapp v1.0.0
├── clap feature \"default\"
│   └── clap v4.5.57
└── clap feature \"derive\"
    └── clap v4.5.57 (*)
";
        let graph = parse(input).unwrap();
        assert!(graph.nodes.contains_key("clap feature \"default\""));
        assert!(graph.nodes.contains_key("clap feature \"derive\""));
        assert!(graph.nodes.contains_key("clap v4.5.57"));
        // clap v4.5.57 appears only once as a node despite two references
        assert_eq!(
            graph
                .nodes
                .keys()
                .filter(|k| k.starts_with("clap v"))
                .count(),
            1
        );
        // Two edges to clap v4.5.57
        let clap_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.to == "clap v4.5.57")
            .collect();
        assert_eq!(clap_edges.len(), 2);
    }

    // -- parse_line unit tests --

    #[test]
    fn parse_line_root() {
        assert_eq!(parse_line("myapp v1.0.0"), Some((0, "myapp v1.0.0")));
    }

    #[test]
    fn parse_line_unicode_depth() {
        assert_eq!(parse_line("├── child v1.0.0"), Some((1, "child v1.0.0")));
        assert_eq!(
            parse_line("│   └── grandchild v1.0.0"),
            Some((2, "grandchild v1.0.0"))
        );
    }

    #[test]
    fn parse_line_ascii_depth() {
        assert_eq!(parse_line("|-- child v1.0.0"), Some((1, "child v1.0.0")));
        assert_eq!(
            parse_line("|   `-- grandchild v1.0.0"),
            Some((2, "grandchild v1.0.0"))
        );
    }

    #[test]
    fn parse_line_blank_continuation() {
        // When parent is last child, uses spaces instead of pipe
        assert_eq!(
            parse_line("    └── child v1.0.0"),
            Some((2, "child v1.0.0"))
        );
    }

    #[test]
    fn parse_line_skips_blank() {
        assert_eq!(parse_line(""), None);
        assert_eq!(parse_line("   "), None);
    }

    #[test]
    fn parse_line_skips_section_headers() {
        assert_eq!(parse_line("[dev-dependencies]"), None);
        assert_eq!(parse_line("[build-dependencies]"), None);
    }

    // -- parse_node_text unit tests --

    #[test]
    fn parse_node_text_simple() {
        let (id, is_dup, attrs) = parse_node_text("clap v4.5.57");
        assert_eq!(id, "clap v4.5.57");
        assert!(!is_dup);
        assert_eq!(attrs.get("version").map(|s| s.as_str()), Some("v4.5.57"));
    }

    #[test]
    fn parse_node_text_dup() {
        let (id, is_dup, attrs) = parse_node_text("clap v4.5.57 (*)");
        assert_eq!(id, "clap v4.5.57");
        assert!(is_dup);
        assert_eq!(attrs.get("version").map(|s| s.as_str()), Some("v4.5.57"));
    }

    #[test]
    fn parse_node_text_proc_macro() {
        let (id, is_dup, attrs) = parse_node_text("clap_derive v4.5.55 (proc-macro)");
        assert_eq!(id, "clap_derive v4.5.55");
        assert!(!is_dup);
        assert_eq!(attrs.get("kind").map(|s| s.as_str()), Some("proc-macro"));
        assert_eq!(attrs.get("version").map(|s| s.as_str()), Some("v4.5.55"));
    }

    #[test]
    fn parse_node_text_proc_macro_dup() {
        let (id, is_dup, attrs) = parse_node_text("clap_derive v4.5.55 (proc-macro) (*)");
        assert_eq!(id, "clap_derive v4.5.55");
        assert!(is_dup);
        assert_eq!(attrs.get("kind").map(|s| s.as_str()), Some("proc-macro"));
        assert_eq!(attrs.get("version").map(|s| s.as_str()), Some("v4.5.55"));
    }

    #[test]
    fn parse_node_text_path() {
        let (id, is_dup, attrs) = parse_node_text("myapp v1.0.0 (my/workspace/path)");
        assert_eq!(id, "myapp v1.0.0");
        assert!(!is_dup);
        assert_eq!(
            attrs.get("path").map(|s| s.as_str()),
            Some("my/workspace/path")
        );
        assert_eq!(attrs.get("version").map(|s| s.as_str()), Some("v1.0.0"));
    }

    #[test]
    fn parse_node_text_path_and_proc_macro() {
        let (id, is_dup, attrs) = parse_node_text("mymacro v0.1.0 (my/path) (proc-macro)");
        assert_eq!(id, "mymacro v0.1.0");
        assert!(!is_dup);
        assert_eq!(attrs.get("kind").map(|s| s.as_str()), Some("proc-macro"));
        assert_eq!(attrs.get("path").map(|s| s.as_str()), Some("my/path"));
        assert_eq!(attrs.get("version").map(|s| s.as_str()), Some("v0.1.0"));
    }

    #[test]
    fn parse_node_text_build_kind() {
        let (id, _, attrs) = parse_node_text("cc v1.0.0 (build)");
        assert_eq!(id, "cc v1.0.0");
        assert_eq!(attrs.get("kind").map(|s| s.as_str()), Some("build"));
        assert_eq!(attrs.get("version").map(|s| s.as_str()), Some("v1.0.0"));
    }

    #[test]
    fn parse_node_text_dev_kind() {
        let (id, _, attrs) = parse_node_text("testlib v1.0.0 (dev)");
        assert_eq!(id, "testlib v1.0.0");
        assert_eq!(attrs.get("kind").map(|s| s.as_str()), Some("dev"));
        assert_eq!(attrs.get("version").map(|s| s.as_str()), Some("v1.0.0"));
    }

    #[test]
    fn parse_node_text_feature_entry() {
        let (id, is_dup, attrs) = parse_node_text("clap feature \"default\"");
        assert_eq!(id, "clap feature \"default\"");
        assert!(!is_dup);
        assert!(attrs.get("version").is_none());
    }

    // -- fixture tests --

    #[test]
    fn fixture_cargo_tree() {
        let input = include_str!("../../../../data/depconv/cargo-tree.txt");
        let graph = parse(input).unwrap();

        // Root node: ID has version, label is just the name
        let root = &graph.nodes["csvizmo-depgraph v0.5.0"];
        assert_eq!(root.label.as_deref(), Some("csvizmo-depgraph"));
        assert_eq!(
            root.attrs.get("version").map(|s| s.as_str()),
            Some("v0.5.0")
        );
        assert_eq!(
            root.attrs.get("path").map(|s| s.as_str()),
            Some("csvizmo/crates/csvizmo-depgraph")
        );

        // Check a few specific nodes
        assert_eq!(graph.nodes["clap v4.5.57"].label.as_deref(), Some("clap"));
        assert_eq!(
            graph.nodes["clap_derive v4.5.55"]
                .attrs
                .get("kind")
                .map(|s| s.as_str()),
            Some("proc-macro")
        );

        // proc-macro2 appears multiple times with (*) but should be one node
        assert!(graph.nodes.contains_key("proc-macro2 v1.0.106"));

        // Multiple edges to proc-macro2 from different parents
        let pm2_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.to == "proc-macro2 v1.0.106")
            .collect();
        assert!(pm2_edges.len() >= 2);

        // Root has no incoming edges
        assert!(
            !graph
                .edges
                .iter()
                .any(|e| e.to == "csvizmo-depgraph v0.5.0")
        );

        // Spot-check a direct dependency edge
        assert!(
            graph
                .edges
                .iter()
                .any(|e| e.from == "csvizmo-depgraph v0.5.0" && e.to == "clap v4.5.57")
        );

        // Dev dependencies should be children of the root
        assert!(
            graph
                .edges
                .iter()
                .any(|e| e.from == "csvizmo-depgraph v0.5.0" && e.to == "csvizmo-test v0.5.0")
        );
    }

    #[test]
    fn fixture_cargo_tree_features() {
        let input = include_str!("../../../../data/depconv/cargo-tree-features.txt");
        let graph = parse(input).unwrap();

        // Root node
        assert!(graph.nodes.contains_key("csvizmo-depgraph v0.5.0"));

        // Feature nodes
        assert!(graph.nodes.contains_key("clap feature \"default\""));
        assert!(graph.nodes.contains_key("clap feature \"derive\""));

        // Regular nodes
        assert!(graph.nodes.contains_key("clap v4.5.57"));

        // Root -> feature edge
        assert!(
            graph
                .edges
                .iter()
                .any(|e| e.from == "csvizmo-depgraph v0.5.0" && e.to == "clap feature \"default\"")
        );
    }
}
