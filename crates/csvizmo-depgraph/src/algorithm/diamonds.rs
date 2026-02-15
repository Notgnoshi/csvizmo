use std::collections::{HashSet, VecDeque};

use clap::Parser;
use indexmap::IndexMap;
use petgraph::Direction;
use petgraph::graph::NodeIndex;

use super::{MatchKey, build_globset};
use crate::{DepGraph, FlatGraphView, NodeInfo};

#[derive(Clone, Debug, Default, Parser)]
pub struct DiamondsArgs {
    /// Glob pattern filtering diamonds by top or bottom node (can be repeated, OR logic)
    #[clap(short, long)]
    pub pattern: Vec<String>,

    /// Match patterns against 'id' or 'label'
    #[clap(long, default_value_t = MatchKey::default())]
    pub key: MatchKey,

    /// Only report diamonds with shortest path >= N edges
    #[clap(long)]
    pub min_depth: Option<usize>,

    /// Suppress edges between different diamond subgraphs
    #[clap(long)]
    pub no_cross_edges: bool,
}

impl DiamondsArgs {
    pub fn pattern(mut self, p: impl Into<String>) -> Self {
        self.pattern.push(p.into());
        self
    }

    pub fn key(mut self, k: MatchKey) -> Self {
        self.key = k;
        self
    }

    pub fn min_depth(mut self, n: usize) -> Self {
        self.min_depth = Some(n);
        self
    }

    pub fn no_cross_edges(mut self) -> Self {
        self.no_cross_edges = true;
        self
    }
}

/// Detect diamond dependencies in the graph and output them as subgraphs.
///
/// A diamond is a pair (top, bottom) where at least two internally vertex-disjoint
/// paths exist from top to bottom. Detection finds join nodes (in-degree >= 2),
/// computes pairwise LCA of their parents, and deduplicates by (top, bottom) pair.
///
/// Each diamond (top, bottom) pair is emitted as its own subgraph. Nodes live inside
/// subgraphs (for DOT cluster rendering), edges live at root level. Edges internal to
/// at least one diamond are always included. Cross-edges (connecting nodes in different
/// diamonds but not on any single diamond's paths) are included by default but can be
/// suppressed with `no_cross_edges`.
///
/// Complexity:
/// - Overall: O( (sum k_j)(N+E) + (sum k_j^2)(N) + D(N+E) ) where k_j = in-degree
///   of join node j, D = diamonds found
/// - Per join node: O(k * (N+E)) for ancestor BFS + O(k^2 * N) for pairwise intersection
/// - Practical: fast for typical dependency graphs (k is usually small); warns for
///   degenerate cases
pub fn diamonds(graph: &DepGraph, args: &DiamondsArgs) -> eyre::Result<DepGraph> {
    let view = FlatGraphView::new(graph);
    let all_nodes = graph.all_nodes();
    let all_edges = graph.all_edges();

    let globset = if args.pattern.is_empty() {
        None
    } else {
        Some(build_globset(&args.pattern)?)
    };

    // Find join nodes (in-degree >= 2) -- potential diamond bottoms.
    let join_nodes: Vec<NodeIndex> = view
        .pg
        .node_indices()
        .filter(|&idx| view.pg.neighbors_directed(idx, Direction::Incoming).count() >= 2)
        .collect();

    // For each join node, find diamond (top, bottom) pairs via pairwise LCA.
    let mut diamond_pairs: HashSet<(NodeIndex, NodeIndex)> = HashSet::new();

    for &bottom in &join_nodes {
        let parents: Vec<NodeIndex> = view
            .pg
            .neighbors_directed(bottom, Direction::Incoming)
            .collect();

        if parents.len() > 20 {
            let bottom_id = view.idx_to_id[bottom.index()];
            tracing::warn!(
                "Join node {bottom_id} has {} parents; pairwise LCA is O(k^2)",
                parents.len()
            );
        }

        // Precompute ancestor sets for each parent via reverse BFS.
        let ancestor_sets: Vec<HashSet<NodeIndex>> = parents
            .iter()
            .map(|&p| view.bfs([p], Direction::Incoming, None))
            .collect();

        // Pairwise intersect to find common ancestors, then filter to LCAs.
        for i in 0..parents.len() {
            for j in (i + 1)..parents.len() {
                let common: HashSet<NodeIndex> = ancestor_sets[i]
                    .intersection(&ancestor_sets[j])
                    .copied()
                    .collect();

                for &node in &common {
                    let is_lca = view
                        .pg
                        .neighbors_directed(node, Direction::Outgoing)
                        .all(|child| !common.contains(&child));
                    if is_lca {
                        diamond_pairs.insert((node, bottom));
                    }
                }
            }
        }
    }

    if diamond_pairs.is_empty() {
        return Ok(DepGraph::default());
    }

    // Apply pattern filter: keep diamonds where top or bottom matches.
    if let Some(ref gs) = globset {
        diamond_pairs.retain(|&(top, bottom)| {
            let top_id = view.idx_to_id[top.index()];
            let bottom_id = view.idx_to_id[bottom.index()];
            let top_text = match args.key {
                MatchKey::Id => top_id,
                MatchKey::Label => all_nodes[top_id].label.as_str(),
            };
            let bottom_text = match args.key {
                MatchKey::Id => bottom_id,
                MatchKey::Label => all_nodes[bottom_id].label.as_str(),
            };
            gs.is_match(top_text) || gs.is_match(bottom_text)
        });
    }

    // Apply min-depth filter.
    if let Some(min_depth) = args.min_depth {
        diamond_pairs.retain(|&(top, bottom)| shortest_path_len(&view, top, bottom) >= min_depth);
    }

    if diamond_pairs.is_empty() {
        return Ok(DepGraph::default());
    }

    // Sort diamond pairs by (top_id, bottom_id) for deterministic output.
    let mut sorted_pairs: Vec<(NodeIndex, NodeIndex)> = diamond_pairs.into_iter().collect();
    sorted_pairs.sort_by_key(|&(top, bottom)| {
        (view.idx_to_id[top.index()], view.idx_to_id[bottom.index()])
    });

    // Build one subgraph per diamond and collect edges.
    let mut subgraphs = Vec::new();
    let mut internal_edge_keys: HashSet<(&str, &str)> = HashSet::new();
    let mut all_diamond_ids: HashSet<&str> = HashSet::new();

    for &(top, bottom) in &sorted_pairs {
        let backward = view.bfs([bottom], Direction::Incoming, None);
        let forward = view.bfs([top], Direction::Outgoing, None);
        let keep: HashSet<NodeIndex> = backward.intersection(&forward).copied().collect();

        let keep_ids: HashSet<&str> = keep
            .iter()
            .filter_map(|idx| view.idx_to_id.get(idx.index()).copied())
            .collect();
        all_diamond_ids.extend(&keep_ids);

        let nodes: IndexMap<String, NodeInfo> = all_nodes
            .iter()
            .filter(|(id, _)| keep_ids.contains(id.as_str()))
            .map(|(id, info)| (id.clone(), info.clone()))
            .collect();

        // Track which edges are internal to at least one diamond.
        for edge in all_edges {
            if keep_ids.contains(edge.from.as_str()) && keep_ids.contains(edge.to.as_str()) {
                internal_edge_keys.insert((edge.from.as_str(), edge.to.as_str()));
            }
        }

        let top_id = view.idx_to_id[top.index()];
        let bottom_id = view.idx_to_id[bottom.index()];
        subgraphs.push(DepGraph {
            id: Some(format!("{top_id}..{bottom_id}")),
            nodes,
            ..Default::default()
        });
    }

    // Collect root-level edges: internal edges (always) + cross-edges (unless suppressed).
    let mut seen_edges: HashSet<(&str, &str)> = HashSet::new();
    let mut edges = Vec::new();
    for edge in all_edges {
        let key = (edge.from.as_str(), edge.to.as_str());
        let is_internal = internal_edge_keys.contains(&key);
        let is_cross = !is_internal
            && all_diamond_ids.contains(edge.from.as_str())
            && all_diamond_ids.contains(edge.to.as_str());
        if (is_internal || (is_cross && !args.no_cross_edges)) && seen_edges.insert(key) {
            edges.push(edge.clone());
        }
    }

    Ok(DepGraph {
        edges,
        subgraphs,
        ..Default::default()
    })
}

/// Compute the shortest path length (in edges) from `from` to `to` via BFS.
fn shortest_path_len(view: &FlatGraphView, from: NodeIndex, to: NodeIndex) -> usize {
    if from == to {
        return 0;
    }
    let mut visited = HashSet::new();
    let mut queue: VecDeque<(NodeIndex, usize)> = VecDeque::new();
    visited.insert(from);
    queue.push_back((from, 0));

    while let Some((node, depth)) = queue.pop_front() {
        for neighbor in view.pg.neighbors_directed(node, Direction::Outgoing) {
            if neighbor == to {
                return depth + 1;
            }
            if visited.insert(neighbor) {
                queue.push_back((neighbor, depth + 1));
            }
        }
    }
    usize::MAX
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DepGraph, Edge, NodeInfo};

    fn make_graph(nodes: &[(&str, &str)], edges: &[(&str, &str)]) -> DepGraph {
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
            ..Default::default()
        }
    }

    fn sorted_node_ids(graph: &DepGraph) -> Vec<&str> {
        let mut ids: Vec<&str> = graph.nodes.keys().map(|s| s.as_str()).collect();
        ids.sort();
        ids
    }

    fn sorted_edge_pairs(graph: &DepGraph) -> Vec<(&str, &str)> {
        let mut pairs: Vec<(&str, &str)> = graph
            .edges
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str()))
            .collect();
        pairs.sort();
        pairs
    }

    #[test]
    fn simple_diamond() {
        // A -> B -> D, A -> C -> D
        let g = make_graph(
            &[("A", "A"), ("B", "B"), ("C", "C"), ("D", "D")],
            &[("A", "B"), ("A", "C"), ("B", "D"), ("C", "D")],
        );
        let result = diamonds(&g, &DiamondsArgs::default()).unwrap();

        assert!(result.nodes.is_empty(), "no top-level nodes");
        assert_eq!(result.subgraphs.len(), 1);

        let sg = &result.subgraphs[0];
        assert_eq!(sg.id.as_deref(), Some("A..D"));
        assert_eq!(sorted_node_ids(sg), vec!["A", "B", "C", "D"]);
        assert_eq!(
            sorted_edge_pairs(&result),
            vec![("A", "B"), ("A", "C"), ("B", "D"), ("C", "D")]
        );
    }

    #[test]
    fn no_diamond_chain() {
        // A -> B -> C: no join nodes
        let g = make_graph(
            &[("A", "A"), ("B", "B"), ("C", "C")],
            &[("A", "B"), ("B", "C")],
        );
        let result = diamonds(&g, &DiamondsArgs::default()).unwrap();
        assert!(result.nodes.is_empty());
        assert!(result.edges.is_empty());
        assert!(result.subgraphs.is_empty());
    }

    #[test]
    fn nested_diamonds() {
        // A -> B -> D, A -> C -> D, D -> E -> G, D -> F -> G
        let g = make_graph(
            &[
                ("A", "A"),
                ("B", "B"),
                ("C", "C"),
                ("D", "D"),
                ("E", "E"),
                ("F", "F"),
                ("G", "G"),
            ],
            &[
                ("A", "B"),
                ("A", "C"),
                ("B", "D"),
                ("C", "D"),
                ("D", "E"),
                ("D", "F"),
                ("E", "G"),
                ("F", "G"),
            ],
        );
        let result = diamonds(&g, &DiamondsArgs::default()).unwrap();
        assert_eq!(result.subgraphs.len(), 2);

        let sg_ad = result
            .subgraphs
            .iter()
            .find(|sg| sg.id.as_deref() == Some("A..D"))
            .unwrap();
        let sg_dg = result
            .subgraphs
            .iter()
            .find(|sg| sg.id.as_deref() == Some("D..G"))
            .unwrap();

        assert_eq!(sorted_node_ids(sg_ad), vec!["A", "B", "C", "D"]);
        assert_eq!(sorted_node_ids(sg_dg), vec!["D", "E", "F", "G"]);
    }

    #[test]
    fn multiple_lcas() {
        // A -> X, A -> Y, B -> X, B -> Y, X -> J, Y -> J
        // Both A and B are LCAs for parents X and Y of join node J.
        let g = make_graph(
            &[("A", "A"), ("B", "B"), ("J", "J"), ("X", "X"), ("Y", "Y")],
            &[
                ("A", "X"),
                ("A", "Y"),
                ("B", "X"),
                ("B", "Y"),
                ("X", "J"),
                ("Y", "J"),
            ],
        );
        let result = diamonds(&g, &DiamondsArgs::default()).unwrap();

        // Two diamonds: (A, J) and (B, J), each with its own subgraph.
        assert_eq!(result.subgraphs.len(), 2);

        let sg_aj = result
            .subgraphs
            .iter()
            .find(|sg| sg.id.as_deref() == Some("A..J"))
            .unwrap();
        let sg_bj = result
            .subgraphs
            .iter()
            .find(|sg| sg.id.as_deref() == Some("B..J"))
            .unwrap();

        assert_eq!(sorted_node_ids(sg_aj), vec!["A", "J", "X", "Y"]);
        assert_eq!(sorted_node_ids(sg_bj), vec!["B", "J", "X", "Y"]);
        assert_eq!(
            sorted_edge_pairs(&result),
            vec![
                ("A", "X"),
                ("A", "Y"),
                ("B", "X"),
                ("B", "Y"),
                ("X", "J"),
                ("Y", "J"),
            ]
        );
    }

    #[test]
    fn deduplication_across_parent_pairs() {
        // A -> B, A -> C, A -> D, B -> J, C -> J, D -> J
        // Three parent pairs (B,C), (B,D), (C,D) all find A as LCA.
        // Only one diamond (A, J) should result.
        let g = make_graph(
            &[("A", "A"), ("B", "B"), ("C", "C"), ("D", "D"), ("J", "J")],
            &[
                ("A", "B"),
                ("A", "C"),
                ("A", "D"),
                ("B", "J"),
                ("C", "J"),
                ("D", "J"),
            ],
        );
        let result = diamonds(&g, &DiamondsArgs::default()).unwrap();

        assert_eq!(result.subgraphs.len(), 1);
        let sg = &result.subgraphs[0];
        assert_eq!(sg.id.as_deref(), Some("A..J"));
        assert_eq!(sorted_node_ids(sg), vec!["A", "B", "C", "D", "J"]);
    }

    #[test]
    fn pattern_filtering() {
        // Nested diamonds: (A, D) and (D, G). Pattern "G" keeps only (D, G).
        let g = make_graph(
            &[
                ("A", "A"),
                ("B", "B"),
                ("C", "C"),
                ("D", "D"),
                ("E", "E"),
                ("F", "F"),
                ("G", "G"),
            ],
            &[
                ("A", "B"),
                ("A", "C"),
                ("B", "D"),
                ("C", "D"),
                ("D", "E"),
                ("D", "F"),
                ("E", "G"),
                ("F", "G"),
            ],
        );
        let args = DiamondsArgs::default().pattern("G");
        let result = diamonds(&g, &args).unwrap();

        assert_eq!(result.subgraphs.len(), 1);
        let sg = &result.subgraphs[0];
        assert_eq!(sg.id.as_deref(), Some("D..G"));
        assert_eq!(sorted_node_ids(sg), vec!["D", "E", "F", "G"]);
    }

    #[test]
    fn min_depth_filtering() {
        // Diamond A -> B -> D, A -> C -> D: shortest path is 2 edges.
        let g = make_graph(
            &[("A", "A"), ("B", "B"), ("C", "C"), ("D", "D")],
            &[("A", "B"), ("A", "C"), ("B", "D"), ("C", "D")],
        );

        // min_depth=2: kept
        let args = DiamondsArgs::default().min_depth(2);
        let result = diamonds(&g, &args).unwrap();
        assert_eq!(result.subgraphs.len(), 1);

        // min_depth=3: filtered out
        let args = DiamondsArgs::default().min_depth(3);
        let result = diamonds(&g, &args).unwrap();
        assert!(result.subgraphs.is_empty());
    }

    #[test]
    fn cross_edges_included_by_default() {
        // Two disjoint diamonds with a cross-edge between them.
        // Diamond (A, D): A -> B -> D, A -> C -> D
        // Diamond (E, H): E -> F -> H, E -> G -> H
        // Cross-edge: B -> E (connects diamond nodes across different diamonds)
        let g = make_graph(
            &[
                ("A", "A"),
                ("B", "B"),
                ("C", "C"),
                ("D", "D"),
                ("E", "E"),
                ("F", "F"),
                ("G", "G"),
                ("H", "H"),
            ],
            &[
                ("A", "B"),
                ("A", "C"),
                ("B", "D"),
                ("B", "E"),
                ("C", "D"),
                ("E", "F"),
                ("E", "G"),
                ("F", "H"),
                ("G", "H"),
            ],
        );
        let result = diamonds(&g, &DiamondsArgs::default()).unwrap();

        assert_eq!(result.subgraphs.len(), 2);
        // B -> E is a cross-edge: B is in (A,D), E is in (E,H), but the edge
        // is not internal to either diamond.
        assert!(
            sorted_edge_pairs(&result).contains(&("B", "E")),
            "cross-edge B->E should be included by default"
        );
    }

    #[test]
    fn cross_edges_suppressed() {
        // Same graph as cross_edges_included_by_default, but with no_cross_edges.
        let g = make_graph(
            &[
                ("A", "A"),
                ("B", "B"),
                ("C", "C"),
                ("D", "D"),
                ("E", "E"),
                ("F", "F"),
                ("G", "G"),
                ("H", "H"),
            ],
            &[
                ("A", "B"),
                ("A", "C"),
                ("B", "D"),
                ("B", "E"),
                ("C", "D"),
                ("E", "F"),
                ("E", "G"),
                ("F", "H"),
                ("G", "H"),
            ],
        );
        let args = DiamondsArgs::default().no_cross_edges();
        let result = diamonds(&g, &args).unwrap();

        assert_eq!(result.subgraphs.len(), 2);
        assert!(
            !sorted_edge_pairs(&result).contains(&("B", "E")),
            "cross-edge B->E should be suppressed"
        );
        // Internal edges are still present.
        assert_eq!(
            sorted_edge_pairs(&result),
            vec![
                ("A", "B"),
                ("A", "C"),
                ("B", "D"),
                ("C", "D"),
                ("E", "F"),
                ("E", "G"),
                ("F", "H"),
                ("G", "H"),
            ]
        );
    }

    #[test]
    fn preserves_node_attributes() {
        let mut g = make_graph(
            &[("A", "A-label"), ("B", "B"), ("C", "C"), ("D", "D")],
            &[("A", "B"), ("A", "C"), ("B", "D"), ("C", "D")],
        );
        g.nodes.get_mut("A").unwrap().node_type = Some("lib".to_string());
        g.nodes
            .get_mut("A")
            .unwrap()
            .attrs
            .insert("color".to_string(), "red".to_string());
        g.clear_caches();

        let result = diamonds(&g, &DiamondsArgs::default()).unwrap();

        assert_eq!(result.subgraphs.len(), 1);
        let sg = &result.subgraphs[0];
        let a_info = sg.nodes.get("A").unwrap();
        assert_eq!(a_info.label, "A-label");
        assert_eq!(a_info.node_type.as_deref(), Some("lib"));
        assert_eq!(a_info.attrs.get("color").map(String::as_str), Some("red"));
    }

    #[test]
    fn empty_graph() {
        let g = DepGraph::default();
        let result = diamonds(&g, &DiamondsArgs::default()).unwrap();
        assert!(result.nodes.is_empty());
        assert!(result.edges.is_empty());
        assert!(result.subgraphs.is_empty());
    }
}
