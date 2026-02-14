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
            "--pattern",
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
            "--pattern",
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
fn select_by_label() {
    let output = tool!("depfilter")
        .args([
            "select",
            "--pattern",
            "myapp",
            "--key",
            "label",
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
    assert_eq!(stdout, "3\tmyapp\n#\n");
}

#[test]
#[ignore]
fn select_with_deps() {
    // Select myapp and include all its dependencies
    let output = tool!("depfilter")
        .args([
            "select",
            "--pattern",
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
#[ignore]
fn select_with_ancestors() {
    // Select libbar and include all nodes that depend on it
    let output = tool!("depfilter")
        .args([
            "select",
            "--pattern",
            "libbar",
            "--ancestors",
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
#[ignore]
fn select_with_depth() {
    // Create a deeper graph for depth testing
    // a -> b -> c -> d
    let deep_graph = "a\nb\nc\nd\n#\na\tb\nb\tc\nc\td\n";
    let output = tool!("depfilter")
        .args([
            "select",
            "--pattern",
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
fn select_multiple_patterns_or() {
    let output = tool!("depfilter")
        .args([
            "select",
            "--pattern",
            "libfoo",
            "--pattern",
            "myapp",
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
    // Should include libfoo and myapp (OR logic), plus edge between them
    assert_eq!(stdout, "1\tlibfoo\n3\tmyapp\n#\n3\t1\n");
}

#[test]
fn select_multiple_patterns_and() {
    // Graph with nodes that match multiple criteria
    let graph =
        "libfoo-alpha\nlibfoo-beta\nlibbar-alpha\n#\n";
    let output = tool!("depfilter")
        .args([
            "select",
            "--pattern",
            "libfoo*",
            "--pattern",
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
#[ignore]
fn filter_single_pattern() {
    let output = tool!("depfilter")
        .args([
            "filter",
            "--pattern",
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
    // Should remove libfoo but keep myapp -> libbar edge
    assert_eq!(stdout, "2\tlibbar\n3\tmyapp\n#\n3\t2\n");
}

#[test]
#[ignore]
fn filter_with_deps_cascade() {
    let output = tool!("depfilter")
        .args([
            "filter",
            "--pattern",
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
    // Should remove myapp and all its dependencies (libfoo, libbar), leaving nothing
    assert_eq!(stdout, "#\n");
}

#[test]
#[ignore]
fn filter_with_ancestors_cascade() {
    let output = tool!("depfilter")
        .args([
            "filter",
            "--pattern",
            "libbar",
            "--ancestors",
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
    // Should remove libbar and all nodes that depend on it (libfoo, myapp), leaving nothing
    assert_eq!(stdout, "#\n");
}

#[test]
#[ignore]
fn filter_preserve_connectivity() {
    // Graph: a -> b -> c
    let chain_graph = "a\nb\nc\n#\na\tb\nb\tc\n";
    let output = tool!("depfilter")
        .args([
            "filter",
            "--pattern",
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
    // Should remove b but create direct edge a -> c
    assert_eq!(stdout, "a\nc\n#\na\tc\n");
}

#[test]
#[ignore]
fn filter_preserve_connectivity_no_self_loops() {
    // Graph: a -> b, b -> a (cycle)
    let cycle_graph = "a\nb\n#\na\tb\nb\ta\n";
    let output = tool!("depfilter")
        .args([
            "filter",
            "--pattern",
            "b",
            "--preserve-connectivity",
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
        ])
        .write_stdin(cycle_graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should remove b, and not create self-loop a -> a
    assert_eq!(stdout, "a\n#\n");
}

#[test]
#[ignore]
fn filter_preserve_connectivity_no_parallel_edges() {
    // Graph: a -> b -> c, a -> c (b is already bypassed)
    let graph = "a\nb\nc\n#\na\tb\nb\tc\na\tc\n";
    let output = tool!("depfilter")
        .args([
            "filter",
            "--pattern",
            "b",
            "--preserve-connectivity",
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
    // Should remove b and not create duplicate a -> c edge
    assert_eq!(stdout, "a\nc\n#\na\tc\n");
}

#[test]
#[ignore]
fn filter_multiple_patterns_or() {
    let output = tool!("depfilter")
        .args([
            "filter",
            "--pattern",
            "libfoo",
            "--pattern",
            "libbar",
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
    // Should remove both libfoo and libbar (OR logic), leaving only myapp
    assert_eq!(stdout, "3\tmyapp\n#\n");
}

#[test]
#[ignore]
fn filter_multiple_patterns_and() {
    // Graph with nodes that match multiple criteria
    let graph =
        "libfoo-alpha\tlibfoo-alpha\nlibfoo-beta\tlibfoo-beta\nlibbar-alpha\tlibbar-alpha\n#\n";
    let output = tool!("depfilter")
        .args([
            "filter",
            "--pattern",
            "libfoo*",
            "--pattern",
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
    // Should remove only libfoo-alpha (matches both patterns)
    assert_eq!(
        stdout,
        "libfoo-beta\tlibfoo-beta\nlibbar-alpha\tlibbar-alpha\n#\n"
    );
}

#[test]
fn select_empty_result() {
    let output = tool!("depfilter")
        .args([
            "select",
            "--pattern",
            "nonexistent",
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
    // Should return empty graph
    assert_eq!(stdout, "#\n");
}

#[test]
#[ignore]
fn filter_empty_result() {
    let output = tool!("depfilter")
        .args([
            "filter",
            "--pattern",
            "nonexistent",
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
    // Should return original graph unchanged
    assert_eq!(stdout, SIMPLE_GRAPH);
}

#[test]
fn select_dot_output() {
    let output = tool!("depfilter")
        .args([
            "select",
            "--pattern",
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

#[test]
#[ignore]
fn filter_dot_input() {
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
            "filter",
            "--pattern",
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
