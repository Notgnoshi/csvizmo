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
fn select_with_deps_and_ancestors() {
    // a -> b -> c -> d: select b with both directions gets everything
    let graph = "a\nb\nc\nd\n#\na\tb\nb\tc\nc\td\n";
    let output = tool!("depfilter")
        .args([
            "select",
            "--pattern",
            "b",
            "--deps",
            "--ancestors",
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

// -- filter integration tests: one per CLI flag --

#[test]
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
    assert_eq!(stdout, "2\tlibbar\n3\tmyapp\n#\n3\t2\n");
}

#[test]
fn filter_with_and() {
    let graph = "libfoo-alpha\nlibfoo-beta\nlibbar-alpha\n#\n";
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
    assert_eq!(stdout, "libfoo-beta\nlibbar-alpha\n#\n");
}

#[test]
fn filter_with_deps() {
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
    assert_eq!(stdout, "#\n");
}

#[test]
fn filter_with_ancestors() {
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
    assert_eq!(stdout, "#\n");
}

#[test]
fn filter_with_deps_and_ancestors() {
    // a -> b -> c, d -> c: filter b with both directions removes a, b, c but keeps d
    let graph = "a\nb\nc\nd\n#\na\tb\nb\tc\nd\tc\n";
    let output = tool!("depfilter")
        .args([
            "filter",
            "--pattern",
            "b",
            "--deps",
            "--ancestors",
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
    assert_eq!(stdout, "d\n#\n");
}

#[test]
fn filter_by_id() {
    let output = tool!("depfilter")
        .args([
            "filter",
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
    assert_eq!(stdout, "2\tlibbar\n3\tmyapp\n#\n3\t2\n");
}

#[test]
fn filter_with_preserve_connectivity() {
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
    assert_eq!(stdout, "a\nc\n#\na\tc\n");
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
fn filter_preserve_connectivity_subgraph() {
    // subgraph { a -> b -> c }: remove b, bypass a -> c stays in subgraph
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
            "filter",
            "--pattern",
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

#[test]
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
