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
digraph {
    libbar [label=\"libbar\"];
    libfoo [label=\"libfoo\"];
    myapp [label=\"My Application\", type=\"box\"];
    myapp -> libfoo;
    myapp -> libbar;
    libfoo -> libbar;
}
"
    );
}
