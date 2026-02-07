use csvizmo_test::{CommandExt, tool};
use pretty_assertions::assert_eq;

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
