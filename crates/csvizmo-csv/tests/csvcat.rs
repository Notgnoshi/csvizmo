use csvizmo_test::{CommandExt, tempfile, tool};
use pretty_assertions::assert_eq;

#[test]
fn cat_nothing() {
    let output = tool!("csvcat").captured_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert_eq!(stdout, "");
}

#[test]
fn cat_from_stdin() {
    let input = "foo,bar,baz\n";
    let output = tool!("csvcat")
        .write_stdin(input)
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert_eq!(stdout, input);
}

#[test]
fn cat_no_header() {
    let input_stdin = "1,2,3\n";
    let input_file = tempfile("a,b,c\n").unwrap();
    let expected = "1,2,3\na,b,c\n";

    let output = tool!("csvcat")
        .arg("--no-header")
        .arg("-")
        .arg(input_file.path())
        .write_stdin(input_stdin)
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert_eq!(expected, stdout);
}

#[test]
fn cat_with_header() {
    let input_stdin = "foo,bar,baz\na,b,c\n";
    // headers don't have to match
    let input_file = tempfile("FOO,BAR,BAZ\n1,2,3\n").unwrap();
    let expected = "foo,bar,baz\na,b,c\n1,2,3\n";

    let output = tool!("csvcat")
        .arg("-")
        .arg(input_file.path())
        .write_stdin(input_stdin)
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert_eq!(expected, stdout);
}

#[test]
fn cat_ragged_columns() {
    let input = "1,2,3\n1,2,3,4\n";
    let expected = "1,2,3\n";

    let output = tool!("csvcat")
        .write_stdin(input)
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !output.status.success(),
        "Does not accept ragged input within a single file"
    );
    assert_eq!(expected, stdout);

    let expected = "1,2,3\n1,2,3,4\n";
    let output = tool!("csvcat")
        .arg("--allow-ragged")
        .write_stdin(input)
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert_eq!(expected, stdout);

    let file1 = tempfile("1,2,3\n").unwrap();
    let file2 = tempfile("a,b,c,d\n").unwrap();
    let expected = "1,2,3\n";

    let output = tool!("csvcat")
        .arg("--no-header")
        .arg(file1.path())
        .arg(file2.path())
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !output.status.success(),
        "Does not accept ragged input within a single file"
    );
    assert_eq!(expected, stdout);

    let expected = "1,2,3\na,b,c,d\n";
    let output = tool!("csvcat")
        .arg("--allow-ragged")
        .arg("--no-header")
        .arg(file1.path())
        .arg(file2.path())
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert_eq!(expected, stdout);
}
