use crate::{DepGraph, Edge, NodeInfo};

/// Parse a line of `tree` output into (depth, name), or None for blank/summary lines.
fn parse_line(line: &str) -> Option<(usize, &str)> {
    let mut depth = 0;
    let mut rest = line;

    loop {
        // Unicode branch markers (last decoration before the name)
        if rest.starts_with("├── ") || rest.starts_with("└── ") {
            // 10 bytes: 3 (box char) + 3 + 3 (dashes) + 1 (space)
            rest = &rest[10..];
            depth += 1;
            break;
        }
        // Unicode continuation
        if rest.starts_with("│   ") {
            // 6 bytes: 3 (box char) + 3 (spaces)
            rest = &rest[6..];
            depth += 1;
            continue;
        }
        // ASCII branch markers
        if rest.starts_with("|-- ") || rest.starts_with("`-- ") || rest.starts_with("\\-- ") {
            rest = &rest[4..];
            depth += 1;
            break;
        }
        // ASCII continuation or blank continuation (last-child ancestor)
        if rest.starts_with("|   ") || rest.starts_with("    ") {
            rest = &rest[4..];
            depth += 1;
            continue;
        }
        break;
    }

    let name = rest.trim_end();
    // Strip trailing markers emitted by tree/cargo-tree for revisited or cyclic nodes.
    let name = name
        .strip_suffix("(*)")
        .or_else(|| name.strip_suffix("(cycle)"))
        .map(|n| n.trim_end())
        .unwrap_or(name);
    if name.is_empty() {
        None
    } else {
        Some((depth, name))
    }
}

/// Detect the summary line that `tree` appends (e.g. "26 directories, 40 files").
fn is_summary(line: &str) -> bool {
    let t = line.trim();
    t.starts_with(|c: char| c.is_ascii_digit()) && t.contains("director") && t.contains("file")
}

pub fn parse(input: &str) -> eyre::Result<DepGraph> {
    let mut graph = DepGraph::default();
    // stack[i] = node ID (full path) of the most recent node at depth i
    let mut stack: Vec<String> = Vec::new();

    for raw_line in input.lines() {
        // Some `tree` builds use NO-BREAK SPACE (U+00A0) in continuation prefixes.
        // Normalize to ASCII space so the fixed-width group matching works.
        let owned;
        let line = if raw_line.contains('\u{a0}') {
            owned = raw_line.replace('\u{a0}', " ");
            owned.as_str()
        } else {
            raw_line
        };

        if line.trim().is_empty() || is_summary(line) {
            continue;
        }

        let (depth, name) = match parse_line(line) {
            Some(pair) => pair,
            None => continue,
        };

        let id = if depth == 0 {
            name.to_string()
        } else if depth <= stack.len() {
            format!("{}/{}", stack[depth - 1], name)
        } else {
            eyre::bail!("unexpected depth jump at line: {line:?}");
        };

        stack.truncate(depth);
        stack.push(id.clone());

        graph
            .nodes
            .insert(id.clone(), NodeInfo::new(name.to_string()));

        if depth > 0 {
            graph.edges.push(Edge {
                from: stack[depth - 1].clone(),
                to: id,
                ..Default::default()
            });
        }
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
        let graph = parse("mydir\n").unwrap();
        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(graph.nodes["mydir"].label.as_str(), "mydir");
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn unicode_simple() {
        let input = "\
root
├── alpha
└── bravo
";
        let graph = parse(input).unwrap();
        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.nodes["root"].label.as_str(), "root");
        assert_eq!(graph.nodes["root/alpha"].label.as_str(), "alpha");
        assert_eq!(graph.nodes["root/bravo"].label.as_str(), "bravo");
        assert_eq!(graph.edges.len(), 2);
        assert_eq!(graph.edges[0].from, "root");
        assert_eq!(graph.edges[0].to, "root/alpha");
        assert_eq!(graph.edges[1].from, "root");
        assert_eq!(graph.edges[1].to, "root/bravo");
    }

    #[test]
    fn unicode_nested() {
        let input = "\
root
├── a
│   └── b
└── c
";
        let graph = parse(input).unwrap();
        assert_eq!(graph.nodes.len(), 4);
        assert!(graph.nodes.contains_key("root/a/b"));
        assert_eq!(graph.edges.len(), 3);
        assert_eq!(graph.edges[1].from, "root/a");
        assert_eq!(graph.edges[1].to, "root/a/b");
    }

    #[test]
    fn ascii_simple() {
        let input = "\
root
|-- alpha
`-- bravo
";
        let graph = parse(input).unwrap();
        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.nodes["root/alpha"].label.as_str(), "alpha");
        assert_eq!(graph.nodes["root/bravo"].label.as_str(), "bravo");
        assert_eq!(graph.edges.len(), 2);
    }

    #[test]
    fn ascii_nested() {
        let input = "\
root
|-- a
|   `-- b
`-- c
";
        let graph = parse(input).unwrap();
        assert_eq!(graph.nodes.len(), 4);
        assert!(graph.nodes.contains_key("root/a/b"));
        assert_eq!(graph.edges.len(), 3);
    }

    #[test]
    fn ascii_backslash_last_child() {
        let input = "\
root
\\-- only
";
        let graph = parse(input).unwrap();
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.nodes["root/only"].label.as_str(), "only");
    }

    #[test]
    fn blank_continuation() {
        // When parent is last child, its children use "    " instead of "|   "
        let input = "\
root
└── parent
    └── child
";
        let graph = parse(input).unwrap();
        assert_eq!(graph.nodes.len(), 3);
        assert!(graph.nodes.contains_key("root/parent/child"));
        assert_eq!(graph.edges[1].from, "root/parent");
        assert_eq!(graph.edges[1].to, "root/parent/child");
    }

    #[test]
    fn summary_line_skipped() {
        let input = "\
root
└── file.txt

1 directory, 1 file
";
        let graph = parse(input).unwrap();
        assert_eq!(graph.nodes.len(), 2);
        assert!(!graph.nodes.contains_key("1 directory, 1 file"));
    }

    #[test]
    fn summary_plural_skipped() {
        let input = "\
root
├── a
└── b

2 directories, 3 files
";
        let graph = parse(input).unwrap();
        assert_eq!(graph.nodes.len(), 3);
    }

    #[test]
    fn depth_returns_to_root_sibling() {
        let input = "\
root
├── a
│   ├── deep1
│   └── deep2
└── b
";
        let graph = parse(input).unwrap();
        assert_eq!(graph.nodes.len(), 5);
        assert_eq!(graph.edges[3].from, "root");
        assert_eq!(graph.edges[3].to, "root/b");
    }

    #[test]
    fn no_attrs() {
        let input = "\
root
└── child
";
        let graph = parse(input).unwrap();
        assert!(graph.nodes["root"].attrs.is_empty());
        assert!(graph.nodes["root/child"].attrs.is_empty());
    }

    #[test]
    fn fixture_tree_unicode() {
        let input = include_str!("../../../../data/depconv/tree.txt");
        let graph = parse(input).unwrap();
        assert!(graph.nodes.contains_key("crates"));
        assert!(graph.nodes.contains_key("crates/csvizmo-can"));
        assert!(graph.nodes.contains_key("crates/csvizmo-can/Cargo.toml"));
        assert!(graph.nodes.contains_key("crates/csvizmo-can/src/bin"));
        assert!(
            graph
                .nodes
                .contains_key("crates/csvizmo-utils/src/stdio.rs")
        );
        assert_eq!(
            graph.nodes["crates/csvizmo-can/Cargo.toml"].label.as_str(),
            "Cargo.toml"
        );
        // "crates" is the root -- no incoming edges
        assert!(!graph.edges.iter().any(|e| e.to == "crates"));
        // Spot-check a few edges
        assert!(
            graph
                .edges
                .iter()
                .any(|e| e.from == "crates" && e.to == "crates/csvizmo-can")
        );
        assert!(graph.edges.iter().any(
            |e| e.from == "crates/csvizmo-can/src" && e.to == "crates/csvizmo-can/src/bin"
        ));
    }

    #[test]
    fn fixture_tree_ascii() {
        let input = include_str!("../../../../data/depconv/tree-ascii.txt");
        let graph = parse(input).unwrap();
        assert!(graph.nodes.contains_key("crates"));
        assert!(graph.nodes.contains_key("crates/csvizmo-can"));
        assert!(graph.nodes.contains_key("crates/csvizmo-can/Cargo.toml"));
        assert!(
            graph
                .nodes
                .contains_key("crates/csvizmo-utils/src/stdio.rs")
        );
        // ASCII fixture has more entries (detect.rs, emit, parse dirs)
        assert!(
            graph
                .nodes
                .contains_key("crates/csvizmo-depgraph/src/detect.rs")
        );
    }

    #[test]
    fn strips_star_marker() {
        let input = "\
root
├── a
│   └── shared
├── b
│   └── shared (*)
";
        let graph = parse(input).unwrap();
        // "shared" under b should resolve to the same name (without marker)
        assert!(graph.nodes.contains_key("root/a/shared"));
        assert!(graph.nodes.contains_key("root/b/shared"));
        assert_eq!(graph.nodes["root/b/shared"].label.as_str(), "shared");
    }

    #[test]
    fn strips_cycle_marker() {
        let input = "\
root
├── a
│   └── root (cycle)
";
        let graph = parse(input).unwrap();
        assert!(graph.nodes.contains_key("root/a/root"));
        assert_eq!(graph.nodes["root/a/root"].label.as_str(), "root");
    }

    #[test]
    fn parse_line_depth() {
        assert_eq!(parse_line("root"), Some((0, "root")));
        assert_eq!(parse_line("├── child"), Some((1, "child")));
        assert_eq!(parse_line("│   └── grandchild"), Some((2, "grandchild")));
        assert_eq!(parse_line("|-- child"), Some((1, "child")));
        assert_eq!(parse_line("|   `-- grandchild"), Some((2, "grandchild")));
        assert_eq!(parse_line(""), None);
    }

    #[test]
    fn parse_line_strips_markers() {
        assert_eq!(parse_line("├── node (*)"), Some((1, "node")));
        assert_eq!(parse_line("└── node (cycle)"), Some((1, "node")));
        assert_eq!(parse_line("├── node  (*)"), Some((1, "node")));
        // No marker -- name preserved as-is
        assert_eq!(parse_line("├── node"), Some((1, "node")));
    }
}
