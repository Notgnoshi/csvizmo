use crate::{DepGraph, Edge, NodeInfo};

fn join_rest(parts: &mut std::str::SplitWhitespace) -> Option<String> {
    let rest: Vec<&str> = parts.collect();
    if rest.is_empty() {
        None
    } else {
        Some(rest.join(" "))
    }
}

pub fn parse(input: &str) -> eyre::Result<DepGraph> {
    let mut graph = DepGraph::default();
    let mut in_edges = false;

    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if line == "#" {
            in_edges = true;
            continue;
        }

        let mut parts = line.split_whitespace();

        if in_edges {
            // Edge line: "from to [label...]"
            let from = parts
                .next()
                .ok_or_else(|| eyre::eyre!("invalid edge line: {line:?}"))?;
            let to = parts
                .next()
                .ok_or_else(|| eyre::eyre!("invalid edge line: {line:?}"))?;
            graph.edges.push(Edge {
                from: from.to_string(),
                to: to.to_string(),
                label: join_rest(&mut parts),
                ..Default::default()
            });
        } else {
            // Node line: "id [label...]"
            let id = parts.next().unwrap();
            graph.nodes.insert(
                id.to_string(),
                NodeInfo {
                    label: join_rest(&mut parts),
                    ..Default::default()
                },
            );
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
    fn just_separator() {
        let graph = parse("#\n").unwrap();
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn nodes_with_labels() {
        let graph = parse("1 libfoo\n2 libbar\n#\n").unwrap();
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.nodes["1"].label.as_deref(), Some("libfoo"));
        assert_eq!(graph.nodes["2"].label.as_deref(), Some("libbar"));
    }

    #[test]
    fn nodes_without_labels() {
        let graph = parse("a\nb\n#\n").unwrap();
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.nodes["a"].label, None);
        assert_eq!(graph.nodes["b"].label, None);
    }

    #[test]
    fn edges_with_labels() {
        let graph = parse("a\nb\n#\na b depends on\n").unwrap();
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].from, "a");
        assert_eq!(graph.edges[0].to, "b");
        assert_eq!(graph.edges[0].label.as_deref(), Some("depends on"));
    }

    #[test]
    fn edges_without_labels() {
        let graph = parse("a\nb\n#\na b\n").unwrap();
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].from, "a");
        assert_eq!(graph.edges[0].to, "b");
        assert_eq!(graph.edges[0].label, None);
    }

    #[test]
    fn tab_separated() {
        let graph = parse("1\tlibfoo\n2\tlibbar\n#\n1\t2\tdepends on\n").unwrap();
        assert_eq!(graph.nodes["1"].label.as_deref(), Some("libfoo"));
        assert_eq!(graph.edges[0].label.as_deref(), Some("depends on"));
    }

    #[test]
    fn multiple_whitespace() {
        let graph = parse("1  libfoo\n2\t\tlibbar\n#\n1  2\n").unwrap();
        assert_eq!(graph.nodes["1"].label.as_deref(), Some("libfoo"));
        assert_eq!(graph.nodes["2"].label.as_deref(), Some("libbar"));
        assert_eq!(graph.edges[0].from, "1");
        assert_eq!(graph.edges[0].to, "2");
    }

    #[test]
    fn blank_lines_ignored() {
        let graph = parse("\n1 libfoo\n\n2 libbar\n\n#\n\n1 2\n\n").unwrap();
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 1);
    }

    #[test]
    fn preserves_node_order() {
        let graph = parse("c C\na A\nb B\n#\n").unwrap();
        let keys: Vec<&str> = graph.nodes.keys().map(|s| s.as_str()).collect();
        assert_eq!(keys, vec!["c", "a", "b"]);
    }

    #[test]
    fn attrs_empty() {
        let graph = parse("1 libfoo\n#\n").unwrap();
        assert!(graph.nodes["1"].attrs.is_empty());
    }

    #[test]
    fn parse_fixture_small() {
        let input = include_str!("../../../../data/depconv/small.tgf");
        let graph = parse(input).unwrap();
        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.nodes["1"].label.as_deref(), Some("libfoo"));
        assert_eq!(graph.nodes["2"].label.as_deref(), Some("libbar"));
        assert_eq!(graph.nodes["3"].label.as_deref(), Some("myapp"));
        assert_eq!(graph.edges.len(), 3);
        assert_eq!(graph.edges[0].from, "3");
        assert_eq!(graph.edges[0].to, "1");
        assert_eq!(graph.edges[0].label, None);
    }

    #[test]
    fn parse_fixture_nodes_only() {
        let input = include_str!("../../../../data/depconv/nodes-only.tgf");
        let graph = parse(input).unwrap();
        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.nodes["a"].label.as_deref(), Some("alpha"));
        assert_eq!(graph.nodes["b"].label.as_deref(), Some("bravo"));
        assert_eq!(graph.nodes["c"].label.as_deref(), Some("charlie"));
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn parse_fixture_edge_labels() {
        let input = include_str!("../../../../data/depconv/edge-labels.tgf");
        let graph = parse(input).unwrap();
        assert_eq!(graph.nodes.len(), 4);
        assert_eq!(graph.nodes["fmt"].label.as_deref(), Some("csvizmo-fmt"));
        assert_eq!(graph.edges.len(), 4);
        assert_eq!(graph.edges[0].from, "depgraph");
        assert_eq!(graph.edges[0].to, "utils");
        assert_eq!(graph.edges[0].label.as_deref(), Some("normal"));
    }
}
