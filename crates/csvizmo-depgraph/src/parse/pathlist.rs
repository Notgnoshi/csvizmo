use crate::{DepGraph, Edge, NodeInfo};

pub fn parse(input: &str) -> eyre::Result<DepGraph> {
    let mut graph = DepGraph::default();

    for line in input.lines() {
        // Strip tab-separated trailing markers (e.g. "path/\t(*)" or "path/\t(cycle)")
        let line = match line.split_once('\t') {
            Some((path, _marker)) => path,
            None => line,
        };

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Strip leading "./" and trailing "/"
        let path = line.strip_prefix("./").unwrap_or(line);
        let path = path.strip_suffix('/').unwrap_or(path);
        if path.is_empty() {
            continue;
        }

        let mut current = String::new();
        for (i, component) in path.split('/').enumerate() {
            let parent = if i > 0 { Some(current.clone()) } else { None };

            if i > 0 {
                current.push('/');
            }
            current.push_str(component);

            // Each unique path is inserted once; edge added with the new node.
            if !graph.nodes.contains_key(&current) {
                graph.nodes.insert(
                    current.clone(),
                    NodeInfo {
                        label: Some(component.to_string()),
                        ..Default::default()
                    },
                );
                if let Some(parent) = parent {
                    graph.edges.push(Edge {
                        from: parent,
                        to: current.clone(),
                        ..Default::default()
                    });
                }
            }
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
    fn single_path() {
        let graph = parse("src/main.rs\n").unwrap();
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.nodes["src"].label.as_deref(), Some("src"));
        assert_eq!(graph.nodes["src/main.rs"].label.as_deref(), Some("main.rs"));
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].from, "src");
        assert_eq!(graph.edges[0].to, "src/main.rs");
    }

    #[test]
    fn shared_prefix() {
        let graph = parse("src/a.rs\nsrc/b.rs\n").unwrap();
        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.nodes.get_index(0).unwrap().0, "src");
        assert_eq!(graph.nodes.get_index(1).unwrap().0, "src/a.rs");
        assert_eq!(graph.nodes.get_index(2).unwrap().0, "src/b.rs");
        assert_eq!(graph.edges.len(), 2);
        assert_eq!(graph.edges[0].from, "src");
        assert_eq!(graph.edges[0].to, "src/a.rs");
        assert_eq!(graph.edges[1].from, "src");
        assert_eq!(graph.edges[1].to, "src/b.rs");
    }

    #[test]
    fn nested_paths() {
        let graph = parse("a/b/c\n").unwrap();
        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.nodes["a"].label.as_deref(), Some("a"));
        assert_eq!(graph.nodes["a/b"].label.as_deref(), Some("b"));
        assert_eq!(graph.nodes["a/b/c"].label.as_deref(), Some("c"));
        assert_eq!(graph.edges.len(), 2);
        assert_eq!(graph.edges[0].from, "a");
        assert_eq!(graph.edges[0].to, "a/b");
        assert_eq!(graph.edges[1].from, "a/b");
        assert_eq!(graph.edges[1].to, "a/b/c");
    }

    #[test]
    fn strips_leading_dot_slash() {
        let graph = parse("./src/main.rs\n").unwrap();
        assert_eq!(graph.nodes.len(), 2);
        assert!(graph.nodes.contains_key("src"));
        assert!(graph.nodes.contains_key("src/main.rs"));
    }

    #[test]
    fn strips_trailing_slash() {
        let graph = parse("src/dir/\n").unwrap();
        assert_eq!(graph.nodes.len(), 2);
        assert!(graph.nodes.contains_key("src"));
        assert!(graph.nodes.contains_key("src/dir"));
    }

    #[test]
    fn blank_lines_ignored() {
        let graph = parse("\nsrc/a.rs\n\nsrc/b.rs\n\n").unwrap();
        assert_eq!(graph.nodes.len(), 3);
    }

    #[test]
    fn dot_slash_only_skipped() {
        let graph = parse("./\n").unwrap();
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn single_component() {
        let graph = parse("README.md\n").unwrap();
        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(graph.nodes["README.md"].label.as_deref(), Some("README.md"));
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn dot_slash_and_no_dot_slash_merge() {
        let graph = parse("./src/a.rs\nsrc/b.rs\n").unwrap();
        // "src" node should be shared
        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.edges.len(), 2);
    }

    #[test]
    fn preserves_insertion_order() {
        let graph = parse("z/b\na/c\n").unwrap();
        let keys: Vec<&str> = graph.nodes.keys().map(|s| s.as_str()).collect();
        assert_eq!(keys, vec!["z", "z/b", "a", "a/c"]);
    }

    #[test]
    fn no_attrs() {
        let graph = parse("src/main.rs\n").unwrap();
        assert!(graph.nodes["src"].attrs.is_empty());
        assert!(graph.nodes["src/main.rs"].attrs.is_empty());
    }

    #[test]
    fn strips_star_marker() {
        let graph = parse("a/b\na/c/\t(*)\n").unwrap();
        assert_eq!(graph.nodes.len(), 3);
        assert!(graph.nodes.contains_key("a"));
        assert!(graph.nodes.contains_key("a/b"));
        assert!(graph.nodes.contains_key("a/c"));
        assert_eq!(graph.edges.len(), 2);
    }

    #[test]
    fn strips_cycle_marker() {
        let graph = parse("a/b\na/b/a/\t(cycle)\n").unwrap();
        assert!(graph.nodes.contains_key("a/b/a"));
    }

    #[test]
    fn fixture_gitfiles() {
        let input = include_str!("../../../../data/depconv/gitfiles.txt");
        let graph = parse(input).unwrap();
        assert!(graph.nodes.contains_key("crates"));
        assert!(graph.nodes.contains_key("crates/csvizmo-can"));
        assert!(graph.nodes.contains_key("crates/csvizmo-can/Cargo.toml"));
        assert_eq!(
            graph.nodes["crates/csvizmo-can/Cargo.toml"]
                .label
                .as_deref(),
            Some("Cargo.toml")
        );
        // Multiple crates share the "crates" prefix -- only one "crates" node
        let crates_children: Vec<&Edge> =
            graph.edges.iter().filter(|e| e.from == "crates").collect();
        assert!(crates_children.len() > 1);
    }

    #[test]
    fn fixture_find() {
        let input = include_str!("../../../../data/depconv/find.txt");
        let graph = parse(input).unwrap();
        // find output uses "./" prefix -- should be stripped
        assert!(!graph.nodes.contains_key("."));
        assert!(graph.nodes.contains_key("crates"));
        assert!(
            graph
                .nodes
                .contains_key("crates/csvizmo-can/src/bin/can2csv.rs")
        );
        assert_eq!(
            graph.nodes["crates/csvizmo-can/src/bin/can2csv.rs"]
                .label
                .as_deref(),
            Some("can2csv.rs")
        );
    }
}
