use csvizmo_test::{CommandExt, tool};
use pretty_assertions::assert_eq;

/// Normalize whitespace for comparison: split each line into tokens, rejoin with single spaces.
fn normalize_whitespace(s: &str) -> String {
    s.lines()
        .map(|line| {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            tokens.join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn empty_input() {
    let output = tool!("depconv")
        .args(["--from", "tgf", "--to", "tgf"])
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert_eq!(stdout, "#\n");
}

#[test]
fn tgf_roundtrip() {
    let input = include_str!("../../../data/depconv/edge-labels.tgf");
    let output = tool!("depconv")
        .args(["--from", "tgf", "--to", "tgf"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(normalize_whitespace(&stdout), normalize_whitespace(input));
}

#[test]
fn tgf_to_dot() {
    let input = include_str!("../../../data/depconv/small.tgf");
    let output = tool!("depconv")
        .args(["--from", "tgf", "--to", "dot"])
        .write_stdin(input)
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
    \"3\" [label=\"myapp\"];
    \"3\" -> \"1\";
    \"3\" -> \"2\";
    \"1\" -> \"2\";
}
"
    );
}

#[cfg(feature = "dot")]
#[test]
fn dot_to_tgf() {
    let input = include_str!("../../../data/depconv/small.dot");
    let output = tool!("depconv")
        .args(["--from", "dot", "--to", "tgf"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "libbar\tlibbar\nlibfoo\tlibfoo\nmyapp\tMy Application\n#\nmyapp\tlibfoo\nmyapp\tlibbar\nlibfoo\tlibbar\n"
    );
}

#[cfg(feature = "dot")]
#[test]
fn dot_edge_and_graph_attrs_roundtrip() {
    let input = r#"digraph deps { rankdir=LR; a [label="A", shape=box]; b; a -> b [style="dashed", color="red"]; }"#;
    let output = tool!("depconv")
        .args(["--from", "dot", "--to", "dot"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph deps {
    rankdir=\"LR\";
    a [label=\"A\", shape=\"box\"];
    b;
    a -> b [style=\"dashed\", color=\"red\"];
}
"
    );
}

#[cfg(feature = "dot")]
#[test]
fn tgf_to_dot_to_tgf_roundtrip() {
    let input = "a\tAlpha\nb\tBravo\n#\na\tb\tuses\n";
    // TGF → DOT
    let dot_output = tool!("depconv")
        .args(["--from", "tgf", "--to", "dot"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(dot_output.status.success());
    let dot = String::from_utf8_lossy(&dot_output.stdout);
    // DOT → TGF
    let tgf_output = tool!("depconv")
        .args(["--from", "dot", "--to", "tgf"])
        .write_stdin(dot.as_ref())
        .captured_output()
        .unwrap();
    assert!(tgf_output.status.success());
    let tgf = String::from_utf8_lossy(&tgf_output.stdout);
    assert_eq!(tgf, input);
}

#[cfg(feature = "dot")]
#[test]
fn dot_to_dot() {
    let input = include_str!("../../../data/depconv/small.dot");
    let output = tool!("depconv")
        .args(["--from", "dot", "--to", "dot"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph deps {
    rankdir=\"LR\";
    libbar [label=\"libbar\"];
    libfoo [label=\"libfoo\"];
    myapp [label=\"My Application\", shape=\"box\"];
    myapp -> libfoo;
    myapp -> libbar;
    libfoo -> libbar;
}
"
    );
}
