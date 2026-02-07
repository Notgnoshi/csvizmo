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
