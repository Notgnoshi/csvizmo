use std::collections::HashMap;

use cargo_metadata::{DependencyKind, Metadata, Package, PackageId};
use indexmap::IndexMap;

use crate::{DepGraph, Edge, NodeInfo};

/// Extract "name version" from a cargo package ID, stripping the source.
///
/// Handles two formats:
/// - Old: "name version (source)" -> "serde 1.0.217"
/// - New: "source#name@version" or "source#version" -> "serde 1.0.217" or "name version"
fn extract_name_version(id: &str) -> String {
    // Check if this is the new format (contains '#')
    if let Some(after_hash) = id.split('#').nth(1) {
        // Format: "source#name@version" or "source#version"
        if let Some((name, version)) = after_hash.split_once('@') {
            // Has '@', so: name@version
            format!("{} {}", name, version)
        } else {
            // No '@', need to extract name from path before '#'
            let name = id
                .split('#')
                .next()
                .and_then(|path| path.rsplit('/').next())
                .unwrap_or("unknown");
            format!("{} {}", name, after_hash)
        }
    } else {
        // Old format: "name version (source)"
        if let Some(name_version) = id.split('(').next() {
            name_version.trim().to_string()
        } else {
            id.to_string()
        }
    }
}

/// Extract just the package name from a cargo package ID.
///
/// Handles two formats:
/// - Old: "name version (source)" -> "serde"
/// - New: "source#name@version" -> "serde"
fn extract_package_name(id: &str) -> String {
    if let Some(after_hash) = id.split('#').nth(1) {
        // New format
        if let Some((name, _version)) = after_hash.split_once('@') {
            name.to_string()
        } else {
            // Path format, extract from before '#'
            id.split('#')
                .next()
                .and_then(|path| path.rsplit('/').next())
                .unwrap_or(id)
                .to_string()
        }
    } else {
        // Old format
        id.split_whitespace().next().unwrap_or(id).to_string()
    }
}

pub fn parse(input: &str) -> eyre::Result<DepGraph> {
    let metadata: Metadata = serde_json::from_str(input)?;

    let resolve = metadata
        .resolve
        .ok_or_else(|| eyre::eyre!("cargo metadata missing 'resolve' field"))?;

    // Build a map from PackageId to Package for looking up optional dependencies
    let package_map: HashMap<&PackageId, &Package> =
        metadata.packages.iter().map(|pkg| (&pkg.id, pkg)).collect();

    let mut graph = DepGraph::default();

    // Create nodes
    for node in &resolve.nodes {
        let mut attrs = IndexMap::new();

        // Get package info to extract name and version
        if let Some(pkg) = package_map.get(&node.id) {
            attrs.insert("version".to_string(), pkg.version.to_string());
        }

        // Store features if present
        if !node.features.is_empty() {
            attrs.insert("features".to_string(), node.features.join(","));
        }

        let node_id = extract_name_version(&node.id.repr);
        let label = package_map
            .get(&node.id)
            .map(|pkg| pkg.name.to_string())
            .unwrap_or_else(|| extract_package_name(&node.id.repr));

        graph.nodes.insert(
            node_id,
            NodeInfo {
                label: Some(label),
                attrs,
            },
        );
    }

    // Create edges
    for node in &resolve.nodes {
        let source_id = extract_name_version(&node.id.repr);
        let source_pkg = package_map.get(&node.id);

        for dep in &node.deps {
            let mut edge_attrs = IndexMap::new();

            // Collect dependency kinds for this edge
            let kinds: Vec<String> = dep
                .dep_kinds
                .iter()
                .map(|dk| match dk.kind {
                    DependencyKind::Normal => "normal",
                    DependencyKind::Development => "dev",
                    DependencyKind::Build => "build",
                    DependencyKind::Unknown => "unknown",
                })
                .map(String::from)
                .collect();

            // Store dependency kind on the edge
            if !kinds.is_empty() {
                edge_attrs.insert("kind".to_string(), kinds.join(","));
            }

            // Check if this is an optional dependency
            if let Some(pkg) = source_pkg
                && let Some(target_pkg) = package_map.get(&dep.pkg)
            {
                let target_name = target_pkg.name.to_string();
                if let Some(feature) = find_feature_for_optional_dep(pkg, &target_name) {
                    edge_attrs.insert("optional".to_string(), feature);
                }
            }

            graph.edges.push(Edge {
                from: source_id.clone(),
                to: extract_name_version(&dep.pkg.repr),
                label: None,
                attrs: edge_attrs,
            });
        }
    }

    Ok(graph)
}

/// Find which feature enables an optional dependency.
///
/// Returns the feature name if the dependency is optional and enabled by a feature.
fn find_feature_for_optional_dep(pkg: &Package, dep_name: &str) -> Option<String> {
    // Check if the dependency is optional
    let is_optional = pkg
        .dependencies
        .iter()
        .any(|d| d.name == dep_name && d.optional);

    if !is_optional {
        return None;
    }

    // Find which feature enables this dependency
    // Features can enable dependencies with "dep:name" or just "name" (implicit)
    for (feature_name, enables) in &pkg.features {
        for item in enables {
            // Check for "dep:name" or just "name"
            if item == &format!("dep:{}", dep_name) || item == dep_name {
                return Some(feature_name.clone());
            }
        }
    }

    // If no feature explicitly enables it, the dependency name itself might be a feature
    if pkg.features.contains_key(dep_name) {
        return Some(dep_name.to_string());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_name_version() {
        // Old format
        assert_eq!(
            extract_name_version(
                "serde 1.0.217 (registry+https://github.com/rust-lang/crates.io-index)"
            ),
            "serde 1.0.217"
        );
        assert_eq!(
            extract_name_version("my-crate 0.1.0 (path+file:///home/user/project)"),
            "my-crate 0.1.0"
        );
        assert_eq!(extract_name_version("simple 1.0.0"), "simple 1.0.0");

        // New format
        assert_eq!(
            extract_name_version(
                "registry+https://github.com/rust-lang/crates.io-index#serde@1.0.217"
            ),
            "serde 1.0.217"
        );
        assert_eq!(
            extract_name_version("path+file://~/src/csvizmo/crates/csvizmo-depgraph#0.5.0"),
            "csvizmo-depgraph 0.5.0"
        );
    }

    #[test]
    fn test_extract_package_name() {
        assert_eq!(
            extract_package_name(
                "serde 1.0.217 (registry+https://github.com/rust-lang/crates.io-index)"
            ),
            "serde"
        );
        assert_eq!(
            extract_package_name("my-crate 0.1.0 (path+file:///home/user/project)"),
            "my-crate"
        );
        assert_eq!(extract_package_name("simple"), "simple");
    }

    // NOTE: Unit tests for the parse function are not included here because
    // the cargo_metadata crate requires the full, valid JSON structure which
    // is tedious to maintain in unit tests. Instead, we rely on integration
    // tests using the real cargo-metadata.json fixture in tests/depconv.rs.
}
