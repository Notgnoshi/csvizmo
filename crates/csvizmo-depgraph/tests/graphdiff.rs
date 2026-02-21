use csvizmo_test::{CommandExt, tempfile, tool};
use pretty_assertions::assert_eq;

// -- annotate --

#[test]
fn annotate_identical_tgf() {
    let f1 = tempfile("a\nb\n#\na\tb\n").unwrap();
    let f2 = tempfile("a\nb\n#\na\tb\n").unwrap();

    let output = tool!("graphdiff")
        .args([
            "annotate",
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
        ])
        .arg(f1.path())
        .arg(f2.path())
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Unchanged nodes keep their labels, TGF drops diff attrs
    assert_eq!(stdout, "a\nb\n#\na\tb\n");
}

#[test]
fn annotate_added_removed_dot() {
    let f1 = tempfile("a\n#\n").unwrap();
    let f2 = tempfile("b\n#\n").unwrap();

    let output = tool!("graphdiff")
        .args([
            "annotate",
            "--input-format",
            "tgf",
            "--output-format",
            "dot",
        ])
        .arg(f1.path())
        .arg(f2.path())
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    b [label=\"+ b\", color=\"green\", fontcolor=\"green\", diff=\"added\"];
    a [label=\"- a\", color=\"red\", fontcolor=\"red\", diff=\"removed\"];
}
"
    );
}

#[test]
fn annotate_changed_dot() {
    let f1 = tempfile("a\tAlpha\n#\n").unwrap();
    let f2 = tempfile("a\tAleph\n#\n").unwrap();

    let output = tool!("graphdiff")
        .args([
            "annotate",
            "--input-format",
            "tgf",
            "--output-format",
            "dot",
        ])
        .arg(f1.path())
        .arg(f2.path())
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    a [label=\"~ Aleph\", color=\"orange\", fontcolor=\"orange\", diff=\"changed\"];
}
"
    );
}

#[test]
fn annotate_edge_annotations_dot() {
    let f1 = tempfile("a\nb\n#\na\tb\n").unwrap();
    let f2 = tempfile("a\nc\n#\na\tc\n").unwrap();

    let output = tool!("graphdiff")
        .args([
            "annotate",
            "--input-format",
            "tgf",
            "--output-format",
            "dot",
        ])
        .arg(f1.path())
        .arg(f2.path())
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    a [diff=\"unchanged\"];
    c [label=\"+ c\", color=\"green\", fontcolor=\"green\", diff=\"added\"];
    b [label=\"- b\", color=\"red\", fontcolor=\"red\", diff=\"removed\"];
    a -> c [color=\"green\", diff=\"added\"];
    a -> b [color=\"red\", diff=\"removed\"];
}
"
    );
}

#[test]
fn annotate_cluster_dot() {
    let f1 = tempfile("a\n#\n").unwrap();
    let f2 = tempfile("b\n#\n").unwrap();

    let output = tool!("graphdiff")
        .args([
            "annotate",
            "--cluster",
            "--input-format",
            "tgf",
            "--output-format",
            "dot",
        ])
        .arg(f1.path())
        .arg(f2.path())
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    subgraph cluster_removed {
        a [label=\"- a\", color=\"red\", fontcolor=\"red\", diff=\"removed\"];
    }
    b [label=\"+ b\", color=\"green\", fontcolor=\"green\", diff=\"added\"];
}
"
    );
}

// -- list --

#[test]
fn list_basic() {
    let f1 = tempfile("a\tAlpha\nb\n#\na\tb\n").unwrap();
    let f2 = tempfile("b\nc\n#\nb\tc\n").unwrap();

    let output = tool!("graphdiff")
        .args(["list", "--input-format", "tgf"])
        .arg(f1.path())
        .arg(f2.path())
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // b unchanged (omitted), c added, a removed (with label)
    // edge b->c added, a->b removed
    assert_eq!(stdout, "+\tc\n-\ta\tAlpha\n+\tb\tc\n-\ta\tb\n");
}

#[test]
fn list_empty_diff() {
    let f1 = tempfile("a\nb\n#\na\tb\n").unwrap();
    let f2 = tempfile("a\nb\n#\na\tb\n").unwrap();

    let output = tool!("graphdiff")
        .args(["list", "--input-format", "tgf"])
        .arg(f1.path())
        .arg(f2.path())
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "");
}

// -- subtract --

#[test]
fn subtract_basic() {
    let f1 = tempfile("a\nb\nc\n#\na\tb\nb\tc\n").unwrap();
    let f2 = tempfile("c\n#\n").unwrap();

    let output = tool!("graphdiff")
        .args([
            "subtract",
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
        ])
        .arg(f1.path())
        .arg(f2.path())
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Only a and b are removed; edge a->b has both endpoints removed
    // Edge b->c is excluded because c is not removed
    assert_eq!(stdout, "a\nb\n#\na\tb\n");
}

#[test]
fn subtract_empty_when_identical() {
    let f1 = tempfile("a\n#\n").unwrap();
    let f2 = tempfile("a\n#\n").unwrap();

    let output = tool!("graphdiff")
        .args([
            "subtract",
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
        ])
        .arg(f1.path())
        .arg(f2.path())
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "#\n");
}

// -- summary --

#[test]
fn summary_basic() {
    let f1 = tempfile("a\nb\n#\na\tb\n").unwrap();
    let f2 = tempfile("b\nc\n#\nb\tc\n").unwrap();

    let output = tool!("graphdiff")
        .args(["summary", "--input-format", "tgf"])
        .arg(f1.path())
        .arg(f2.path())
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
added_nodes\t1
removed_nodes\t1
changed_nodes\t0
moved_nodes\t0
unchanged_nodes\t1
added_edges\t1
removed_edges\t1
changed_edges\t0
unchanged_edges\t0
"
    );
}

#[test]
fn summary_all_unchanged() {
    let f1 = tempfile("a\nb\n#\na\tb\n").unwrap();
    let f2 = tempfile("a\nb\n#\na\tb\n").unwrap();

    let output = tool!("graphdiff")
        .args(["summary", "--input-format", "tgf"])
        .arg(f1.path())
        .arg(f2.path())
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
added_nodes\t0
removed_nodes\t0
changed_nodes\t0
moved_nodes\t0
unchanged_nodes\t2
added_edges\t0
removed_edges\t0
changed_edges\t0
unchanged_edges\t1
"
    );
}

#[test]
fn summary_all_different() {
    let f1 = tempfile("a\nb\n#\na\tb\n").unwrap();
    let f2 = tempfile("c\nd\n#\nc\td\n").unwrap();

    let output = tool!("graphdiff")
        .args(["summary", "--input-format", "tgf"])
        .arg(f1.path())
        .arg(f2.path())
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
added_nodes\t2
removed_nodes\t2
changed_nodes\t0
moved_nodes\t0
unchanged_nodes\t0
added_edges\t1
removed_edges\t1
changed_edges\t0
unchanged_edges\t0
"
    );
}

#[test]
fn summary_empty_graphs() {
    let f1 = tempfile("#\n").unwrap();
    let f2 = tempfile("#\n").unwrap();

    let output = tool!("graphdiff")
        .args(["summary", "--input-format", "tgf"])
        .arg(f1.path())
        .arg(f2.path())
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
added_nodes\t0
removed_nodes\t0
changed_nodes\t0
moved_nodes\t0
unchanged_nodes\t0
added_edges\t0
removed_edges\t0
changed_edges\t0
unchanged_edges\t0
"
    );
}

// -- input handling --

#[test]
fn stdin_and_file() {
    let f1 = tempfile("a\n#\n").unwrap();

    let output = tool!("graphdiff")
        .args(["summary", "--input-format", "tgf"])
        .arg(f1.path())
        .arg("-")
        .write_stdin("b\n#\n")
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
added_nodes\t1
removed_nodes\t1
changed_nodes\t0
moved_nodes\t0
unchanged_nodes\t0
added_edges\t0
removed_edges\t0
changed_edges\t0
unchanged_edges\t0
"
    );
}

#[test]
fn both_stdin_error() {
    let output = tool!("graphdiff")
        .args(["summary", "--input-format", "tgf", "-", "-"])
        .write_stdin("")
        .captured_output()
        .unwrap();
    assert!(!output.status.success());
}

// -- check flag --

#[test]
fn check_identical_exits_zero() {
    let f1 = tempfile("a\nb\n#\na\tb\n").unwrap();
    let f2 = tempfile("a\nb\n#\na\tb\n").unwrap();

    let output = tool!("graphdiff")
        .args(["summary", "--check", "--input-format", "tgf"])
        .arg(f1.path())
        .arg(f2.path())
        .captured_output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn check_different_exits_nonzero() {
    let f1 = tempfile("a\n#\n").unwrap();
    let f2 = tempfile("b\n#\n").unwrap();

    let output = tool!("graphdiff")
        .args(["summary", "--check", "--input-format", "tgf"])
        .arg(f1.path())
        .arg(f2.path())
        .captured_output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn check_still_produces_output() {
    let f1 = tempfile("a\n#\n").unwrap();
    let f2 = tempfile("b\n#\n").unwrap();

    let output = tool!("graphdiff")
        .args(["list", "--check", "--input-format", "tgf"])
        .arg(f1.path())
        .arg(f2.path())
        .captured_output()
        .unwrap();
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "+\tb\n-\ta\n");
}

// -- moved --

#[test]
fn moved_node_list() {
    // c has same info but different single parent
    let f1 = tempfile("p1\nc\n#\np1\tc\n").unwrap();
    let f2 = tempfile("p2\nc\n#\np2\tc\n").unwrap();

    let output = tool!("graphdiff")
        .args(["list", "--input-format", "tgf"])
        .arg(f1.path())
        .arg(f2.path())
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // p2 added, c moved, p1 removed; edges follow
    assert_eq!(stdout, "+\tp2\n>\tc\n-\tp1\n+\tp2\tc\n-\tp1\tc\n");
}

#[test]
fn moved_node_annotate_dot() {
    let f1 = tempfile("p1\nc\n#\np1\tc\n").unwrap();
    let f2 = tempfile("p2\nc\n#\np2\tc\n").unwrap();

    let output = tool!("graphdiff")
        .args([
            "annotate",
            "--input-format",
            "tgf",
            "--output-format",
            "dot",
        ])
        .arg(f1.path())
        .arg(f2.path())
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    p2 [label=\"+ p2\", color=\"green\", fontcolor=\"green\", diff=\"added\"];
    c [label=\"> c\", color=\"blue\", fontcolor=\"blue\", diff=\"moved\"];
    p1 [label=\"- p1\", color=\"red\", fontcolor=\"red\", diff=\"removed\"];
    p2 -> c [color=\"green\", diff=\"added\"];
    p1 -> c [color=\"red\", diff=\"removed\"];
}
"
    );
}

#[test]
fn multi_parent_not_moved() {
    // c has multiple parents in both graphs -- not moved
    let f1 = tempfile("p1\np2\nc\n#\np1\tc\np2\tc\n").unwrap();
    let f2 = tempfile("p1\np3\nc\n#\np1\tc\np3\tc\n").unwrap();

    let output = tool!("graphdiff")
        .args(["summary", "--input-format", "tgf"])
        .arg(f1.path())
        .arg(f2.path())
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // c is unchanged (not moved), p1 unchanged, p3 added, p2 removed
    assert_eq!(
        stdout,
        "\
added_nodes\t1
removed_nodes\t1
changed_nodes\t0
moved_nodes\t0
unchanged_nodes\t2
added_edges\t1
removed_edges\t1
changed_edges\t0
unchanged_edges\t1
"
    );
}

// -- edge cases --

#[test]
fn changed_edges() {
    // Same edge (a->b) but different label
    let f1 = tempfile("a\nb\n#\na\tb\tuses\n").unwrap();
    let f2 = tempfile("a\nb\n#\na\tb\tdepends\n").unwrap();

    let output = tool!("graphdiff")
        .args(["summary", "--input-format", "tgf"])
        .arg(f1.path())
        .arg(f2.path())
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
added_nodes\t0
removed_nodes\t0
changed_nodes\t0
moved_nodes\t0
unchanged_nodes\t2
added_edges\t0
removed_edges\t0
changed_edges\t1
unchanged_edges\t0
"
    );
}

#[test]
fn duplicate_edges() {
    // Two edges a->b with different labels in each graph
    // "uses" is in both (unchanged), "dev" only in before (removed), "test" only in after (added)
    let f1 = tempfile("a\nb\n#\na\tb\tuses\na\tb\tdev\n").unwrap();
    let f2 = tempfile("a\nb\n#\na\tb\tuses\na\tb\ttest\n").unwrap();

    let output = tool!("graphdiff")
        .args(["summary", "--input-format", "tgf"])
        .arg(f1.path())
        .arg(f2.path())
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // "uses" matched unchanged, "test" paired with "dev" as changed
    assert_eq!(
        stdout,
        "\
added_nodes\t0
removed_nodes\t0
changed_nodes\t0
moved_nodes\t0
unchanged_nodes\t2
added_edges\t0
removed_edges\t0
changed_edges\t1
unchanged_edges\t1
"
    );
}

#[test]
fn nodes_only_graphs() {
    let f1 = tempfile("a\nb\n#\n").unwrap();
    let f2 = tempfile("b\nc\n#\n").unwrap();

    let output = tool!("graphdiff")
        .args(["list", "--input-format", "tgf"])
        .arg(f1.path())
        .arg(f2.path())
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "+\tc\n-\ta\n");
}
