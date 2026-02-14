use std::collections::{HashSet, VecDeque};

use indexmap::IndexMap;
use petgraph::Direction;
use petgraph::graph::{DiGraph, NodeIndex};

#[derive(Clone, Debug, Default)]
pub struct DepGraph {
    /// Graph or subgraph identifier (e.g. DOT `digraph <id>` / `subgraph <id>`).
    pub id: Option<String>,
    /// Graph-level attributes (e.g. DOT `rankdir`, `label`, `color`).
    pub attrs: IndexMap<String, String>,
    pub nodes: IndexMap<String, NodeInfo>,
    pub edges: Vec<Edge>,
    /// Nested subgraphs, each owning its own nodes and edges.
    pub subgraphs: Vec<DepGraph>,
}

impl DepGraph {
    /// Collect all nodes from this graph and all nested subgraphs.
    ///
    /// This function recurses over subgraphs to aggregate the results. If you're doing repeated
    /// lookups, consider caching the results.
    pub fn all_nodes(&self) -> IndexMap<&str, &NodeInfo> {
        let mut result = IndexMap::new();
        // Recurse over the subgraphs in DFS order to collect nodes from each
        self.collect_nodes(&mut result);
        result
    }

    fn collect_nodes<'a>(&'a self, result: &mut IndexMap<&'a str, &'a NodeInfo>) {
        for (id, info) in &self.nodes {
            result.insert(id.as_str(), info);
        }
        for sg in &self.subgraphs {
            sg.collect_nodes(result);
        }
    }

    /// Collect all edges from this graph and all nested subgraphs.
    ///
    /// This function recurses over subgraphs to aggregate the results. If you're doing repeated
    /// lookups, consider caching the results.
    pub fn all_edges(&self) -> Vec<&Edge> {
        let mut result = Vec::new();
        // Recurse over the subgraphs in DFS order to collect edges from each
        self.collect_edges(&mut result);
        result
    }

    fn collect_edges<'a>(&'a self, result: &mut Vec<&'a Edge>) {
        result.extend(&self.edges);
        for sg in &self.subgraphs {
            sg.collect_edges(result);
        }
    }

    /// Build an adjacency list from all edges across all subgraphs.
    ///
    /// This function recurses over subgraphs to aggregate the results. If you're doing repeated
    /// lookups, consider caching the results.
    pub fn adjacency_list(&self) -> IndexMap<&str, Vec<&str>> {
        let mut adj = IndexMap::new();
        for edge in self.all_edges() {
            adj.entry(edge.from.as_str())
                .or_insert_with(Vec::new)
                .push(edge.to.as_str());
        }
        adj
    }
}

#[derive(Clone, Debug)]
pub struct NodeInfo {
    pub label: String,
    /// Node type/kind (e.g. "lib", "bin", "proc-macro", "build-script").
    /// Semantics are format-specific on input; normalized to canonical names where possible.
    /// Formats that don't support types leave this as None.
    pub node_type: Option<String>,
    /// Arbitrary extra attributes. Parsers populate these from format-specific features;
    /// emitters carry them through where the output format allows.
    pub attrs: IndexMap<String, String>,
}

impl NodeInfo {
    /// Create a new NodeInfo with the given label.
    /// Node type and attributes are initialized to their defaults (None and empty, respectively).
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            node_type: None,
            attrs: Default::default(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    /// Arbitrary extra attributes (e.g. DOT `style`, `color`).
    pub attrs: IndexMap<String, String>,
}

/// A flattened view of a [`DepGraph`] as a petgraph [`DiGraph`].
///
/// Bridges `DepGraph` (which has nested subgraphs and string-keyed maps) with petgraph's
/// graph algorithms by flattening all nodes and edges into a single directed graph.
pub struct FlatGraphView<'a> {
    /// The source dependency graph.
    pub graph: &'a DepGraph,
    /// Flattened petgraph with all nodes and edges from all subgraph levels.
    pub pg: DiGraph<(), ()>,
    /// Map from node ID string to petgraph NodeIndex.
    pub id_to_idx: IndexMap<&'a str, NodeIndex>,
    /// Map from petgraph NodeIndex (as usize index) to node ID string.
    pub idx_to_id: Vec<&'a str>,
}

impl<'a> FlatGraphView<'a> {
    /// Create a new `FlatGraphView` from a `DepGraph`.
    ///
    /// Collects all nodes and edges from the graph and its nested subgraphs into a flat
    /// petgraph `DiGraph`. Edges whose endpoints are not present in the node set are skipped.
    pub fn new(graph: &'a DepGraph) -> Self {
        let all_nodes = graph.all_nodes();
        let all_edges = graph.all_edges();

        let mut pg = DiGraph::new();
        let mut id_to_idx = IndexMap::new();
        let mut idx_to_id = Vec::with_capacity(all_nodes.len());

        for id in all_nodes.keys() {
            let idx = pg.add_node(());
            id_to_idx.insert(*id, idx);
            idx_to_id.push(*id);
        }

        for edge in &all_edges {
            let from = id_to_idx.get(edge.from.as_str());
            let to = id_to_idx.get(edge.to.as_str());
            if let (Some(&from_idx), Some(&to_idx)) = (from, to) {
                pg.add_edge(from_idx, to_idx, ());
            }
        }

        Self {
            graph,
            pg,
            id_to_idx,
            idx_to_id,
        }
    }

    /// Return all root nodes (nodes with no incoming edges).
    pub fn roots(&self) -> impl Iterator<Item = NodeIndex> + '_ {
        self.pg.node_indices().filter(|&idx| {
            self.pg
                .neighbors_directed(idx, Direction::Incoming)
                .next()
                .is_none()
        })
    }

    /// BFS from `seeds` following edges in `direction`, returning all visited nodes.
    ///
    /// If `max_depth` is `Some(n)`, only nodes within `n` hops of a seed are included.
    /// The seeds themselves are always included (depth 0).
    pub fn bfs(
        &self,
        seeds: impl IntoIterator<Item = NodeIndex>,
        direction: Direction,
        max_depth: Option<usize>,
    ) -> HashSet<NodeIndex> {
        let mut visited = HashSet::new();
        let mut queue: VecDeque<(NodeIndex, usize)> = VecDeque::new();
        for seed in seeds {
            if visited.insert(seed) {
                queue.push_back((seed, 0));
            }
        }

        while let Some((node, depth)) = queue.pop_front() {
            if max_depth.is_some_and(|max| depth >= max) {
                continue;
            }
            for neighbor in self.pg.neighbors_directed(node, direction) {
                if visited.insert(neighbor) {
                    queue.push_back((neighbor, depth + 1));
                }
            }
        }

        visited
    }

    /// Filter the original `DepGraph` to only include nodes in the `keep` set.
    ///
    /// Returns a new `DepGraph` that preserves the original subgraph structure but only
    /// contains nodes whose `NodeIndex` is in `keep`, plus edges where both endpoints survive.
    /// Empty subgraphs (no nodes and no non-empty child subgraphs) are dropped.
    pub fn filter(&self, keep: &HashSet<NodeIndex>) -> DepGraph {
        let keep_ids: HashSet<&str> = keep
            .iter()
            .filter_map(|idx| self.idx_to_id.get(idx.index()).copied())
            .collect();
        filter_depgraph(self.graph, &keep_ids)
    }
}

fn filter_depgraph(graph: &DepGraph, keep: &HashSet<&str>) -> DepGraph {
    DepGraph {
        id: graph.id.clone(),
        attrs: graph.attrs.clone(),
        nodes: graph
            .nodes
            .iter()
            .filter(|(id, _)| keep.contains(id.as_str()))
            .map(|(id, info)| (id.clone(), info.clone()))
            .collect(),
        edges: graph
            .edges
            .iter()
            .filter(|e| keep.contains(e.from.as_str()) && keep.contains(e.to.as_str()))
            .cloned()
            .collect(),
        subgraphs: graph
            .subgraphs
            .iter()
            .map(|sg| filter_depgraph(sg, keep))
            .filter(|sg| !sg.nodes.is_empty() || !sg.subgraphs.is_empty())
            .collect(),
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
            id: None,
            attrs: IndexMap::new(),
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
        }
    }

    #[test]
    fn new_empty() {
        let g = DepGraph::default();
        let view = FlatGraphView::new(&g);
        assert_eq!(view.pg.node_count(), 0);
        assert_eq!(view.pg.edge_count(), 0);
        assert!(view.id_to_idx.is_empty());
        assert!(view.idx_to_id.is_empty());
    }

    #[test]
    fn new_flat() {
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c")],
            vec![],
        );
        let view = FlatGraphView::new(&g);

        assert_eq!(view.pg.node_count(), 3);
        assert_eq!(view.pg.edge_count(), 2);

        // Round-trip: id -> idx -> id
        for &id in &["a", "b", "c"] {
            let idx = view.id_to_idx[id];
            assert_eq!(view.idx_to_id[idx.index()], id);
        }
    }

    #[test]
    fn new_with_subgraphs() {
        let sub = make_graph(&[("c", "C")], &[("a", "c")], vec![]);
        let g = make_graph(&[("a", "A"), ("b", "B")], &[("a", "b")], vec![sub]);
        let view = FlatGraphView::new(&g);

        assert_eq!(view.pg.node_count(), 3);
        // a->b from root, a->c from subgraph
        assert_eq!(view.pg.edge_count(), 2);
        assert!(view.id_to_idx.contains_key("c"));
    }

    #[test]
    fn new_skips_dangling_edges() {
        let g = make_graph(
            &[("a", "A")],
            &[("a", "b"), ("x", "a")], // b and x don't exist
            vec![],
        );
        let view = FlatGraphView::new(&g);

        assert_eq!(view.pg.node_count(), 1);
        assert_eq!(view.pg.edge_count(), 0);
    }

    #[test]
    fn filter_keeps_matching_nodes() {
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c"), ("a", "c")],
            vec![],
        );
        let view = FlatGraphView::new(&g);

        let keep: HashSet<NodeIndex> = ["a", "b"].iter().map(|id| view.id_to_idx[*id]).collect();
        let filtered = view.filter(&keep);

        assert_eq!(filtered.nodes.len(), 2);
        assert!(filtered.nodes.contains_key("a"));
        assert!(filtered.nodes.contains_key("b"));
        assert_eq!(filtered.edges.len(), 1);
        assert_eq!(filtered.edges[0].from, "a");
        assert_eq!(filtered.edges[0].to, "b");
    }

    #[test]
    fn filter_drops_unmatched_edges() {
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c")],
            vec![],
        );
        let view = FlatGraphView::new(&g);

        // Keep a and c but not b -- both edges touch b so both are dropped
        let keep: HashSet<NodeIndex> = ["a", "c"].iter().map(|id| view.id_to_idx[*id]).collect();
        let filtered = view.filter(&keep);

        assert_eq!(filtered.nodes.len(), 2);
        assert!(filtered.edges.is_empty());
    }

    #[test]
    fn filter_preserves_subgraph_structure() {
        let sub = make_graph(&[("c", "C")], &[], vec![]);
        let g = make_graph(&[("a", "A"), ("b", "B")], &[("a", "b")], vec![sub]);
        let view = FlatGraphView::new(&g);

        // Keep all three nodes
        let keep: HashSet<NodeIndex> = ["a", "b", "c"]
            .iter()
            .map(|id| view.id_to_idx[*id])
            .collect();
        let filtered = view.filter(&keep);

        assert_eq!(filtered.nodes.len(), 2); // a, b at root
        assert_eq!(filtered.subgraphs.len(), 1);
        assert_eq!(filtered.subgraphs[0].nodes.len(), 1); // c in subgraph
        assert!(filtered.subgraphs[0].nodes.contains_key("c"));
    }

    #[test]
    fn filter_drops_empty_subgraphs() {
        let sub = make_graph(&[("c", "C")], &[], vec![]);
        let g = make_graph(&[("a", "A"), ("b", "B")], &[], vec![sub]);
        let view = FlatGraphView::new(&g);

        // Keep only root nodes, subgraph node c is excluded
        let keep: HashSet<NodeIndex> = ["a", "b"].iter().map(|id| view.id_to_idx[*id]).collect();
        let filtered = view.filter(&keep);

        assert_eq!(filtered.nodes.len(), 2);
        assert!(filtered.subgraphs.is_empty());
    }

    #[test]
    fn filter_preserves_subgraph_attrs() {
        let mut sub = make_graph(&[("c", "C")], &[], vec![]);
        sub.id = Some("cluster_0".to_string());
        sub.attrs.insert("color".to_string(), "blue".to_string());

        let g = make_graph(&[("a", "A")], &[], vec![sub]);
        let view = FlatGraphView::new(&g);

        let keep: HashSet<NodeIndex> = ["a", "c"].iter().map(|id| view.id_to_idx[*id]).collect();
        let filtered = view.filter(&keep);

        assert_eq!(filtered.subgraphs.len(), 1);
        assert_eq!(filtered.subgraphs[0].id.as_deref(), Some("cluster_0"));
        assert_eq!(
            filtered.subgraphs[0].attrs.get("color").map(String::as_str),
            Some("blue")
        );
    }

    // -- roots --

    #[test]
    fn roots_empty_graph() {
        let g = DepGraph::default();
        let view = FlatGraphView::new(&g);
        assert_eq!(view.roots().count(), 0);
    }

    #[test]
    fn roots_no_edges() {
        let g = make_graph(&[("a", "A"), ("b", "B")], &[], vec![]);
        let view = FlatGraphView::new(&g);
        let root_ids: Vec<&str> = view
            .roots()
            .map(|idx| view.idx_to_id[idx.index()])
            .collect();
        assert_eq!(root_ids, vec!["a", "b"]);
    }

    #[test]
    fn roots_chain() {
        // a -> b -> c: only a is a root
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c")],
            vec![],
        );
        let view = FlatGraphView::new(&g);
        let root_ids: Vec<&str> = view
            .roots()
            .map(|idx| view.idx_to_id[idx.index()])
            .collect();
        assert_eq!(root_ids, vec!["a"]);
    }

    #[test]
    fn roots_diamond() {
        // a -> b, a -> c, b -> d, c -> d
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C"), ("d", "D")],
            &[("a", "b"), ("a", "c"), ("b", "d"), ("c", "d")],
            vec![],
        );
        let view = FlatGraphView::new(&g);
        let root_ids: Vec<&str> = view
            .roots()
            .map(|idx| view.idx_to_id[idx.index()])
            .collect();
        assert_eq!(root_ids, vec!["a"]);
    }

    #[test]
    fn roots_cycle() {
        // a -> b -> c -> a: every node has an incoming edge, no roots
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c"), ("c", "a")],
            vec![],
        );
        let view = FlatGraphView::new(&g);
        assert_eq!(view.roots().count(), 0);
    }

    // -- bfs --

    #[test]
    fn bfs_outgoing_full() {
        // a -> b -> c
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c")],
            vec![],
        );
        let view = FlatGraphView::new(&g);
        let result = view.bfs([view.id_to_idx["a"]], Direction::Outgoing, None);
        let mut ids: Vec<&str> = result
            .iter()
            .map(|idx| view.idx_to_id[idx.index()])
            .collect();
        ids.sort();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn bfs_incoming_full() {
        // a -> b -> c: ancestors of c = {a, b, c}
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c")],
            vec![],
        );
        let view = FlatGraphView::new(&g);
        let result = view.bfs([view.id_to_idx["c"]], Direction::Incoming, None);
        let mut ids: Vec<&str> = result
            .iter()
            .map(|idx| view.idx_to_id[idx.index()])
            .collect();
        ids.sort();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn bfs_depth_limited() {
        // a -> b -> c -> d: depth 1 from a = {a, b}
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C"), ("d", "D")],
            &[("a", "b"), ("b", "c"), ("c", "d")],
            vec![],
        );
        let view = FlatGraphView::new(&g);
        let result = view.bfs([view.id_to_idx["a"]], Direction::Outgoing, Some(1));
        let mut ids: Vec<&str> = result
            .iter()
            .map(|idx| view.idx_to_id[idx.index()])
            .collect();
        ids.sort();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn bfs_depth_zero() {
        // depth 0 from a = just {a}
        let g = make_graph(&[("a", "A"), ("b", "B")], &[("a", "b")], vec![]);
        let view = FlatGraphView::new(&g);
        let result = view.bfs([view.id_to_idx["a"]], Direction::Outgoing, Some(0));
        let ids: Vec<&str> = result
            .iter()
            .map(|idx| view.idx_to_id[idx.index()])
            .collect();
        assert_eq!(ids, vec!["a"]);
    }

    #[test]
    fn bfs_multiple_seeds() {
        // a -> b, c -> d: seeds {a, c} outgoing = {a, b, c, d}
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C"), ("d", "D")],
            &[("a", "b"), ("c", "d")],
            vec![],
        );
        let view = FlatGraphView::new(&g);
        let result = view.bfs(
            [view.id_to_idx["a"], view.id_to_idx["c"]],
            Direction::Outgoing,
            None,
        );
        let mut ids: Vec<&str> = result
            .iter()
            .map(|idx| view.idx_to_id[idx.index()])
            .collect();
        ids.sort();
        assert_eq!(ids, vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn bfs_cycle() {
        // a -> b -> c -> a: full traversal doesn't loop forever
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c"), ("c", "a")],
            vec![],
        );
        let view = FlatGraphView::new(&g);
        let result = view.bfs([view.id_to_idx["a"]], Direction::Outgoing, None);
        assert_eq!(result.len(), 3);
    }
}
