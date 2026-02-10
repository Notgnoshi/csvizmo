use dot_parser::ast;
use either::Either;

use crate::{DepGraph, Edge, NodeInfo};

/// Decode a string value from the `dot-parser` crate.
///
/// The dot-parser is inconsistent: it strips outer quotes from attribute values
/// but preserves them on node IDs, edge endpoints, and graph names. Escape
/// sequences like `\"` are preserved as-is in both cases.
///
/// This function:
/// 1. Strips surrounding `"..."` if present (needed for node IDs / endpoints).
/// 2. Unescapes `\"` -> `"` (needed for both -- attribute values still have escapes).
///
/// We intentionally do NOT decode `\\` -> `\`. DOT uses `\n`, `\l`, `\r` as label
/// formatting directives, and decoding `\\` would make `\\n` (literal backslash + n)
/// indistinguishable from `\n` (centered newline), corrupting DOT->DOT round-trips.
pub(crate) fn unquote(s: &str) -> String {
    let inner = if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        &s[1..s.len() - 1]
    } else {
        s
    };
    inner.replace("\\\"", "\"")
}

type AstGraph<'a> = ast::Graph<(ast::ID<'a>, ast::ID<'a>)>;
type AstStmt<'a> = ast::Stmt<(ast::ID<'a>, ast::ID<'a>)>;
type AstAttrList<'a> = ast::AttrList<(ast::ID<'a>, ast::ID<'a>)>;
type AstSubgraph<'a> = ast::Subgraph<(ast::ID<'a>, ast::ID<'a>)>;
type AstEdgeStmt<'a> = ast::EdgeStmt<(ast::ID<'a>, ast::ID<'a>)>;

/// Convert an `ast::ID` to a String. The ID's inner field is private, so
/// we must use the `Into<String>` impl which consumes the value.
fn id_to_string(id: &ast::ID) -> String {
    let s: String = id.clone().into();
    s
}

pub fn parse(input: &str) -> eyre::Result<DepGraph> {
    let ast_graph: AstGraph =
        ast::Graph::try_from(input).map_err(|e| eyre::eyre!("DOT parse error: {e}"))?;

    let mut dep = DepGraph {
        id: ast_graph.name.map(|n| unquote(&n)),
        ..Default::default()
    };

    walk_stmts(&ast_graph.stmts.stmts, &mut dep);
    dep.nodes.sort_keys();

    Ok(dep)
}

/// Walk a list of AST statements, populating nodes, edges, attrs, and subgraphs
/// on the given DepGraph.
fn walk_stmts(stmts: &[AstStmt], dep: &mut DepGraph) {
    let mut subgraphs = Vec::new();

    for stmt in stmts {
        match stmt {
            AstStmt::NodeStmt(node_stmt) => {
                add_node(node_stmt, dep);
            }
            AstStmt::EdgeStmt(edge_stmt) => {
                add_edges(edge_stmt, dep);
            }
            AstStmt::AttrStmt(attr_stmt) => match attr_stmt {
                ast::AttrStmt::Graph(attr_list) => {
                    extract_graph_attrs(attr_list, &mut dep.attrs);
                }
                // `node [fontsize="12"]` and `edge [style=invis]` are default
                // attribute statements -- rendering hints, not semantic data.
                // The canonical::Graph conversion used to apply these per-node,
                // but we skip them intentionally.
                ast::AttrStmt::Node(_) | ast::AttrStmt::Edge(_) => {}
            },
            AstStmt::IDEq(k, v) => {
                dep.attrs.insert(unquote(k), unquote(v));
            }
            AstStmt::Subgraph(sub) => {
                subgraphs.push(collect_subgraph(sub));
            }
        }
    }

    dep.subgraphs = subgraphs;
    remove_implicit_duplicates(dep);
}

/// Remove nodes from this level that were implicitly created by edge processing
/// but are explicitly declared (with label or attrs) in a descendant subgraph.
///
/// This runs bottom-up: inner subgraphs are already cleaned by their own
/// `walk_stmts` call before the parent runs this.
fn remove_implicit_duplicates(dep: &mut DepGraph) {
    if dep.subgraphs.is_empty() {
        return;
    }
    let subgraph_nodes = dep
        .subgraphs
        .iter()
        .flat_map(|sg| sg.all_nodes())
        .collect::<indexmap::IndexMap<&str, _>>();
    dep.nodes.retain(|id, info| {
        let is_implicit = info.label.is_none() && info.attrs.is_empty();
        !(is_implicit && subgraph_nodes.contains_key(id.as_str()))
    });
}

/// Build a DepGraph from an AST subgraph.
fn collect_subgraph(sub: &AstSubgraph) -> DepGraph {
    let mut dep = DepGraph {
        id: sub.id.as_ref().map(|s| unquote(s)),
        ..Default::default()
    };
    walk_stmts(&sub.stmts.stmts, &mut dep);
    dep.nodes.sort_keys();
    dep
}

/// Add a node from a NodeStmt into the DepGraph, returning the unquoted node ID.
fn add_node(node_stmt: &ast::NodeStmt<(ast::ID, ast::ID)>, dep: &mut DepGraph) -> String {
    let id = unquote(&node_stmt.node.id);
    let mut info = NodeInfo::default();

    if let Some(attr_list) = &node_stmt.attr {
        for alist in &attr_list.elems {
            for (k, v) in &alist.elems {
                let key = unquote(&id_to_string(k));
                let value = unquote(&id_to_string(v));
                if key == "label" {
                    info.label = Some(value);
                } else {
                    info.attrs.insert(key, value);
                }
            }
        }
    }

    dep.nodes.insert(id.clone(), info);
    id
}

/// Flatten an EdgeStmt into individual edges and add them to the DepGraph.
/// Handles chained edges (a -> b -> c) and subgraph endpoints ({ a b } -> c).
fn add_edges(edge_stmt: &AstEdgeStmt, dep: &mut DepGraph) {
    // Extract edge attributes (shared across all flattened edges).
    let mut edge_label = None;
    let mut edge_attrs = indexmap::IndexMap::new();
    if let Some(attr_list) = &edge_stmt.attr {
        for alist in &attr_list.elems {
            for (k, v) in &alist.elems {
                let key = unquote(&id_to_string(k));
                let value = unquote(&id_to_string(v));
                if key == "label" {
                    edge_label = Some(value);
                } else {
                    edge_attrs.insert(key, value);
                }
            }
        }
    }

    // Collect all endpoints in the chain: from -> to1 -> to2 -> ...
    let mut endpoints: Vec<Either<&ast::NodeID, &AstSubgraph>> = Vec::new();
    endpoints.push(edge_stmt.from.as_ref());
    let mut rhs = &edge_stmt.next;
    loop {
        endpoints.push(rhs.to.as_ref());
        match &rhs.next {
            Some(next) => rhs = next,
            None => break,
        }
    }

    // For each consecutive pair, create edges between all node IDs.
    for pair in endpoints.windows(2) {
        let from_ids = endpoint_node_ids(&pair[0], dep);
        let to_ids = endpoint_node_ids(&pair[1], dep);
        for from_id in &from_ids {
            for to_id in &to_ids {
                // Ensure implicit nodes exist.
                dep.nodes.entry(from_id.clone()).or_default();
                dep.nodes.entry(to_id.clone()).or_default();
                dep.edges.push(Edge {
                    from: from_id.clone(),
                    to: to_id.clone(),
                    label: edge_label.clone(),
                    attrs: edge_attrs.clone(),
                });
            }
        }
    }
}

/// Extract node IDs from an edge endpoint, which may be a single node or an
/// anonymous subgraph containing multiple nodes.
fn endpoint_node_ids(
    endpoint: &Either<&ast::NodeID, &AstSubgraph>,
    dep: &mut DepGraph,
) -> Vec<String> {
    match endpoint {
        Either::Left(node_id) => vec![unquote(&node_id.id)],
        Either::Right(sub) => {
            // Anonymous subgraph as edge endpoint: collect all node IDs.
            let mut ids = Vec::new();
            collect_endpoint_ids(&sub.stmts.stmts, &mut ids, dep);
            ids
        }
    }
}

/// Recursively collect node IDs from statements inside an anonymous subgraph
/// used as an edge endpoint.
fn collect_endpoint_ids(stmts: &[AstStmt], ids: &mut Vec<String>, dep: &mut DepGraph) {
    for stmt in stmts {
        match stmt {
            AstStmt::NodeStmt(node_stmt) => {
                ids.push(add_node(node_stmt, dep));
            }
            AstStmt::EdgeStmt(edge_stmt) => {
                // Edges inside anonymous subgraph endpoints still define nodes.
                add_edges(edge_stmt, dep);
                // Collect the from-endpoint node IDs.
                let mut inner_ids = endpoint_node_ids(&edge_stmt.from.as_ref(), dep);
                ids.append(&mut inner_ids);
            }
            AstStmt::Subgraph(sub) => {
                collect_endpoint_ids(&sub.stmts.stmts, ids, dep);
            }
            _ => {}
        }
    }
}

/// Extract key-value pairs from an AttrList into the attrs map.
fn extract_graph_attrs(attr_list: &AstAttrList, attrs: &mut indexmap::IndexMap<String, String>) {
    for alist in &attr_list.elems {
        for (k, v) in &alist.elems {
            attrs.insert(unquote(&id_to_string(k)), unquote(&id_to_string(v)));
        }
    }
}

#[cfg(test)]
mod tests {
    use dot_parser::canonical;

    use super::*;

    #[test]
    fn dot_parser_behavior() {
        let ast = ast::Graph::try_from(r#"digraph { a [label="My Label"]; "quoted node" -> a; }"#)
            .unwrap();
        let graph = canonical::Graph::from(ast);

        // Attribute value: parser strips outer quotes, gives bare text.
        let (_, node_a) = graph.nodes.set.iter().find(|(id, _)| *id == "a").unwrap();
        let (_, val) = &node_a.attr.elems[0];
        let val_str: String = val.clone().into();
        assert_eq!(val_str, "My Label");

        // Node ID: parser preserves outer quotes on quoted identifiers.
        assert!(graph.nodes.set.contains_key("\"quoted node\""));
        assert!(!graph.nodes.set.contains_key("quoted node"));

        // Edge endpoint: quotes also preserved.
        assert_eq!(graph.edges.set[0].from, "\"quoted node\"");
    }

    #[test]
    fn dot_parser_preserves_escape_sequences() {
        let ast =
            ast::Graph::try_from(r#"digraph { a [label="say \"hi\"", tooltip="path\\here"]; }"#)
                .unwrap();
        let graph = canonical::Graph::from(ast);

        let (_, node) = graph.nodes.set.iter().find(|(id, _)| *id == "a").unwrap();
        let vals: Vec<(String, String)> = node
            .attr
            .elems
            .iter()
            .map(|(k, v)| {
                let k: String = k.clone().into();
                let v: String = v.clone().into();
                (k, v)
            })
            .collect();

        // Escaped quotes: parser gives us the raw escape sequence, NOT unescaped.
        let label_val = &vals.iter().find(|(k, _)| k == "label").unwrap().1;
        assert_eq!(
            label_val, r#"say \"hi\""#,
            "dot-parser preserves \\\" as-is (does not unescape)"
        );

        // Escaped backslash: parser gives us the raw escape sequence.
        let tooltip_val = &vals.iter().find(|(k, _)| k == "tooltip").unwrap().1;
        assert_eq!(
            tooltip_val, r"path\\here",
            "dot-parser preserves \\\\ as-is (does not unescape)"
        );
    }

    #[test]
    fn unquote_strips_outer_quotes() {
        assert_eq!(unquote(r#""hello""#), "hello");
    }

    #[test]
    fn unquote_bare_id_unchanged() {
        assert_eq!(unquote("hello"), "hello");
    }

    #[test]
    fn unquote_escaped_quote_with_outer_quotes() {
        // Node ID case: outer quotes present + escape sequences.
        assert_eq!(unquote(r#""say \"hi\"""#), r#"say "hi""#);
    }

    #[test]
    fn unquote_escaped_quote_without_outer_quotes() {
        // Attribute value case: dot-parser already stripped outer quotes,
        // but escape sequences remain. Must still unescape.
        assert_eq!(unquote(r#"say \"hi\""#), r#"say "hi""#);
    }

    #[test]
    fn unquote_backslash_preserved() {
        // Backslashes are preserved verbatim (DOT formatting directives).
        assert_eq!(unquote(r#""a\\b""#), r"a\\b");
        assert_eq!(unquote(r"a\\b"), r"a\\b");
    }

    #[test]
    fn unquote_formatting_directives_preserved() {
        // DOT \n, \l, \r are label formatting directives and must survive.
        assert_eq!(unquote(r#""line1\nline2""#), r"line1\nline2");
        assert_eq!(unquote(r"line1\nline2"), r"line1\nline2");
        assert_eq!(unquote(r"line1\lline2"), r"line1\lline2");
        assert_eq!(unquote(r"line1\rline2"), r"line1\rline2");
    }

    #[test]
    fn unquote_escaped_backslash_before_quote() {
        // DOT \\\" = escaped backslash + escaped quote.
        // We preserve \\ but decode \" -> ", so \\\" -> \\"
        assert_eq!(unquote(r#""a\\\"b""#), r#"a\\"b"#);
        assert_eq!(unquote(r#"a\\\"b"#), r#"a\\"b"#);
    }

    #[test]
    fn empty_digraph() {
        let graph = parse("digraph {}").unwrap();
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn named_digraph() {
        let graph = parse("digraph deps {}").unwrap();
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn simple_edge() {
        let graph = parse("digraph { a -> b; }").unwrap();
        assert_eq!(graph.nodes.len(), 2);
        assert!(graph.nodes.contains_key("a"));
        assert!(graph.nodes.contains_key("b"));
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].from, "a");
        assert_eq!(graph.edges[0].to, "b");
        assert_eq!(graph.edges[0].label, None);
    }

    #[test]
    fn node_labels() {
        let graph = parse(r#"digraph { a [label="Alpha"]; b [label="Bravo"]; a -> b; }"#).unwrap();
        assert_eq!(graph.nodes["a"].label.as_deref(), Some("Alpha"));
        assert_eq!(graph.nodes["b"].label.as_deref(), Some("Bravo"));
    }

    #[test]
    fn edge_labels() {
        let graph =
            parse(r#"digraph { a -> b [label="depends"]; a -> c [label="uses"]; }"#).unwrap();
        assert_eq!(graph.edges.len(), 2);
        assert_eq!(graph.edges[0].label.as_deref(), Some("depends"));
        assert_eq!(graph.edges[1].label.as_deref(), Some("uses"));
    }

    #[test]
    fn type_attr_in_attrs() {
        let graph = parse(r#"digraph { a [type="lib"]; }"#).unwrap();
        assert_eq!(
            graph.nodes["a"].attrs.get("type").map(|s| s.as_str()),
            Some("lib")
        );
    }

    #[test]
    fn shape_attr_in_attrs() {
        let graph = parse(r#"digraph { a [shape=box]; }"#).unwrap();
        assert_eq!(
            graph.nodes["a"].attrs.get("shape").map(|s| s.as_str()),
            Some("box")
        );
    }

    #[test]
    fn type_and_shape_coexist_in_attrs() {
        let graph = parse(r#"digraph { a [shape=box, type="lib"]; }"#).unwrap();
        assert_eq!(
            graph.nodes["a"].attrs.get("type").map(|s| s.as_str()),
            Some("lib")
        );
        assert_eq!(
            graph.nodes["a"].attrs.get("shape").map(|s| s.as_str()),
            Some("box")
        );
    }

    #[test]
    fn extra_attrs_preserved() {
        let graph = parse(r#"digraph { a [label="A", color="red", style="bold"]; }"#).unwrap();
        assert_eq!(graph.nodes["a"].label.as_deref(), Some("A"));
        assert_eq!(
            graph.nodes["a"].attrs.get("color").map(|s| s.as_str()),
            Some("red")
        );
        assert_eq!(
            graph.nodes["a"].attrs.get("style").map(|s| s.as_str()),
            Some("bold")
        );
    }

    #[test]
    fn implicit_nodes_from_edges() {
        // Nodes only defined implicitly by edges should still appear in the graph.
        let graph = parse("digraph { a -> b -> c; }").unwrap();
        assert_eq!(graph.nodes.len(), 3);
        assert!(graph.nodes.contains_key("a"));
        assert!(graph.nodes.contains_key("b"));
        assert!(graph.nodes.contains_key("c"));
    }

    #[test]
    fn graph_attrs_captured() {
        let graph = parse(r#"digraph { rankdir=LR; a -> b; }"#).unwrap();
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.attrs.get("rankdir").map(|s| s.as_str()), Some("LR"));
    }

    #[test]
    fn graph_name_captured() {
        let graph = parse("digraph deps { a -> b; }").unwrap();
        assert_eq!(graph.id.as_deref(), Some("deps"));
    }

    #[test]
    fn quoted_ids() {
        let graph =
            parse(r#"digraph { "my node" [label="My Node"]; "my node" -> "other"; }"#).unwrap();
        assert!(graph.nodes.contains_key("my node"));
        assert_eq!(graph.nodes["my node"].label.as_deref(), Some("My Node"));
    }

    #[test]
    fn edge_attrs_captured() {
        let graph = parse(r#"digraph { a -> b [style="dashed", color="red"]; }"#).unwrap();
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(
            graph.edges[0].attrs.get("style").map(|s| s.as_str()),
            Some("dashed")
        );
        assert_eq!(
            graph.edges[0].attrs.get("color").map(|s| s.as_str()),
            Some("red")
        );
    }

    #[test]
    fn edge_label_and_attrs() {
        let graph = parse(r#"digraph { a -> b [label="uses", style="bold"]; }"#).unwrap();
        assert_eq!(graph.edges[0].label.as_deref(), Some("uses"));
        assert_eq!(
            graph.edges[0].attrs.get("style").map(|s| s.as_str()),
            Some("bold")
        );
    }

    #[test]
    fn escaped_quotes_in_label() {
        // This exercises the bug path: dot-parser strips outer quotes from
        // attribute values but preserves \" escape sequences. Our unquote
        // must decode them so they don't get double-escaped by quote().
        let graph = parse(r#"digraph { a [label="say \"hi\""]; }"#).unwrap();
        assert_eq!(graph.nodes["a"].label.as_deref(), Some(r#"say "hi""#));
    }

    #[test]
    fn escaped_quotes_in_label_roundtrip() {
        // Full parse->emit round-trip with escaped quotes.
        let input = r#"digraph { a [label="say \"hi\""]; }"#;
        let graph = parse(input).unwrap();
        let mut buf = Vec::new();
        crate::emit::dot::emit(&graph, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "digraph {\n    a [label=\"say \\\"hi\\\"\"];\n}\n");
        // And parse the output again to verify it's valid.
        let graph2 = parse(&output).unwrap();
        assert_eq!(graph2.nodes["a"].label.as_deref(), Some(r#"say "hi""#));
    }

    #[test]
    fn fixture_small_dot() {
        let input = include_str!("../../../../data/depconv/small.dot");
        let graph = parse(input).unwrap();
        assert_eq!(graph.nodes.len(), 3);
        assert!(graph.nodes.contains_key("myapp"));
        assert!(graph.nodes.contains_key("libfoo"));
        assert!(graph.nodes.contains_key("libbar"));
        assert_eq!(
            graph.nodes["myapp"].label.as_deref(),
            Some("My Application")
        );
        // shape=box stored in attrs
        assert_eq!(
            graph.nodes["myapp"].attrs.get("shape").map(|s| s.as_str()),
            Some("box")
        );
        assert_eq!(graph.edges.len(), 3);
        // Graph name and rankdir captured
        assert_eq!(graph.id.as_deref(), Some("deps"));
        assert_eq!(graph.attrs.get("rankdir").map(|s| s.as_str()), Some("LR"));
    }

    #[test]
    fn subgraph_basic() {
        let graph = parse(
            r#"digraph {
                top;
                subgraph cluster0 {
                    label = "Group A";
                    a;
                    b;
                }
            }"#,
        )
        .unwrap();
        // Top-level has only the standalone node.
        assert_eq!(graph.nodes.len(), 1);
        assert!(graph.nodes.contains_key("top"));
        // One subgraph.
        assert_eq!(graph.subgraphs.len(), 1);
        assert_eq!(graph.subgraphs[0].id.as_deref(), Some("cluster0"));
        assert_eq!(
            graph.subgraphs[0].attrs.get("label").map(|s| s.as_str()),
            Some("Group A")
        );
        assert_eq!(graph.subgraphs[0].nodes.len(), 2);
        assert!(graph.subgraphs[0].nodes.contains_key("a"));
        assert!(graph.subgraphs[0].nodes.contains_key("b"));
    }

    #[test]
    fn subgraph_nested() {
        let graph = parse(
            r#"digraph {
                subgraph outer {
                    x;
                    subgraph inner {
                        y;
                    }
                }
            }"#,
        )
        .unwrap();
        assert_eq!(graph.subgraphs.len(), 1);
        let outer = &graph.subgraphs[0];
        assert_eq!(outer.id.as_deref(), Some("outer"));
        assert_eq!(outer.nodes.len(), 1);
        assert!(outer.nodes.contains_key("x"));
        assert_eq!(outer.subgraphs.len(), 1);
        let inner = &outer.subgraphs[0];
        assert_eq!(inner.id.as_deref(), Some("inner"));
        assert_eq!(inner.nodes.len(), 1);
        assert!(inner.nodes.contains_key("y"));
    }

    #[test]
    fn subgraph_edges_stay_local() {
        let graph = parse(
            r#"digraph {
                a -> b;
                subgraph cluster0 {
                    c -> d;
                }
            }"#,
        )
        .unwrap();
        // Parent-level edges only.
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].from, "a");
        assert_eq!(graph.edges[0].to, "b");
        // Subgraph edges only.
        assert_eq!(graph.subgraphs[0].edges.len(), 1);
        assert_eq!(graph.subgraphs[0].edges[0].from, "c");
        assert_eq!(graph.subgraphs[0].edges[0].to, "d");
    }

    #[test]
    fn cross_subgraph_edge_no_duplicate_node() {
        // Edge at top level references node declared in subgraph.
        // The implicit default at top level should be removed.
        let graph = parse(
            r#"digraph {
                a -> b;
                subgraph cluster0 {
                    b [label="B"];
                }
            }"#,
        )
        .unwrap();
        // `a` stays at top level (only defined here).
        // `b` should NOT be at top level -- it's in the subgraph.
        assert_eq!(graph.nodes.len(), 1);
        assert!(graph.nodes.contains_key("a"));
        assert!(!graph.nodes.contains_key("b"));
        // `b` lives in the subgraph with its label.
        assert_eq!(graph.subgraphs[0].nodes.len(), 1);
        assert_eq!(graph.subgraphs[0].nodes["b"].label.as_deref(), Some("B"));
        // Flattened view still has both nodes.
        let all = graph.all_nodes();
        assert_eq!(all.len(), 2);
        assert!(all.contains_key("a"));
        assert!(all.contains_key("b"));
    }

    #[test]
    fn cross_subgraph_edge_forward_reference() {
        // Edge appears before the subgraph that declares the node.
        let graph = parse(
            r#"digraph {
                subgraph cluster0 {
                    a [label="A"];
                }
                a -> b;
            }"#,
        )
        .unwrap();
        assert_eq!(graph.nodes.len(), 1);
        assert!(graph.nodes.contains_key("b"));
        assert!(!graph.nodes.contains_key("a"));
        assert_eq!(graph.subgraphs[0].nodes["a"].label.as_deref(), Some("A"));
    }

    #[test]
    fn cross_subgraph_edge_nested_dedup() {
        // Node declared in deeply nested subgraph, edges at multiple levels.
        let graph = parse(
            r#"digraph {
                a -> b;
                subgraph outer {
                    b -> c;
                    subgraph inner {
                        b [label="B"];
                        c [label="C"];
                    }
                }
            }"#,
        )
        .unwrap();
        // Top level: only `a` (b was deduped).
        assert_eq!(graph.nodes.len(), 1);
        assert!(graph.nodes.contains_key("a"));
        // Outer: b and c were deduped (implicit defaults, declared in inner).
        assert_eq!(graph.subgraphs[0].nodes.len(), 0);
        // Inner: b and c with labels.
        let inner = &graph.subgraphs[0].subgraphs[0];
        assert_eq!(inner.nodes.len(), 2);
        assert_eq!(inner.nodes["b"].label.as_deref(), Some("B"));
        assert_eq!(inner.nodes["c"].label.as_deref(), Some("C"));
        // Flattened view has all three.
        let all = graph.all_nodes();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn explicit_top_level_node_not_deduped() {
        // Node explicitly declared with attrs at top level AND in subgraph.
        // Both should be kept (no data loss).
        let graph = parse(
            r#"digraph {
                a [color="red"];
                subgraph cluster0 {
                    a [label="A"];
                }
            }"#,
        )
        .unwrap();
        assert_eq!(graph.nodes.len(), 1);
        assert!(graph.nodes.contains_key("a"));
        assert_eq!(
            graph.nodes["a"].attrs.get("color").map(|s| s.as_str()),
            Some("red")
        );
        assert_eq!(graph.subgraphs[0].nodes.len(), 1);
        assert_eq!(graph.subgraphs[0].nodes["a"].label.as_deref(), Some("A"));
    }

    #[test]
    fn fixture_cmake_geos_subgraph() {
        let input = include_str!("../../../../data/depconv/cmake.geos.dot");
        let graph = parse(input).unwrap();

        assert_eq!(graph.id.as_deref(), Some("GEOS"));

        // One subgraph: clusterLegend.
        assert_eq!(graph.subgraphs.len(), 1);
        let legend = &graph.subgraphs[0];
        assert_eq!(legend.id.as_deref(), Some("clusterLegend"));
        assert_eq!(
            legend.attrs.get("label").map(|s| s.as_str()),
            Some("Legend")
        );
        assert_eq!(legend.attrs.get("color").map(|s| s.as_str()), Some("black"));

        // Legend subgraph: 8 nodes (legendNode0-7), 7 edges.
        assert_eq!(legend.nodes.len(), 8);
        assert!(legend.nodes.contains_key("legendNode0"));
        assert!(legend.nodes.contains_key("legendNode7"));
        assert_eq!(legend.edges.len(), 7);

        // Parent: 11 nodes (node0-node10), 13 edges.
        assert_eq!(graph.nodes.len(), 11);
        assert!(graph.nodes.contains_key("node0"));
        assert!(graph.nodes.contains_key("node8"));
        assert!(graph.nodes.contains_key("node10"));
        // node8's label is "Threads::Threads".
        assert_eq!(
            graph.nodes["node8"].label.as_deref(),
            Some("Threads::Threads")
        );
        assert_eq!(graph.edges.len(), 13);

        // No legend attributes leaked into parent.
        assert_eq!(graph.attrs.get("label"), None);
        assert_eq!(graph.attrs.get("color"), None);
    }

    #[test]
    fn all_nodes_flattens() {
        let graph = parse(
            r#"digraph {
                a;
                subgraph s1 {
                    b;
                    subgraph s2 {
                        c;
                    }
                }
            }"#,
        )
        .unwrap();
        let all = graph.all_nodes();
        assert_eq!(all.len(), 3);
        assert!(all.contains_key("a"));
        assert!(all.contains_key("b"));
        assert!(all.contains_key("c"));
    }

    #[test]
    fn all_edges_flattens() {
        let graph = parse(
            r#"digraph {
                a -> b;
                subgraph s1 {
                    c -> d;
                }
            }"#,
        )
        .unwrap();
        let all = graph.all_edges();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].from, "a");
        assert_eq!(all[0].to, "b");
        assert_eq!(all[1].from, "c");
        assert_eq!(all[1].to, "d");
    }

    #[test]
    fn adjacency_list_across_subgraphs() {
        let graph = parse(
            r#"digraph {
                a -> b;
                subgraph s1 {
                    b -> c;
                    c -> d;
                }
            }"#,
        )
        .unwrap();
        let adj = graph.adjacency_list();
        assert_eq!(adj.get("a").map(|v| v.as_slice()), Some(["b"].as_slice()));
        assert_eq!(adj.get("b").map(|v| v.as_slice()), Some(["c"].as_slice()));
        assert_eq!(adj.get("c").map(|v| v.as_slice()), Some(["d"].as_slice()));
    }
}
