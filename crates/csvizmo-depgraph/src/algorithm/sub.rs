use std::collections::HashSet;

use indexmap::IndexMap;
use regex::Regex;

use crate::{DepGraph, Edge, NodeInfo};

/// Which field to apply the substitution to.
#[derive(Debug, Clone)]
pub enum SubKey {
    /// Apply to node IDs; merge nodes when IDs collide.
    Id,
    /// Apply to a named node field (label, or an attribute name).
    Node(String),
    /// Apply to a named edge field (label, or an attribute name).
    Edge(String),
}

impl SubKey {
    /// Parse a `--key` value: `id`, `node:NAME`, or `edge:NAME`.
    pub fn parse(s: &str) -> eyre::Result<Self> {
        match s {
            "id" => Ok(Self::Id),
            s if s.starts_with("node:") => Ok(Self::Node(s["node:".len()..].to_string())),
            s if s.starts_with("edge:") => Ok(Self::Edge(s["edge:".len()..].to_string())),
            _ => eyre::bail!(
                "invalid --key value: {s:?}. Expected 'id', 'node:NAME', or 'edge:NAME'"
            ),
        }
    }
}

/// Parsed sed-style substitution: `s/pattern/replacement/`.
pub struct Substitution {
    pub regex: Regex,
    pub replacement: String,
}

impl Substitution {
    /// Parse a sed-style `s/pattern/replacement/` string.
    ///
    /// The first character after `s` is the delimiter. Supports any single-byte
    /// ASCII delimiter character (e.g. `/`, `|`, `#`, etc.).
    pub fn parse(s: &str) -> eyre::Result<Self> {
        let s = s.as_bytes();
        if s.is_empty() || s[0] != b's' {
            eyre::bail!("substitution must start with 's'");
        }
        if s.len() < 4 {
            eyre::bail!("substitution too short");
        }

        let delim = s[1];
        // Find the second delimiter (end of pattern), skipping escaped delimiters.
        let pattern_start = 2;
        let pattern_end = find_unescaped(s, delim, pattern_start)
            .ok_or_else(|| eyre::eyre!("missing second delimiter in substitution"))?;
        let replacement_start = pattern_end + 1;
        // The trailing delimiter is optional.
        let replacement_end = find_unescaped(s, delim, replacement_start).unwrap_or(s.len());

        let pattern = std::str::from_utf8(&s[pattern_start..pattern_end])?;
        let replacement = std::str::from_utf8(&s[replacement_start..replacement_end])?;
        // Unescape \<delim> in the replacement. The pattern doesn't need this
        // because the regex engine already interprets \<delim> as a literal match.
        // But the regex crate treats \ literally in replacements, so \<delim>
        // would produce a spurious backslash without unescaping.
        let replacement = unescape_delimiter(replacement, delim);

        let regex = Regex::new(pattern).map_err(|e| eyre::eyre!("invalid regex: {e}"))?;

        Ok(Self { regex, replacement })
    }

    fn apply(&self, input: &str) -> String {
        self.regex
            .replace_all(input, &self.replacement)
            .into_owned()
    }
}

/// Remove backslash escapes before the delimiter character in a string.
///
/// Only `\<delim>` sequences are unescaped; other backslash sequences are left as-is.
fn unescape_delimiter(s: &str, delim: u8) -> String {
    let bytes = s.as_bytes();
    let mut result = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() && bytes[i + 1] == delim {
            result.push(delim);
            i += 2;
        } else {
            result.push(bytes[i]);
            i += 1;
        }
    }
    // Safety: we only removed ASCII backslashes before ASCII delimiter bytes,
    // so all other valid UTF-8 sequences are preserved.
    String::from_utf8(result).expect("unescape produced invalid UTF-8")
}

/// Find the next unescaped occurrence of `delim` starting at `start`.
fn find_unescaped(s: &[u8], delim: u8, start: usize) -> Option<usize> {
    let mut i = start;
    while i < s.len() {
        if s[i] == b'\\' {
            i += 2; // skip escaped character
        } else if s[i] == delim {
            return Some(i);
        } else {
            i += 1;
        }
    }
    None
}

/// Apply a sed-style substitution to a dependency graph.
pub fn sub(graph: &DepGraph, substitution: &Substitution, key: &SubKey) -> DepGraph {
    match key {
        SubKey::Id => sub_id(graph, substitution),
        SubKey::Node(field) => sub_node_field(graph, substitution, field),
        SubKey::Edge(field) => sub_edge_field(graph, substitution, field),
    }
}

/// Apply substitution to node IDs, merging nodes that collide.
fn sub_id(graph: &DepGraph, substitution: &Substitution) -> DepGraph {
    // Build old->new ID mapping from all nodes across all subgraphs.
    let all_nodes = graph.all_nodes();
    let mut id_map: IndexMap<String, String> = IndexMap::new();
    for old_id in all_nodes.keys() {
        let new_id = substitution.apply(old_id);
        id_map.insert(old_id.clone(), new_id);
    }

    // Track which new IDs we've already placed (first subgraph wins).
    let mut placed: HashSet<String> = HashSet::new();

    remap_subgraph(graph, &id_map, &mut placed)
}

/// Recursively remap node IDs in a graph/subgraph, merging colliding nodes.
fn remap_subgraph(
    graph: &DepGraph,
    id_map: &IndexMap<String, String>,
    placed: &mut HashSet<String>,
) -> DepGraph {
    // Remap nodes in this level, merging on collision.
    // Nodes whose new ID is empty are removed.
    let mut nodes: IndexMap<String, NodeInfo> = IndexMap::new();
    for (old_id, info) in &graph.nodes {
        let new_id = &id_map[old_id];
        if new_id.is_empty() || placed.contains(new_id) {
            continue;
        }
        match nodes.get_mut(new_id) {
            Some(existing) => {
                // Merge: keep first label, first non-None node_type, merge attrs (first wins).
                if existing.node_type.is_none() {
                    existing.node_type.clone_from(&info.node_type);
                }
                for (k, v) in &info.attrs {
                    existing.attrs.entry(k.clone()).or_insert_with(|| v.clone());
                }
            }
            None => {
                nodes.insert(new_id.clone(), info.clone());
            }
        }
    }

    // Record all new IDs placed at this level.
    for new_id in nodes.keys() {
        placed.insert(new_id.clone());
    }

    // Recurse into subgraphs.
    let subgraphs: Vec<DepGraph> = graph
        .subgraphs
        .iter()
        .map(|sg| remap_subgraph(sg, id_map, placed))
        .filter(|sg| !sg.nodes.is_empty() || !sg.subgraphs.is_empty())
        .collect();

    // Remap edges, remove self-loops, deduplicate.
    let mut seen_edges: HashSet<(String, String, Option<String>)> = HashSet::new();
    let edges: Vec<Edge> = graph
        .edges
        .iter()
        .filter_map(|e| {
            let from = id_map.get(&e.from).unwrap_or(&e.from);
            let to = id_map.get(&e.to).unwrap_or(&e.to);
            if from.is_empty() || to.is_empty() {
                return None; // endpoint was removed
            }
            if from == to {
                return None; // self-loop
            }
            let key = (from.clone(), to.clone(), e.label.clone());
            if !seen_edges.insert(key) {
                return None; // duplicate
            }
            Some(Edge {
                from: from.clone(),
                to: to.clone(),
                label: e.label.clone(),
                attrs: e.attrs.clone(),
            })
        })
        .collect();

    DepGraph {
        id: graph.id.clone(),
        attrs: graph.attrs.clone(),
        nodes,
        edges,
        subgraphs,
        ..Default::default()
    }
}

/// Apply substitution to a node field (label, node_type, or attribute).
/// Empty results are treated as removal: label resets to node ID,
/// Option fields become None, and attributes are deleted.
fn sub_node_field(graph: &DepGraph, substitution: &Substitution, field: &str) -> DepGraph {
    DepGraph {
        id: graph.id.clone(),
        attrs: graph.attrs.clone(),
        nodes: graph
            .nodes
            .iter()
            .map(|(id, info)| {
                let mut info = info.clone();
                match field {
                    "label" => {
                        let new_label = substitution.apply(&info.label);
                        info.label = if new_label.is_empty() {
                            id.clone()
                        } else {
                            new_label
                        };
                    }
                    "node_type" => {
                        if let Some(ref nt) = info.node_type {
                            let new_nt = substitution.apply(nt);
                            info.node_type = if new_nt.is_empty() {
                                None
                            } else {
                                Some(new_nt)
                            };
                        }
                    }
                    _ => {
                        if let Some(val) = info.attrs.get(field) {
                            let new_val = substitution.apply(val);
                            if new_val.is_empty() {
                                info.attrs.swap_remove(field);
                            } else {
                                info.attrs.insert(field.to_string(), new_val);
                            }
                        }
                    }
                }
                (id.clone(), info)
            })
            .collect(),
        edges: graph.edges.clone(),
        subgraphs: graph
            .subgraphs
            .iter()
            .map(|sg| sub_node_field(sg, substitution, field))
            .collect(),
        ..Default::default()
    }
}

/// Apply substitution to an edge field (label or attribute).
fn sub_edge_field(graph: &DepGraph, substitution: &Substitution, field: &str) -> DepGraph {
    DepGraph {
        id: graph.id.clone(),
        attrs: graph.attrs.clone(),
        nodes: graph.nodes.clone(),
        edges: graph
            .edges
            .iter()
            .map(|e| {
                let mut e = e.clone();
                if field == "label" {
                    if let Some(ref label) = e.label {
                        let new_label = substitution.apply(label);
                        e.label = if new_label.is_empty() {
                            None
                        } else {
                            Some(new_label)
                        };
                    }
                } else if let Some(val) = e.attrs.get(field) {
                    let new_val = substitution.apply(val);
                    if new_val.is_empty() {
                        e.attrs.swap_remove(field);
                    } else {
                        e.attrs.insert(field.to_string(), new_val);
                    }
                }
                e
            })
            .collect(),
        subgraphs: graph
            .subgraphs
            .iter()
            .map(|sg| sub_edge_field(sg, substitution, field))
            .collect(),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn node_ids(graph: &DepGraph) -> Vec<&str> {
        graph.nodes.keys().map(|s| s.as_str()).collect()
    }

    fn edge_pairs(graph: &DepGraph) -> Vec<(&str, &str)> {
        graph
            .edges
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str()))
            .collect()
    }

    // -- Substitution parsing --

    #[test]
    fn parse_basic() {
        let s = Substitution::parse("s/foo/bar/").unwrap();
        assert_eq!(s.apply("foobar"), "barbar");
    }

    #[test]
    fn parse_alternate_delimiter() {
        let s = Substitution::parse("s|foo|bar|").unwrap();
        assert_eq!(s.apply("foo"), "bar");
    }

    #[test]
    fn parse_no_trailing_delimiter() {
        let s = Substitution::parse("s/foo/bar").unwrap();
        assert_eq!(s.apply("foo"), "bar");
    }

    #[test]
    fn parse_capture_groups() {
        let s = Substitution::parse("s/([^.]+)\\..*/$1/").unwrap();
        assert_eq!(s.apply("acl-native.do_compile"), "acl-native");
    }

    #[test]
    fn parse_empty_replacement() {
        let s = Substitution::parse("s/\\.do_.*//").unwrap();
        assert_eq!(s.apply("acl-native.do_compile"), "acl-native");
    }

    #[test]
    fn parse_escaped_delimiter_in_replacement() {
        // s/a/b\/c/ -- replacement should be b/c, not b\/c
        let s = Substitution::parse("s/a/b\\/c/").unwrap();
        assert_eq!(s.apply("a"), "b/c");
    }

    #[test]
    fn parse_escaped_alt_delimiter_in_replacement() {
        // s|a|b\|c| -- replacement should be b|c
        let s = Substitution::parse("s|a|b\\|c|").unwrap();
        assert_eq!(s.apply("a"), "b|c");
    }

    #[test]
    fn parse_non_delimiter_backslash_preserved_in_replacement() {
        // s/a/b\\nc/ -- \n is not the delimiter, so kept as literal \n
        let s = Substitution::parse("s/a/b\\nc/").unwrap();
        assert_eq!(s.apply("a"), "b\\nc");
    }

    #[test]
    fn parse_invalid_no_s() {
        assert!(Substitution::parse("foo").is_err());
    }

    // -- SubKey parsing --

    #[test]
    fn subkey_id() {
        matches!(SubKey::parse("id").unwrap(), SubKey::Id);
    }

    #[test]
    fn subkey_node() {
        match SubKey::parse("node:label").unwrap() {
            SubKey::Node(name) => assert_eq!(name, "label"),
            _ => panic!("expected Node"),
        }
    }

    #[test]
    fn subkey_edge() {
        match SubKey::parse("edge:label").unwrap() {
            SubKey::Edge(name) => assert_eq!(name, "label"),
            _ => panic!("expected Edge"),
        }
    }

    #[test]
    fn subkey_invalid() {
        assert!(SubKey::parse("invalid").is_err());
    }

    // -- sub with id key --

    #[test]
    fn sub_id_no_collision() {
        let g = make_graph(
            &[("a.do_compile", "A"), ("b.do_build", "B")],
            &[("a.do_compile", "b.do_build")],
            vec![],
        );
        let s = Substitution::parse("s/\\.do_.*//").unwrap();
        let result = sub(&g, &s, &SubKey::Id);
        assert_eq!(node_ids(&result), vec!["a", "b"]);
        assert_eq!(edge_pairs(&result), vec![("a", "b")]);
    }

    #[test]
    fn sub_id_merges_nodes() {
        let g = make_graph(
            &[("a.do_compile", "A"), ("a.do_build", "B")],
            &[("a.do_compile", "a.do_build")],
            vec![],
        );
        let s = Substitution::parse("s/\\.do_.*//").unwrap();
        let result = sub(&g, &s, &SubKey::Id);
        // Both map to "a", merged into one node
        assert_eq!(node_ids(&result), vec!["a"]);
        // Self-loop removed
        assert!(result.edges.is_empty());
        // First node's label wins
        assert_eq!(result.nodes["a"].label, "A");
    }

    #[test]
    fn sub_id_deduplicates_edges() {
        // a.x -> b, a.y -> b: both map to a -> b, should deduplicate
        let g = make_graph(
            &[("a.x", "A"), ("a.y", "A"), ("b", "B")],
            &[("a.x", "b"), ("a.y", "b")],
            vec![],
        );
        let s = Substitution::parse("s/\\..*//").unwrap();
        let result = sub(&g, &s, &SubKey::Id);
        assert_eq!(node_ids(&result), vec!["a", "b"]);
        assert_eq!(edge_pairs(&result), vec![("a", "b")]);
    }

    #[test]
    fn sub_id_subgraph_first_wins() {
        // subgraph has a.x, root has a.y. Both map to "a".
        // Subgraph nodes are processed first in document order (root level first),
        // but here a.y is at root and a.x is in subgraph.
        let sub_g = make_graph(&[("a.x", "SubA")], &[], vec![]);
        let g = make_graph(&[("a.y", "RootA")], &[], vec![sub_g]);
        let s = Substitution::parse("s/\\..*//").unwrap();
        let result = sub(&g, &s, &SubKey::Id);
        // Root level is processed first, so "a" appears at root
        assert_eq!(node_ids(&result), vec!["a"]);
        assert_eq!(result.nodes["a"].label, "RootA");
        // Subgraph should be empty and dropped
        assert!(result.subgraphs.is_empty());
    }

    #[test]
    fn sub_id_drops_empty_subgraphs() {
        let sub_g = make_graph(&[("a.x", "A")], &[], vec![]);
        let g = make_graph(&[("a.y", "A")], &[], vec![sub_g]);
        let s = Substitution::parse("s/\\..*//").unwrap();
        let result = sub(&g, &s, &SubKey::Id);
        assert!(result.subgraphs.is_empty());
    }

    // -- sub removing nodes (empty ID) --

    #[test]
    fn sub_id_removes_empty() {
        // b matches the pattern and maps to ""; it should be removed along with its edges.
        let g = make_graph(
            &[("a", "A"), ("b.remove", "B"), ("c", "C")],
            &[("a", "b.remove"), ("b.remove", "c"), ("a", "c")],
            vec![],
        );
        let s = Substitution::parse("s/^b\\..*//").unwrap();
        let result = sub(&g, &s, &SubKey::Id);
        assert_eq!(node_ids(&result), vec!["a", "c"]);
        assert_eq!(edge_pairs(&result), vec![("a", "c")]);
    }

    #[test]
    fn sub_id_removes_all_empty() {
        // All nodes map to "" -- result should be completely empty.
        let g = make_graph(&[("a", "A"), ("b", "B")], &[("a", "b")], vec![]);
        let s = Substitution::parse("s/.*//").unwrap();
        let result = sub(&g, &s, &SubKey::Id);
        assert!(result.nodes.is_empty());
        assert!(result.edges.is_empty());
    }

    // -- sub with node key --

    #[test]
    fn sub_node_label() {
        let g = make_graph(&[("a", "hello world"), ("b", "goodbye world")], &[], vec![]);
        let s = Substitution::parse("s/world/earth/").unwrap();
        let result = sub(&g, &s, &SubKey::Node("label".to_string()));
        assert_eq!(result.nodes["a"].label, "hello earth");
        assert_eq!(result.nodes["b"].label, "goodbye earth");
    }

    #[test]
    fn sub_node_attr() {
        let mut g = make_graph(&[("a", "A")], &[], vec![]);
        g.nodes
            .get_mut("a")
            .unwrap()
            .attrs
            .insert("color".to_string(), "red".to_string());
        let s = Substitution::parse("s/red/blue/").unwrap();
        let result = sub(&g, &s, &SubKey::Node("color".to_string()));
        assert_eq!(result.nodes["a"].attrs["color"], "blue");
    }

    #[test]
    fn sub_node_label_resets_to_id_when_empty() {
        let g = make_graph(&[("a", "hello"), ("b", "goodbye")], &[], vec![]);
        let s = Substitution::parse("s/.*//").unwrap();
        let result = sub(&g, &s, &SubKey::Node("label".to_string()));
        assert_eq!(result.nodes["a"].label, "a");
        assert_eq!(result.nodes["b"].label, "b");
    }

    #[test]
    fn sub_node_type_removes_empty() {
        let mut g = make_graph(&[("a", "A")], &[], vec![]);
        g.nodes.get_mut("a").unwrap().node_type = Some("lib".to_string());
        let s = Substitution::parse("s/.*//").unwrap();
        let result = sub(&g, &s, &SubKey::Node("node_type".to_string()));
        assert_eq!(result.nodes["a"].node_type, None);
    }

    #[test]
    fn sub_node_attr_removes_empty() {
        let mut g = make_graph(&[("a", "A")], &[], vec![]);
        g.nodes
            .get_mut("a")
            .unwrap()
            .attrs
            .insert("color".to_string(), "red".to_string());
        let s = Substitution::parse("s/.*//").unwrap();
        let result = sub(&g, &s, &SubKey::Node("color".to_string()));
        assert!(!result.nodes["a"].attrs.contains_key("color"));
    }

    // -- sub with edge key --

    #[test]
    fn sub_edge_label() {
        let mut g = make_graph(&[("a", "A"), ("b", "B")], &[], vec![]);
        g.edges.push(Edge {
            from: "a".to_string(),
            to: "b".to_string(),
            label: Some("depends_on".to_string()),
            ..Default::default()
        });
        let s = Substitution::parse("s/depends_on/uses/").unwrap();
        let result = sub(&g, &s, &SubKey::Edge("label".to_string()));
        assert_eq!(result.edges[0].label.as_deref(), Some("uses"));
    }

    #[test]
    fn sub_edge_label_removes_empty() {
        let mut g = make_graph(&[("a", "A"), ("b", "B")], &[], vec![]);
        g.edges.push(Edge {
            from: "a".to_string(),
            to: "b".to_string(),
            label: Some("depends_on".to_string()),
            ..Default::default()
        });
        let s = Substitution::parse("s/.*//").unwrap();
        let result = sub(&g, &s, &SubKey::Edge("label".to_string()));
        assert_eq!(result.edges[0].label, None);
    }

    #[test]
    fn sub_edge_attr_removes_empty() {
        let mut g = make_graph(&[("a", "A"), ("b", "B")], &[], vec![]);
        let mut edge = Edge {
            from: "a".to_string(),
            to: "b".to_string(),
            ..Default::default()
        };
        edge.attrs.insert("style".to_string(), "dashed".to_string());
        g.edges.push(edge);
        let s = Substitution::parse("s/.*//").unwrap();
        let result = sub(&g, &s, &SubKey::Edge("style".to_string()));
        assert!(!result.edges[0].attrs.contains_key("style"));
    }
}
