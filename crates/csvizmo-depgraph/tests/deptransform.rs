use csvizmo_test::{CommandExt, tool};
use pretty_assertions::assert_eq;

// -- reverse integration tests --

#[test]
fn reverse_simple_chain() {
    // a -> b -> c becomes c -> b, b -> a
    let graph = "a\nb\nc\n#\na\tb\nb\tc\n";
    let output = tool!("deptransform")
        .args(["reverse", "--input-format", "tgf", "--output-format", "tgf"])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "a\nb\nc\n#\nb\ta\nc\tb\n");
}

#[test]
fn reverse_preserves_labels() {
    let graph = "1\tAlpha\n2\tBeta\n#\n1\t2\n";
    let output = tool!("deptransform")
        .args(["reverse", "--input-format", "tgf", "--output-format", "tgf"])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "1\tAlpha\n2\tBeta\n#\n2\t1\n");
}

#[test]
fn reverse_empty_graph() {
    let graph = "#\n";
    let output = tool!("deptransform")
        .args(["reverse", "--input-format", "tgf", "--output-format", "tgf"])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "#\n");
}

#[test]
fn reverse_dot_output() {
    let graph = "a\nb\n#\na\tb\n";
    let output = tool!("deptransform")
        .args(["reverse", "--input-format", "tgf", "--output-format", "dot"])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    a;
    b;
    b -> a;
}
"
    );
}

// -- simplify integration tests --

#[test]
fn simplify_removes_redundant_edge() {
    // a -> b -> c, a -> c: the direct a->c is redundant
    let graph = "a\nb\nc\n#\na\tb\nb\tc\na\tc\n";
    let output = tool!("deptransform")
        .args([
            "simplify",
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
fn simplify_diamond() {
    // a -> b -> d, a -> c -> d, a -> d: a->d is redundant
    let graph = "a\nb\nc\nd\n#\na\tb\na\tc\nb\td\nc\td\na\td\n";
    let output = tool!("deptransform")
        .args([
            "simplify",
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
    assert_eq!(stdout, "a\nb\nc\nd\n#\na\tb\na\tc\nb\td\nc\td\n");
}

#[test]
fn simplify_no_redundant_edges() {
    // a -> b -> c: nothing to remove
    let graph = "a\nb\nc\n#\na\tb\nb\tc\n";
    let output = tool!("deptransform")
        .args([
            "simplify",
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
fn simplify_errors_on_cycle() {
    // a -> b -> a: cycle, should fail
    let graph = "a\nb\n#\na\tb\nb\ta\n";
    let output = tool!("deptransform")
        .args([
            "simplify",
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
        ])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cycles"), "stderr: {stderr}");
}
