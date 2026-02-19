use std::collections::HashSet;

use petgraph::Direction;
use petgraph::algo::{connected_components, tarjan_scc, toposort};

use crate::{DepGraph, FlatGraphView};

#[derive(Debug)]
pub struct GraphMetrics {
    pub nodes: usize,
    pub edges: usize,
    pub roots: usize,
    pub leaves: usize,
    pub max_depth: Option<usize>,
    pub max_fan_out: usize,
    pub max_fan_in: usize,
    pub avg_fan_out: f64,
    pub density: f64,
    pub cycles: usize,
    pub diamonds: usize,
    pub components: usize,
}

pub fn metrics(graph: &DepGraph) -> GraphMetrics {
    let view = FlatGraphView::new(graph);
    let node_count = view.pg.node_count();
    let edge_count = view.pg.edge_count();

    let mut roots = 0usize;
    let mut leaves = 0usize;
    let mut max_fan_out = 0usize;
    let mut max_fan_in = 0usize;

    for idx in view.pg.node_indices() {
        let in_deg = view.pg.neighbors_directed(idx, Direction::Incoming).count();
        let out_deg = view.pg.neighbors_directed(idx, Direction::Outgoing).count();
        if in_deg == 0 {
            roots += 1;
        }
        if out_deg == 0 {
            leaves += 1;
        }
        max_fan_in = max_fan_in.max(in_deg);
        max_fan_out = max_fan_out.max(out_deg);
    }

    let avg_fan_out = if node_count > 0 {
        edge_count as f64 / node_count as f64
    } else {
        0.0
    };

    let density = if node_count > 1 {
        edge_count as f64 / (node_count as f64 * (node_count as f64 - 1.0))
    } else {
        0.0
    };

    // Cycles: count SCCs with 2+ nodes
    let sccs = tarjan_scc(&view.pg);
    let cycle_count = sccs.iter().filter(|scc| scc.len() >= 2).count();

    // Max depth: longest path from any root to any leaf via topo-order DP
    let max_depth = if cycle_count > 0 {
        None
    } else {
        match toposort(&view.pg, None) {
            Ok(sorted) => {
                let mut dist = vec![0usize; node_count];
                let mut max_d = 0usize;
                for &node in &sorted {
                    let d = dist[node.index()];
                    for neighbor in view.pg.neighbors_directed(node, Direction::Outgoing) {
                        let nd = d + 1;
                        if nd > dist[neighbor.index()] {
                            dist[neighbor.index()] = nd;
                        }
                        max_d = max_d.max(nd);
                    }
                }
                Some(max_d)
            }
            Err(_) => None,
        }
    };

    // Weakly connected components
    let components = connected_components(&view.pg);

    // Diamonds: nodes with in-degree >= 2 whose parents share a common ancestor
    let diamonds = count_diamonds(&view);

    GraphMetrics {
        nodes: node_count,
        edges: edge_count,
        roots,
        leaves,
        max_depth,
        max_fan_out,
        max_fan_in,
        avg_fan_out,
        density,
        cycles: cycle_count,
        diamonds,
        components,
    }
}

// Count "merge points" -- nodes with 2+ parents that share a common ancestor.
fn count_diamonds(view: &FlatGraphView) -> usize {
    let mut count = 0;
    for idx in view.pg.node_indices() {
        let parents: Vec<_> = view
            .pg
            .neighbors_directed(idx, Direction::Incoming)
            .collect();
        if parents.len() < 2 {
            continue;
        }

        // For each parent, compute the set of ancestors (excluding the node itself).
        // If any two parents share an ancestor, this node is a diamond.
        let mut is_diamond = false;
        let mut seen_ancestors: HashSet<_> = HashSet::new();
        for &parent in &parents {
            let ancestors = view.bfs([parent], Direction::Incoming, None);
            for ancestor in &ancestors {
                if !seen_ancestors.insert(*ancestor) {
                    is_diamond = true;
                    break;
                }
            }
            if is_diamond {
                break;
            }
        }
        if is_diamond {
            count += 1;
        }
    }
    count
}

impl std::fmt::Display for GraphMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "nodes\t{}", self.nodes)?;
        writeln!(f, "edges\t{}", self.edges)?;
        writeln!(f, "roots\t{}", self.roots)?;
        writeln!(f, "leaves\t{}", self.leaves)?;
        match self.max_depth {
            Some(d) => writeln!(f, "max_depth\t{d}")?,
            None => writeln!(f, "max_depth\t")?,
        }
        writeln!(f, "max_fan_out\t{}", self.max_fan_out)?;
        writeln!(f, "max_fan_in\t{}", self.max_fan_in)?;
        writeln!(f, "avg_fan_out\t{:.2}", self.avg_fan_out)?;
        writeln!(f, "density\t{:.6}", self.density)?;
        writeln!(f, "cycles\t{}", self.cycles)?;
        writeln!(f, "diamonds\t{}", self.diamonds)?;
        writeln!(f, "components\t{}", self.components)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Edge, NodeInfo};

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

    #[test]
    fn empty_graph() {
        let g = DepGraph::default();
        let m = metrics(&g);
        assert_eq!(m.nodes, 0);
        assert_eq!(m.edges, 0);
        assert_eq!(m.roots, 0);
        assert_eq!(m.leaves, 0);
        assert_eq!(m.max_depth, Some(0));
        assert_eq!(m.max_fan_out, 0);
        assert_eq!(m.max_fan_in, 0);
        assert_eq!(m.avg_fan_out, 0.0);
        assert_eq!(m.density, 0.0);
        assert_eq!(m.cycles, 0);
        assert_eq!(m.diamonds, 0);
        assert_eq!(m.components, 0);
    }

    #[test]
    fn single_node() {
        let g = make_graph(&[("a", "A")], &[]);
        let m = metrics(&g);
        assert_eq!(m.nodes, 1);
        assert_eq!(m.edges, 0);
        assert_eq!(m.roots, 1);
        assert_eq!(m.leaves, 1);
        assert_eq!(m.max_depth, Some(0));
        assert_eq!(m.density, 0.0);
        assert_eq!(m.components, 1);
    }

    #[test]
    fn chain() {
        // a -> b -> c
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c")],
        );
        let m = metrics(&g);
        assert_eq!(m.nodes, 3);
        assert_eq!(m.edges, 2);
        assert_eq!(m.roots, 1);
        assert_eq!(m.leaves, 1);
        assert_eq!(m.max_depth, Some(2));
        assert_eq!(m.max_fan_out, 1);
        assert_eq!(m.max_fan_in, 1);
        assert!((m.avg_fan_out - 2.0 / 3.0).abs() < 1e-9);
        assert!((m.density - 2.0 / 6.0).abs() < 1e-9);
        assert_eq!(m.cycles, 0);
        assert_eq!(m.diamonds, 0);
        assert_eq!(m.components, 1);
    }

    #[test]
    fn diamond() {
        // a -> b, a -> c, b -> d, c -> d
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C"), ("d", "D")],
            &[("a", "b"), ("a", "c"), ("b", "d"), ("c", "d")],
        );
        let m = metrics(&g);
        assert_eq!(m.nodes, 4);
        assert_eq!(m.edges, 4);
        assert_eq!(m.roots, 1);
        assert_eq!(m.leaves, 1);
        assert_eq!(m.max_depth, Some(2));
        assert_eq!(m.max_fan_out, 2);
        assert_eq!(m.max_fan_in, 2);
        assert_eq!(m.cycles, 0);
        assert_eq!(m.diamonds, 1);
        assert_eq!(m.components, 1);
    }

    #[test]
    fn cycle_graph() {
        // a -> b -> c -> a
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c"), ("c", "a")],
        );
        let m = metrics(&g);
        assert_eq!(m.cycles, 1);
        assert!(m.max_depth.is_none());
        assert_eq!(m.roots, 0);
        assert_eq!(m.leaves, 0);
    }

    #[test]
    fn disjoint_components() {
        // a -> b, c -> d (two components)
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C"), ("d", "D")],
            &[("a", "b"), ("c", "d")],
        );
        let m = metrics(&g);
        assert_eq!(m.components, 2);
        assert_eq!(m.roots, 2);
        assert_eq!(m.leaves, 2);
    }

    #[test]
    fn display_format() {
        let g = make_graph(
            &[("a", "A"), ("b", "B"), ("c", "C")],
            &[("a", "b"), ("b", "c")],
        );
        let m = metrics(&g);
        let output = m.to_string();
        assert!(output.contains("nodes\t3\n"));
        assert!(output.contains("edges\t2\n"));
        assert!(output.contains("max_depth\t2\n"));
    }
}
