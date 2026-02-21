use std::collections::HashSet;

use graphrs::algorithms::community::{leiden, louvain};
use graphrs::{Edge, Graph, GraphSpecs, Node};

use super::clusters_to_depgraph;
use crate::DepGraph;

/// Convert a DepGraph into a graphrs Graph.
fn depgraph_to_graphrs(graph: &DepGraph, directed: bool) -> eyre::Result<Graph<String, ()>> {
    let specs = if directed {
        GraphSpecs::directed_create_missing()
    } else {
        GraphSpecs::undirected_create_missing()
    };

    let all_nodes = graph.all_nodes();
    let all_edges = graph.all_edges();

    let nodes: Vec<_> = all_nodes
        .keys()
        .map(|id| Node::from_name(id.clone()))
        .collect();

    let edges: Vec<_> = all_edges
        .iter()
        .map(|e| Edge::new(e.from.clone(), e.to.clone()))
        .collect();

    let g = Graph::new_from_nodes_and_edges(nodes, edges, specs)
        .map_err(|e| eyre::eyre!("graphrs error: {e}"))?;

    Ok(g)
}

/// Convert graphrs community result (`Vec<HashSet<String>>`) to our partition format.
fn communities_to_partition(graph: &DepGraph, communities: Vec<HashSet<String>>) -> Vec<Vec<&str>> {
    let all_nodes = graph.all_nodes();
    communities
        .iter()
        .map(|community| {
            let mut ids: Vec<&str> = all_nodes
                .keys()
                .filter(|id| community.contains(id.as_str()))
                .map(|id| id.as_str())
                .collect();
            ids.sort();
            ids
        })
        .collect()
}

/// Run the Louvain community detection algorithm on the dependency graph.
pub fn louvain_clustering(
    graph: &DepGraph,
    directed: bool,
    resolution: f64,
    seed: Option<u64>,
) -> eyre::Result<DepGraph> {
    let g = depgraph_to_graphrs(graph, directed)?;

    let communities = louvain::louvain_communities(&g, false, Some(resolution), None, seed)
        .map_err(|e| eyre::eyre!("louvain error: {e}"))?;

    let partition = communities_to_partition(graph, communities);
    Ok(clusters_to_depgraph(graph, &partition))
}

/// Run the Leiden community detection algorithm on the dependency graph.
pub fn leiden_clustering(
    graph: &DepGraph,
    directed: bool,
    resolution: f64,
) -> eyre::Result<DepGraph> {
    let g = depgraph_to_graphrs(graph, directed)?;

    let communities = leiden::leiden(
        &g,
        false,
        leiden::QualityFunction::CPM,
        Some(resolution),
        None,
        None,
    )
    .map_err(|e| eyre::eyre!("leiden error: {e}"))?;

    let partition = communities_to_partition(graph, communities);
    Ok(clusters_to_depgraph(graph, &partition))
}
