use csvizmo_test::{CommandExt, tool};
use pretty_assertions::assert_eq;

/// Two disconnected components: a-b and c-d.
const TWO_COMPONENTS: &str = "1\ta\n2\tb\n3\tc\n4\td\n#\n1\t2\n3\t4\n";

/// Single clique: a -> b -> c -> a (all connected).
const SINGLE_CLIQUE: &str = "1\ta\n2\tb\n3\tc\n#\n1\t2\n2\t3\n3\t1\n";

// -- LPA tests (deterministic without seed) --

#[test]
fn lpa_two_disconnected_components() {
    let output = tool!("depcluster")
        .args(["--input-format", "tgf", "-a", "lpa"])
        .write_stdin(TWO_COMPONENTS)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    subgraph cluster_0 {
        \"1\" [label=\"a\"];
        \"2\" [label=\"b\"];
        \"1\" -> \"2\";
    }
    subgraph cluster_1 {
        \"3\" [label=\"c\"];
        \"4\" [label=\"d\"];
        \"3\" -> \"4\";
    }
}
"
    );
}

#[test]
fn lpa_single_clique() {
    let output = tool!("depcluster")
        .args(["--input-format", "tgf", "-a", "lpa"])
        .write_stdin(SINGLE_CLIQUE)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    subgraph cluster_0 {
        \"1\" [label=\"a\"];
        \"2\" [label=\"b\"];
        \"3\" [label=\"c\"];
        \"1\" -> \"2\";
        \"2\" -> \"3\";
        \"3\" -> \"1\";
    }
}
"
    );
}

#[test]
fn lpa_empty_graph() {
    let output = tool!("depcluster")
        .args(["--input-format", "tgf", "-a", "lpa"])
        .write_stdin("")
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "digraph {\n}\n");
}

#[test]
fn lpa_directed_flag() {
    let output = tool!("depcluster")
        .args(["--input-format", "tgf", "-a", "lpa", "--directed"])
        .write_stdin(TWO_COMPONENTS)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // With --directed, should still find two components (no cross edges)
    assert!(stdout.contains("cluster_0"));
    assert!(stdout.contains("cluster_1"));
}

#[test]
fn lpa_seed_flag() {
    let output = tool!("depcluster")
        .args(["--input-format", "tgf", "-a", "lpa", "--seed", "42"])
        .write_stdin(TWO_COMPONENTS)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Two disconnected components should still produce two clusters
    assert!(stdout.contains("cluster_0"));
    assert!(stdout.contains("cluster_1"));
}

#[test]
fn lpa_max_iter_flag() {
    let output = tool!("depcluster")
        .args(["--input-format", "tgf", "-a", "lpa", "--max-iter", "1"])
        .write_stdin(TWO_COMPONENTS)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
}

// -- Louvain tests --

#[test]
fn louvain_two_disconnected_components() {
    let output = tool!("depcluster")
        .args(["--input-format", "tgf", "-a", "louvain"])
        .write_stdin(TWO_COMPONENTS)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should produce two clusters with all four nodes and no cross edges
    assert!(stdout.contains("cluster_0"));
    assert!(stdout.contains("cluster_1"));
    assert!(!stdout.contains("cluster_2"));
    for node in &["\"1\"", "\"2\"", "\"3\"", "\"4\""] {
        assert!(stdout.contains(node), "missing node {node}");
    }
}

#[test]
fn louvain_resolution_flag() {
    let output = tool!("depcluster")
        .args([
            "--input-format",
            "tgf",
            "-a",
            "louvain",
            "--resolution",
            "0.5",
        ])
        .write_stdin(TWO_COMPONENTS)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn louvain_seed_flag() {
    let output = tool!("depcluster")
        .args(["--input-format", "tgf", "-a", "louvain", "--seed", "123"])
        .write_stdin(TWO_COMPONENTS)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn louvain_empty_graph() {
    let output = tool!("depcluster")
        .args(["--input-format", "tgf", "-a", "louvain"])
        .write_stdin("")
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "digraph {\n}\n");
}

// -- Leiden tests --

#[test]
fn leiden_accepts_input() {
    let output = tool!("depcluster")
        .args(["--input-format", "tgf", "-a", "leiden"])
        .write_stdin(TWO_COMPONENTS)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // All nodes should appear somewhere in the output
    for node in &["\"1\"", "\"2\"", "\"3\"", "\"4\""] {
        assert!(stdout.contains(node), "missing node {node}");
    }
}

#[test]
fn leiden_resolution_flag() {
    let output = tool!("depcluster")
        .args([
            "--input-format",
            "tgf",
            "-a",
            "leiden",
            "--resolution",
            "0.1",
        ])
        .write_stdin(TWO_COMPONENTS)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn leiden_empty_graph() {
    let output = tool!("depcluster")
        .args(["--input-format", "tgf", "-a", "leiden"])
        .write_stdin("")
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "digraph {\n}\n");
}

// -- Cross-algorithm tests --

#[test]
fn cross_cluster_edges_at_top_level() {
    // Two cliques connected by one directed edge: a<->b connected to c<->d via b->c.
    // In directed mode, each node only has 1 outgoing neighbor within its pair,
    // so the single cross-edge b->c doesn't merge the clusters.
    let input = "1\ta\n2\tb\n3\tc\n4\td\n#\n1\t2\n2\t1\n3\t4\n4\t3\n2\t3\n";
    let output = tool!("depcluster")
        .args(["--input-format", "tgf", "-a", "lpa", "--directed"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    subgraph cluster_0 {
        \"1\" [label=\"a\"];
        \"2\" [label=\"b\"];
        \"1\" -> \"2\";
        \"2\" -> \"1\";
    }
    subgraph cluster_1 {
        \"3\" [label=\"c\"];
        \"4\" [label=\"d\"];
        \"3\" -> \"4\";
        \"4\" -> \"3\";
    }
    \"2\" -> \"3\";
}
"
    );
}

#[test]
fn output_format_tgf() {
    let output = tool!("depcluster")
        .args([
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
            "-a",
            "lpa",
        ])
        .write_stdin(TWO_COMPONENTS)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // TGF doesn't support subgraphs, but the output should still be valid
    assert!(stdout.contains("#"));
}
