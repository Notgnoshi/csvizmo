use csvizmo_test::{CommandExt, tool};
use pretty_assertions::assert_eq;

// Test graph: myapp -> libfoo -> libbar
//             myapp -> libbar
const SIMPLE_GRAPH: &str = "1\tlibfoo\n2\tlibbar\n3\tmyapp\n#\n3\t1\n3\t2\n1\t2\n";

#[test]
fn select_single_pattern() {
    let output = tool!("depfilter")
        .args([
            "select",
            "--include",
            "lib*",
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
        ])
        .write_stdin(SIMPLE_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "1\tlibfoo\n2\tlibbar\n#\n1\t2\n");
}

#[test]
fn select_by_id() {
    let output = tool!("depfilter")
        .args([
            "select",
            "--include",
            "1",
            "--key",
            "id",
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
        ])
        .write_stdin(SIMPLE_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "1\tlibfoo\n#\n");
}

#[test]
fn select_with_deps() {
    // Select myapp and include all its dependencies
    let output = tool!("depfilter")
        .args([
            "select",
            "--include",
            "myapp",
            "--deps",
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
        ])
        .write_stdin(SIMPLE_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should include myapp, libfoo, and libbar with all edges
    assert_eq!(stdout, SIMPLE_GRAPH);
}

#[test]
fn select_with_rdeps() {
    // Select libbar and include all nodes that depend on it
    let output = tool!("depfilter")
        .args([
            "select",
            "--include",
            "libbar",
            "--rdeps",
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
        ])
        .write_stdin(SIMPLE_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should include libbar, libfoo (depends on libbar), and myapp (depends on libbar)
    assert_eq!(stdout, SIMPLE_GRAPH);
}

#[test]
fn select_with_depth() {
    // Create a deeper graph for depth testing
    // a -> b -> c -> d
    let deep_graph = "a\nb\nc\nd\n#\na\tb\nb\tc\nc\td\n";
    let output = tool!("depfilter")
        .args([
            "select",
            "--include",
            "a",
            "--deps",
            "--depth",
            "1",
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
        ])
        .write_stdin(deep_graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should include only a and b (1 level deep)
    assert_eq!(stdout, "a\nb\n#\na\tb\n");
}

#[test]
fn select_depth_from_roots() {
    // a -> b -> c -> d: no pattern, depth 1 seeds from roots (a), keeps a and b
    let deep_graph = "a\nb\nc\nd\n#\na\tb\nb\tc\nc\td\n";
    let output = tool!("depfilter")
        .args([
            "select",
            "--depth",
            "1",
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
        ])
        .write_stdin(deep_graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "a\nb\n#\na\tb\n");
}

#[test]
fn select_multiple_patterns_and() {
    // Graph with nodes that match multiple criteria
    let graph = "libfoo-alpha\nlibfoo-beta\nlibbar-alpha\n#\n";
    let output = tool!("depfilter")
        .args([
            "select",
            "--include",
            "libfoo*",
            "--include",
            "*alpha",
            "--and",
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
        ])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should include only libfoo-alpha (matches both patterns)
    assert_eq!(stdout, "libfoo-alpha\n#\n");
}

#[test]
fn select_with_deps_and_rdeps() {
    // a -> b -> c -> d: select b with both directions gets everything
    let graph = "a\nb\nc\nd\n#\na\tb\nb\tc\nc\td\n";
    let output = tool!("depfilter")
        .args([
            "select",
            "--include",
            "b",
            "--deps",
            "--rdeps",
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
        ])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "a\nb\nc\nd\n#\na\tb\nb\tc\nc\td\n");
}

// -- exclude integration tests --

#[test]
fn exclude_single_pattern() {
    let output = tool!("depfilter")
        .args([
            "select",
            "--exclude",
            "libfoo",
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
        ])
        .write_stdin(SIMPLE_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "2\tlibbar\n3\tmyapp\n#\n3\t2\n");
}

#[test]
fn exclude_by_id() {
    let output = tool!("depfilter")
        .args([
            "select",
            "--exclude",
            "1",
            "--key",
            "id",
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
        ])
        .write_stdin(SIMPLE_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "2\tlibbar\n3\tmyapp\n#\n3\t2\n");
}

#[test]
fn exclude_with_preserve_connectivity() {
    let chain_graph = "a\nb\nc\n#\na\tb\nb\tc\n";
    let output = tool!("depfilter")
        .args([
            "select",
            "--exclude",
            "b",
            "--preserve-connectivity",
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
        ])
        .write_stdin(chain_graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "a\nc\n#\na\tc\n");
}

#[test]
fn exclude_multiple_patterns() {
    let graph = "a\nb\nc\nd\n#\na\tb\nb\tc\nc\td\n";
    let output = tool!("depfilter")
        .args([
            "select",
            "--exclude",
            "b",
            "--exclude",
            "c",
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
        ])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "a\nd\n#\n");
}

#[test]
fn select_dot_output() {
    let output = tool!("depfilter")
        .args([
            "select",
            "--include",
            "lib*",
            "--input-format",
            "tgf",
            "--output-format",
            "dot",
        ])
        .write_stdin(SIMPLE_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    \"1\" [label=\"libfoo\"];
    \"2\" [label=\"libbar\"];
    \"1\" -> \"2\";
}
"
    );
}

#[cfg(feature = "dot")]
#[test]
fn exclude_preserve_connectivity_subgraph() {
    // subgraph { a -> b -> c }: exclude b, bypass a -> c stays in subgraph
    let dot_input = "\
digraph {
    subgraph cluster_0 {
        a;
        b;
        c;
        a -> b;
        b -> c;
    }
}
";
    let output = tool!("depfilter")
        .args([
            "select",
            "--exclude",
            "b",
            "--preserve-connectivity",
            "--input-format",
            "dot",
            "--output-format",
            "dot",
        ])
        .write_stdin(dot_input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    subgraph cluster_0 {
        a;
        c;
        a -> c;
    }
}
"
    );
}

#[cfg(feature = "dot")]
#[test]
fn exclude_dot_input() {
    let dot_input = "\
digraph {
    \"1\" [label=\"libfoo\"];
    \"2\" [label=\"libbar\"];
    \"3\" [label=\"myapp\"];
    \"3\" -> \"1\";
    \"3\" -> \"2\";
    \"1\" -> \"2\";
}
";
    let output = tool!("depfilter")
        .args([
            "select",
            "--exclude",
            "libfoo",
            "--input-format",
            "dot",
            "--output-format",
            "tgf",
        ])
        .write_stdin(dot_input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "2\tlibbar\n3\tmyapp\n#\n3\t2\n");
}

// -- include + exclude combined integration test --

#[test]
fn include_with_exclude() {
    // a -> b -> c -> d: include a with deps, exclude c
    let graph = "a\nb\nc\nd\n#\na\tb\nb\tc\nc\td\n";
    let output = tool!("depfilter")
        .args([
            "select",
            "-g",
            "a",
            "--deps",
            "-x",
            "c",
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
        ])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "a\nb\nd\n#\na\tb\n");
}

// -- between integration tests --

#[test]
fn between_two_nodes() {
    // a -> b -> c: between a and c includes intermediate b
    let graph = "a\nb\nc\n#\na\tb\nb\tc\n";
    let output = tool!("depfilter")
        .args([
            "between",
            "-g",
            "a",
            "-g",
            "c",
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
        ])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "a\nb\nc\n#\na\tb\nb\tc\n");
}

#[test]
fn between_by_id() {
    let output = tool!("depfilter")
        .args([
            "between",
            "-g",
            "1",
            "-g",
            "2",
            "--key",
            "id",
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
        ])
        .write_stdin(SIMPLE_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "1\tlibfoo\n2\tlibbar\n#\n1\t2\n");
}

#[test]
fn between_glob_multiple_nodes() {
    // a -> b -> c: glob "?" matches all three, paths exist between all pairs
    let graph = "a\nb\nc\n#\na\tb\nb\tc\n";
    let output = tool!("depfilter")
        .args([
            "between",
            "-g",
            "?",
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
        ])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "a\nb\nc\n#\na\tb\nb\tc\n");
}

#[test]
fn between_no_matching_patterns() {
    let graph = "a\nb\n#\na\tb\n";
    let output = tool!("depfilter")
        .args([
            "between",
            "-g",
            "nonexistent",
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
        ])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "#\n");
}

#[test]
fn between_no_path() {
    // a -> b, c -> d: no path between a and c
    let graph = "a\nb\nc\nd\n#\na\tb\nc\td\n";
    let output = tool!("depfilter")
        .args([
            "between",
            "-g",
            "a",
            "-g",
            "c",
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
        ])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "#\n");
}

#[test]
fn between_cargo_metadata_fixture() {
    // Use the real cargo-metadata.json fixture to test between on a non-trivial graph.
    // csvizmo-depgraph depends on clap both directly and via csvizmo-utils,
    // so csvizmo-utils is an intermediate node on a path to clap.
    // clap in turn depends on clap_builder, clap_derive, and clap_builder -> clap_lex.
    let input = include_str!("../../../data/depconv/cargo-metadata.json");
    let output = tool!("depfilter")
        .args([
            "between",
            "-g",
            "csvizmo-depgraph",
            "-g",
            "clap*",
            "--input-format",
            "cargo-metadata",
            "--output-format",
            "tgf",
        ])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
clap_4.5.57\tclap
clap_builder_4.5.57\tclap_builder
clap_derive_4.5.55\tclap_derive
clap_lex_0.7.7\tclap_lex
csvizmo-depgraph_0.5.0\tcsvizmo-depgraph
csvizmo-utils_0.5.0\tcsvizmo-utils
#
clap_4.5.57\tclap_builder_4.5.57
clap_4.5.57\tclap_derive_4.5.55
clap_builder_4.5.57\tclap_lex_0.7.7
csvizmo-depgraph_0.5.0\tclap_4.5.57
csvizmo-depgraph_0.5.0\tcsvizmo-utils_0.5.0
csvizmo-utils_0.5.0\tclap_4.5.57
"
    );
}

// -- cycles integration tests --

#[test]
fn cycles_dag_no_cycles() {
    // a -> b -> c: no cycles, empty output
    let graph = "a\nb\nc\n#\na\tb\nb\tc\n";
    let output = tool!("depfilter")
        .args(["cycles", "--input-format", "tgf", "--output-format", "tgf"])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "#\n");
}

#[test]
fn cycles_simple_three_node() {
    // a -> b -> c -> a: single cycle with all three nodes
    let graph = "a\nb\nc\n#\na\tb\nb\tc\nc\ta\n";
    let output = tool!("depfilter")
        .args(["cycles", "--input-format", "tgf", "--output-format", "dot"])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    subgraph cluster_cycle_0 {
        label=\"cycle_0\";
        a;
        b;
        c;
        a -> b;
        b -> c;
        c -> a;
    }
}
"
    );
}

#[test]
fn cycles_multiple_disjoint() {
    // cycle1: a <-> b, cycle2: c <-> d (no edges between them)
    let graph = "a\nb\nc\nd\n#\na\tb\nb\ta\nc\td\nd\tc\n";
    let output = tool!("depfilter")
        .args(["cycles", "--input-format", "tgf", "--output-format", "dot"])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Both cycles appear as separate subgraphs, no cross-cycle edges.
    assert_eq!(
        stdout,
        "\
digraph {
    subgraph cluster_cycle_0 {
        label=\"cycle_0\";
        a;
        b;
        a -> b;
        b -> a;
    }
    subgraph cluster_cycle_1 {
        label=\"cycle_1\";
        c;
        d;
        c -> d;
        d -> c;
    }
}
"
    );
}

#[test]
fn cycles_mixed_graph_excludes_acyclic() {
    // x -> a <-> b -> y: only a and b form a cycle; x and y excluded
    let graph = "x\na\nb\ny\n#\nx\ta\na\tb\nb\ta\nb\ty\n";
    let output = tool!("depfilter")
        .args(["cycles", "--input-format", "tgf", "--output-format", "dot"])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    subgraph cluster_cycle_0 {
        label=\"cycle_0\";
        a;
        b;
        a -> b;
        b -> a;
    }
}
"
    );
}

#[test]
fn cycles_cross_cycle_edges_at_top_level() {
    // cycle1: a <-> b, cycle2: c <-> d, cross-edge: b -> c
    let graph = "a\nb\nc\nd\n#\na\tb\nb\ta\nc\td\nd\tc\nb\tc\n";
    let output = tool!("depfilter")
        .args(["cycles", "--input-format", "tgf", "--output-format", "dot"])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // tarjan_scc returns SCCs in reverse topological order: c,d before a,b
    assert_eq!(
        stdout,
        "\
digraph {
    subgraph cluster_cycle_0 {
        label=\"cycle_0\";
        c;
        d;
        c -> d;
        d -> c;
    }
    subgraph cluster_cycle_1 {
        label=\"cycle_1\";
        a;
        b;
        a -> b;
        b -> a;
    }
    b -> c;
}
"
    );
}

#[test]
fn cycles_self_loop_ignored() {
    // a -> a: self-loop is not a cycle (SCC size 1)
    let graph = "a\n#\na\ta\n";
    let output = tool!("depfilter")
        .args(["cycles", "--input-format", "tgf", "--output-format", "tgf"])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "#\n");
}

// -- slice integration tests --

#[cfg(feature = "dot")]
#[test]
fn slice_removes_cross_subgraph_edges() {
    let dot_input = "\
digraph {
    subgraph cluster_0 {
        a;
    }
    subgraph cluster_1 {
        b;
    }
    a -> b;
}
";
    let output = tool!("depfilter")
        .args(["slice", "--input-format", "dot", "--output-format", "dot"])
        .write_stdin(dot_input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    subgraph cluster_0 {
        a;
    }
    subgraph cluster_1 {
        b;
    }
}
"
    );
}

#[cfg(feature = "dot")]
#[test]
fn slice_preserves_root_nodes_by_default() {
    let dot_input = "\
digraph {
    subgraph cluster_0 {
        a;
    }
    orphan;
    orphan -> a;
}
";
    let output = tool!("depfilter")
        .args(["slice", "--input-format", "dot", "--output-format", "dot"])
        .write_stdin(dot_input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    subgraph cluster_0 {
        a;
    }
    orphan;
}
"
    );
}

#[cfg(feature = "dot")]
#[test]
fn slice_drop_orphans() {
    let dot_input = "\
digraph {
    subgraph cluster_0 {
        a;
    }
    orphan;
    orphan -> a;
}
";
    let output = tool!("depfilter")
        .args([
            "slice",
            "--drop-orphans",
            "--input-format",
            "dot",
            "--output-format",
            "dot",
        ])
        .write_stdin(dot_input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    subgraph cluster_0 {
        a;
    }
}
"
    );
}

#[test]
fn slice_no_subgraphs() {
    let graph = "a\nb\n#\na\tb\n";
    let output = tool!("depfilter")
        .args(["slice", "--input-format", "tgf", "--output-format", "tgf"])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "a\nb\n#\na\tb\n");
}

#[test]
fn slice_no_subgraphs_drop_orphans() {
    let graph = "a\nb\n#\na\tb\n";
    let output = tool!("depfilter")
        .args([
            "slice",
            "--drop-orphans",
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
        ])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "#\n");
}

#[cfg(feature = "dot")]
#[test]
fn slice_recursive() {
    // Outer subgraph has node a and a nested inner subgraph with node b.
    // Edge a -> b crosses the inner boundary; recursive mode should cut it.
    let dot_input = "\
digraph {
    subgraph cluster_outer {
        subgraph cluster_inner {
            b;
        }
        a;
        a -> b;
    }
}
";
    let output = tool!("depfilter")
        .args([
            "slice",
            "--recursive",
            "--input-format",
            "dot",
            "--output-format",
            "dot",
        ])
        .write_stdin(dot_input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    subgraph cluster_outer {
        subgraph cluster_inner {
            b;
        }
        a;
    }
}
"
    );
}
