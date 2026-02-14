use indexmap::IndexMap;

use crate::DepGraph;

/// Apply default visual styles based on semantic metadata.
///
/// Populates `attrs` with visual defaults (shape, style, color) based on
/// `node_type` and edge `kind`. Only sets attrs that are not already present,
/// so explicit styling takes priority over defaults.
///
/// This runs once between parse and emit, centralizing the mapping from
/// semantic metadata to visual attributes.
pub fn apply_default_styles(graph: &mut DepGraph) {
    for (_id, info) in &mut graph.nodes {
        if let Some(node_type) = &info.node_type {
            match node_type.as_str() {
                "proc-macro" => set_default(&mut info.attrs, "shape", "diamond"),
                "bin" => set_default(&mut info.attrs, "shape", "box"),
                "build-script" => set_default(&mut info.attrs, "shape", "note"),
                "optional" => set_default(&mut info.attrs, "style", "dashed"),
                "lib" => set_default(&mut info.attrs, "shape", "ellipse"),
                "test" => set_default(&mut info.attrs, "shape", "hexagon"),
                _ => {}
            }
        }
    }

    for edge in &mut graph.edges {
        if let Some(kind) = edge.attrs.get("kind").cloned() {
            let kinds: Vec<&str> = kind.split(',').map(|k| k.trim()).collect();
            if kinds.contains(&"dev") {
                set_default(&mut edge.attrs, "style", "dashed");
                set_default(&mut edge.attrs, "color", "gray60");
            } else if kinds.contains(&"build") {
                set_default(&mut edge.attrs, "style", "dashed");
            }
        }
    }

    for sg in &mut graph.subgraphs {
        apply_default_styles(sg);
    }
}

fn set_default(attrs: &mut IndexMap<String, String>, key: &str, val: &str) {
    if !attrs.contains_key(key) {
        attrs.insert(key.to_string(), val.to_string());
    }
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;

    use super::*;
    use crate::{Edge, NodeInfo};

    #[test]
    fn proc_macro_gets_diamond() {
        let mut graph = DepGraph {
            nodes: IndexMap::from([(
                "pm".into(),
                NodeInfo {
                    node_type: Some("proc-macro".into()),
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };
        apply_default_styles(&mut graph);
        assert_eq!(graph.nodes["pm"].attrs.get("shape").unwrap(), "diamond");
    }

    #[test]
    fn bin_gets_box() {
        let mut graph = DepGraph {
            nodes: IndexMap::from([(
                "b".into(),
                NodeInfo {
                    node_type: Some("bin".into()),
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };
        apply_default_styles(&mut graph);
        assert_eq!(graph.nodes["b"].attrs.get("shape").unwrap(), "box");
    }

    #[test]
    fn build_script_gets_note() {
        let mut graph = DepGraph {
            nodes: IndexMap::from([(
                "bs".into(),
                NodeInfo {
                    node_type: Some("build-script".into()),
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };
        apply_default_styles(&mut graph);
        assert_eq!(graph.nodes["bs"].attrs.get("shape").unwrap(), "note");
    }

    #[test]
    fn optional_gets_dashed() {
        let mut graph = DepGraph {
            nodes: IndexMap::from([(
                "opt".into(),
                NodeInfo {
                    node_type: Some("optional".into()),
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };
        apply_default_styles(&mut graph);
        assert_eq!(graph.nodes["opt"].attrs.get("style").unwrap(), "dashed");
    }

    #[test]
    fn lib_gets_ellipse() {
        let mut graph = DepGraph {
            nodes: IndexMap::from([(
                "l".into(),
                NodeInfo {
                    node_type: Some("lib".into()),
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };
        apply_default_styles(&mut graph);
        assert_eq!(graph.nodes["l"].attrs.get("shape").unwrap(), "ellipse");
    }

    #[test]
    fn test_gets_hexagon() {
        let mut graph = DepGraph {
            nodes: IndexMap::from([(
                "t".into(),
                NodeInfo {
                    node_type: Some("test".into()),
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };
        apply_default_styles(&mut graph);
        assert_eq!(graph.nodes["t"].attrs.get("shape").unwrap(), "hexagon");
    }

    #[test]
    fn no_override_existing_shape() {
        let mut graph = DepGraph {
            nodes: IndexMap::from([(
                "pm".into(),
                NodeInfo {
                    node_type: Some("proc-macro".into()),
                    attrs: IndexMap::from([("shape".into(), "box".into())]),
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };
        apply_default_styles(&mut graph);
        assert_eq!(graph.nodes["pm"].attrs.get("shape").unwrap(), "box");
    }

    #[test]
    fn no_override_existing_style() {
        let mut graph = DepGraph {
            nodes: IndexMap::from([(
                "opt".into(),
                NodeInfo {
                    node_type: Some("optional".into()),
                    attrs: IndexMap::from([("style".into(), "bold".into())]),
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };
        apply_default_styles(&mut graph);
        assert_eq!(graph.nodes["opt"].attrs.get("style").unwrap(), "bold");
    }

    #[test]
    fn unknown_type_no_attrs() {
        let mut graph = DepGraph {
            nodes: IndexMap::from([(
                "x".into(),
                NodeInfo {
                    node_type: Some("unknown-thing".into()),
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };
        apply_default_styles(&mut graph);
        assert!(graph.nodes["x"].attrs.is_empty());
    }

    #[test]
    fn no_type_no_attrs() {
        let mut graph = DepGraph {
            nodes: IndexMap::from([("x".into(), NodeInfo::default())]),
            ..Default::default()
        };
        apply_default_styles(&mut graph);
        assert!(graph.nodes["x"].attrs.is_empty());
    }

    #[test]
    fn edge_dev_kind() {
        let mut graph = DepGraph {
            edges: vec![Edge {
                from: "a".into(),
                to: "b".into(),
                attrs: IndexMap::from([("kind".into(), "dev".into())]),
                ..Default::default()
            }],
            ..Default::default()
        };
        apply_default_styles(&mut graph);
        assert_eq!(graph.edges[0].attrs.get("style").unwrap(), "dashed");
        assert_eq!(graph.edges[0].attrs.get("color").unwrap(), "gray60");
    }

    #[test]
    fn edge_build_kind() {
        let mut graph = DepGraph {
            edges: vec![Edge {
                from: "a".into(),
                to: "b".into(),
                attrs: IndexMap::from([("kind".into(), "build".into())]),
                ..Default::default()
            }],
            ..Default::default()
        };
        apply_default_styles(&mut graph);
        assert_eq!(graph.edges[0].attrs.get("style").unwrap(), "dashed");
        assert!(graph.edges[0].attrs.get("color").is_none());
    }

    #[test]
    fn edge_normal_kind_no_styling() {
        let mut graph = DepGraph {
            edges: vec![Edge {
                from: "a".into(),
                to: "b".into(),
                attrs: IndexMap::from([("kind".into(), "normal".into())]),
                ..Default::default()
            }],
            ..Default::default()
        };
        apply_default_styles(&mut graph);
        assert!(graph.edges[0].attrs.get("style").is_none());
        assert!(graph.edges[0].attrs.get("color").is_none());
    }

    #[test]
    fn edge_mixed_kind_with_dev() {
        let mut graph = DepGraph {
            edges: vec![Edge {
                from: "a".into(),
                to: "b".into(),
                attrs: IndexMap::from([("kind".into(), "normal,dev".into())]),
                ..Default::default()
            }],
            ..Default::default()
        };
        apply_default_styles(&mut graph);
        assert_eq!(graph.edges[0].attrs.get("style").unwrap(), "dashed");
        assert_eq!(graph.edges[0].attrs.get("color").unwrap(), "gray60");
    }

    #[test]
    fn edge_no_override_existing_style() {
        let mut graph = DepGraph {
            edges: vec![Edge {
                from: "a".into(),
                to: "b".into(),
                attrs: IndexMap::from([
                    ("kind".into(), "dev".into()),
                    ("style".into(), "bold".into()),
                ]),
                ..Default::default()
            }],
            ..Default::default()
        };
        apply_default_styles(&mut graph);
        assert_eq!(graph.edges[0].attrs.get("style").unwrap(), "bold");
        // color still gets added since it wasn't present
        assert_eq!(graph.edges[0].attrs.get("color").unwrap(), "gray60");
    }

    #[test]
    fn subgraph_recursion() {
        let mut graph = DepGraph {
            subgraphs: vec![DepGraph {
                nodes: IndexMap::from([(
                    "inner".into(),
                    NodeInfo {
                        node_type: Some("bin".into()),
                        ..Default::default()
                    },
                )]),
                edges: vec![Edge {
                    from: "a".into(),
                    to: "b".into(),
                    attrs: IndexMap::from([("kind".into(), "dev".into())]),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        apply_default_styles(&mut graph);
        assert_eq!(
            graph.subgraphs[0].nodes["inner"]
                .attrs
                .get("shape")
                .unwrap(),
            "box"
        );
        assert_eq!(
            graph.subgraphs[0].edges[0].attrs.get("style").unwrap(),
            "dashed"
        );
    }
}
