use csvizmo_test::{CommandExt, tool};
use pretty_assertions::assert_eq;

// Test graph: a -> b -> c, a -> c
const SIMPLE_GRAPH: &str = "1\talpha\n2\tbeta\n3\tgamma\n#\n1\t2\n2\t3\n1\t3\n";

// Chain: a -> b -> c -> d
const CHAIN_GRAPH: &str = "a\nb\nc\nd\n#\na\tb\nb\tc\nc\td\n";

// Diamond: a -> b, a -> c, b -> d, c -> d
const DIAMOND_GRAPH: &str = "a\nb\nc\nd\n#\na\tb\na\tc\nb\td\nc\td\n";

// -- nodes subcommand --

#[test]
fn nodes_all_default() {
    let output = tool!("depquery")
        .args(["nodes", "--input-format", "tgf"])
        .write_stdin(SIMPLE_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "alpha\nbeta\ngamma\n");
}

#[test]
fn nodes_format_id() {
    let output = tool!("depquery")
        .args(["nodes", "--format", "id", "--input-format", "tgf"])
        .write_stdin(SIMPLE_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "1\n2\n3\n");
}

#[test]
fn nodes_select_roots() {
    let output = tool!("depquery")
        .args(["nodes", "--select", "roots", "--input-format", "tgf"])
        .write_stdin(CHAIN_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "a\n");
}

#[test]
fn nodes_select_leaves() {
    let output = tool!("depquery")
        .args(["nodes", "--select", "leaves", "--input-format", "tgf"])
        .write_stdin(CHAIN_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "d\n");
}

#[test]
fn nodes_include_pattern() {
    let output = tool!("depquery")
        .args(["nodes", "-g", "al*", "--input-format", "tgf"])
        .write_stdin(SIMPLE_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "alpha\n");
}

#[test]
fn nodes_exclude_pattern() {
    let output = tool!("depquery")
        .args(["nodes", "-x", "b*", "--input-format", "tgf"])
        .write_stdin(SIMPLE_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "alpha\ngamma\n");
}

#[test]
fn nodes_sort_topo() {
    let output = tool!("depquery")
        .args(["nodes", "--sort", "topo", "--input-format", "tgf"])
        .write_stdin(CHAIN_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "a\nb\nc\nd\n");
}

#[test]
fn nodes_sort_topo_reverse() {
    let output = tool!("depquery")
        .args([
            "nodes",
            "--sort",
            "topo",
            "--reverse",
            "--input-format",
            "tgf",
        ])
        .write_stdin(CHAIN_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "d\nc\nb\na\n");
}

#[test]
fn nodes_sort_out_degree() {
    let output = tool!("depquery")
        .args(["nodes", "--sort", "out-degree", "--input-format", "tgf"])
        .write_stdin(SIMPLE_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // alpha(id=1) has out-degree 2, beta(id=2) has 1, gamma(id=3) has 0
    assert_eq!(stdout, "alpha\t2\nbeta\t1\ngamma\t0\n");
}

#[test]
fn nodes_sort_in_degree() {
    let output = tool!("depquery")
        .args(["nodes", "--sort", "in-degree", "--input-format", "tgf"])
        .write_stdin(SIMPLE_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // gamma(id=3) has in-degree 2, beta(id=2) has 1, alpha(id=1) has 0
    assert_eq!(stdout, "gamma\t2\nbeta\t1\nalpha\t0\n");
}

#[test]
fn nodes_limit() {
    let output = tool!("depquery")
        .args([
            "nodes",
            "--sort",
            "topo",
            "--limit",
            "2",
            "--input-format",
            "tgf",
        ])
        .write_stdin(CHAIN_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "a\nb\n");
}

#[test]
fn nodes_deps_from_roots() {
    let output = tool!("depquery")
        .args([
            "nodes",
            "--select",
            "roots",
            "--deps",
            "--depth",
            "1",
            "--sort",
            "topo",
            "--input-format",
            "tgf",
        ])
        .write_stdin(CHAIN_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "a\nb\n");
}

#[test]
fn nodes_rdeps_from_leaves() {
    let output = tool!("depquery")
        .args([
            "nodes",
            "--select",
            "leaves",
            "--rdeps",
            "--depth",
            "1",
            "--sort",
            "topo",
            "--input-format",
            "tgf",
        ])
        .write_stdin(CHAIN_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "c\nd\n");
}

#[test]
fn nodes_match_by_id() {
    let output = tool!("depquery")
        .args(["nodes", "-g", "1", "--key", "id", "--input-format", "tgf"])
        .write_stdin(SIMPLE_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "alpha\n");
}

#[test]
fn nodes_include_and_mode() {
    let graph = "foo-alpha\nfoo-beta\nbar-alpha\n#\n";
    let output = tool!("depquery")
        .args([
            "nodes",
            "-g",
            "foo*",
            "-g",
            "*alpha",
            "--and",
            "--input-format",
            "tgf",
        ])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "foo-alpha\n");
}

// -- edges subcommand --

#[test]
fn edges_all_default() {
    let output = tool!("depquery")
        .args(["edges", "--input-format", "tgf"])
        .write_stdin(SIMPLE_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Edge order follows TGF input: 1->2, 2->3, 1->3
    assert_eq!(stdout, "alpha\tbeta\nbeta\tgamma\nalpha\tgamma\n");
}

#[test]
fn edges_format_id() {
    let output = tool!("depquery")
        .args(["edges", "--format", "id", "--input-format", "tgf"])
        .write_stdin(SIMPLE_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Edge order follows TGF input: 1->2, 2->3, 1->3
    assert_eq!(stdout, "1\t2\n2\t3\n1\t3\n");
}

#[test]
fn edges_include_filter() {
    let output = tool!("depquery")
        .args(["edges", "-g", "alpha", "--input-format", "tgf"])
        .write_stdin(SIMPLE_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // edges touching "alpha": alpha->beta, alpha->gamma
    assert_eq!(stdout, "alpha\tbeta\nalpha\tgamma\n");
}

#[test]
fn edges_exclude_filter() {
    let output = tool!("depquery")
        .args(["edges", "-x", "gamma", "--input-format", "tgf"])
        .write_stdin(SIMPLE_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // exclude edges touching gamma: only alpha->beta remains
    assert_eq!(stdout, "alpha\tbeta\n");
}

#[test]
fn edges_sort_by_source() {
    let graph = "a\tC\nb\tA\nc\tB\n#\na\tb\nc\ta\nb\tc\n";
    let output = tool!("depquery")
        .args(["edges", "--sort", "source", "--input-format", "tgf"])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "A\tB\nB\tC\nC\tA\n");
}

#[test]
fn edges_limit() {
    let output = tool!("depquery")
        .args(["edges", "--limit", "1", "--input-format", "tgf"])
        .write_stdin(SIMPLE_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "alpha\tbeta\n");
}

// -- metrics subcommand --

#[test]
fn metrics_chain() {
    let output = tool!("depquery")
        .args(["metrics", "--input-format", "tgf"])
        .write_stdin(CHAIN_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
nodes\t4
edges\t3
roots\t1
leaves\t1
max_depth\t3
max_fan_out\t1
max_fan_in\t1
avg_fan_out\t0.75
density\t0.250000
cycles\t0
diamonds\t0
components\t1
"
    );
}

#[test]
fn metrics_diamond() {
    let output = tool!("depquery")
        .args(["metrics", "--input-format", "tgf"])
        .write_stdin(DIAMOND_GRAPH)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
nodes\t4
edges\t4
roots\t1
leaves\t1
max_depth\t2
max_fan_out\t2
max_fan_in\t2
avg_fan_out\t1.00
density\t0.333333
cycles\t0
diamonds\t1
components\t1
"
    );
}

#[test]
fn metrics_cycle() {
    let graph = "a\nb\nc\n#\na\tb\nb\tc\nc\ta\n";
    let output = tool!("depquery")
        .args(["metrics", "--input-format", "tgf"])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
nodes\t3
edges\t3
roots\t0
leaves\t0
max_depth\t
max_fan_out\t1
max_fan_in\t1
avg_fan_out\t1.00
density\t0.500000
cycles\t1
diamonds\t0
components\t1
"
    );
}

#[test]
fn metrics_empty() {
    let output = tool!("depquery")
        .args(["metrics", "--input-format", "tgf"])
        .write_stdin("#\n")
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
nodes\t0
edges\t0
roots\t0
leaves\t0
max_depth\t0
max_fan_out\t0
max_fan_in\t0
avg_fan_out\t0.00
density\t0.000000
cycles\t0
diamonds\t0
components\t0
"
    );
}

#[test]
fn metrics_disjoint() {
    // Two separate components: a -> b, c -> d
    let graph = "a\nb\nc\nd\n#\na\tb\nc\td\n";
    let output = tool!("depquery")
        .args(["metrics", "--input-format", "tgf"])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
nodes\t4
edges\t2
roots\t2
leaves\t2
max_depth\t1
max_fan_out\t1
max_fan_in\t1
avg_fan_out\t0.50
density\t0.166667
cycles\t0
diamonds\t0
components\t2
"
    );
}
