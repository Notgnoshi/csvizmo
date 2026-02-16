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

// -- shorten integration tests --

#[test]
fn shorten_default_strips_common_prefix() {
    // Nodes share common prefix "src/foo/" -- defaults strip it
    let graph = "src/foo/bar.rs\nsrc/foo/baz.rs\n#\nsrc/foo/bar.rs\tsrc/foo/baz.rs\n";
    let output = tool!("deptransform")
        .args(["shorten", "--input-format", "tgf", "--output-format", "tgf"])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "bar.rs\nbaz.rs\n#\nbar.rs\tbaz.rs\n");
}

#[test]
fn shorten_dot_separator() {
    let graph = "com.example.foo\ncom.example.bar\n#\ncom.example.foo\tcom.example.bar\n";
    let output = tool!("deptransform")
        .args([
            "shorten",
            "--separator",
            ".",
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
    assert_eq!(stdout, "foo\nbar\n#\nfoo\tbar\n");
}

#[test]
fn shorten_id_only() {
    // --key id: shorten IDs but leave labels untouched
    let graph = "src/foo/bar.rs\tOriginal\nsrc/foo/baz.rs\tOther\n#\n";
    let output = tool!("deptransform")
        .args([
            "shorten",
            "--key",
            "id",
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
    assert_eq!(stdout, "bar.rs\tOriginal\nbaz.rs\tOther\n#\n");
}

#[test]
fn shorten_single_letter() {
    // Explicit --single-letter overrides defaults
    let graph = "src/foo/bar.rs\n#\n";
    let output = tool!("deptransform")
        .args([
            "shorten",
            "--single-letter",
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
    assert_eq!(stdout, "s/f/bar.rs\n#\n");
}

// -- sub integration tests --

#[test]
fn sub_id_no_collision() {
    // Rename node IDs with no collisions; labels are preserved from original
    let graph = "a.do_compile\nb.do_build\n#\na.do_compile\tb.do_build\n";
    let output = tool!("deptransform")
        .args([
            "sub",
            "s/\\.do_.*//",
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
    assert_eq!(stdout, "a\ta.do_compile\nb\tb.do_build\n#\na\tb\n");
}

#[test]
fn sub_id_merges_and_removes_self_loops() {
    // Two nodes map to the same ID; self-loop removed, first label wins
    let graph = "a.x\ta.x\na.y\ta.y\n#\na.x\ta.y\n";
    let output = tool!("deptransform")
        .args([
            "sub",
            "s/\\..*//",
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
    assert_eq!(stdout, "a\ta.x\n#\n");
}

#[test]
fn sub_id_deduplicates_edges() {
    // a.x -> b, a.y -> b both become a -> b; only one edge kept
    let graph = "a.x\ta.x\na.y\ta.y\nb\tb\n#\na.x\tb\na.y\tb\n";
    let output = tool!("deptransform")
        .args([
            "sub",
            "s/\\..*//",
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
    assert_eq!(stdout, "a\ta.x\nb\n#\na\tb\n");
}

#[test]
fn sub_node_label() {
    let graph = "a\thello world\nb\tgoodbye world\n#\n";
    let output = tool!("deptransform")
        .args([
            "sub",
            "--key",
            "node:label",
            "s/world/earth/",
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
    assert_eq!(stdout, "a\thello earth\nb\tgoodbye earth\n#\n");
}

#[test]
fn sub_alternate_delimiter() {
    let graph = "a/b\nc/d\n#\na/b\tc/d\n";
    let output = tool!("deptransform")
        .args([
            "sub",
            "s|/|.|",
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
    assert_eq!(stdout, "a.b\ta/b\nc.d\tc/d\n#\na.b\tc.d\n");
}

#[test]
fn sub_capture_groups() {
    // Use capture group to extract first component
    let graph = "foo.bar\nbaz.qux\n#\nfoo.bar\tbaz.qux\n";
    let output = tool!("deptransform")
        .args([
            "sub",
            "s/([^.]+)\\..*/$1/",
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
    assert_eq!(stdout, "foo\tfoo.bar\nbaz\tbaz.qux\n#\nfoo\tbaz\n");
}

#[test]
fn sub_invalid_expr() {
    let graph = "a\n#\n";
    let output = tool!("deptransform")
        .args([
            "sub",
            "not-a-substitution",
            "--input-format",
            "tgf",
            "--output-format",
            "tgf",
        ])
        .write_stdin(graph)
        .captured_output()
        .unwrap();
    assert!(!output.status.success());
}
