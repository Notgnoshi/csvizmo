use pretty_assertions::assert_eq;

use crate::{csvdelta, CommandExt};

#[test]
fn test_column_name_or_index() {
    let input = b"\
        header1,header2\n\
        0,a\n\
        1,b\n\
        2,c\n\
    ";

    let expected = "\
        header1,header2,header1-deltas\n\
        0,a,\n\
        1,b,1\n\
        2,c,1\n\
    ";

    // Try the column name
    let mut cmd = csvdelta();
    cmd.arg("--column").arg("header1");
    let output = cmd.write_stdin(input).captured_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, expected);

    // Try a column index
    let mut cmd = csvdelta();
    cmd.arg("--column").arg("0");
    let output = cmd.write_stdin(input).captured_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, expected);

    // Do it again without a header
    let input = b"\
        0,a\n\
        1,b\n\
        2,c\n\
    ";

    let expected = "\
        0,a,\n\
        1,b,1\n\
        2,c,1\n\
    ";
    let mut cmd = csvdelta();
    cmd.arg("--column").arg("0").arg("--no-header");
    let output = cmd.write_stdin(input).captured_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, expected);
}

#[test]
fn test_center_mean() {
    // 0..=4 has a mean of 2
    let input = b"\
        0\n\
        1\n\
        2\n\
        3\n\
        4\n\
    ";

    let expected = "\
        0,-2\n\
        1,-1\n\
        2,0\n\
        3,1\n\
        4,2\n\
    ";

    let mut cmd = csvdelta();
    cmd.arg("--column")
        .arg("0")
        .arg("--center-mean")
        .arg("--no-header");
    let output = cmd.write_stdin(input).captured_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, expected);
}

#[test]
fn test_center_first() {
    let input = b"\
        6\n\
        1\n\
        2\n\
        3\n\
        4\n\
    ";

    let expected = "\
        6,0\n\
        1,-5\n\
        2,-4\n\
        3,-3\n\
        4,-2\n\
    ";

    let mut cmd = csvdelta();
    cmd.arg("--column")
        .arg("0")
        .arg("--center-first")
        .arg("--no-header");
    let output = cmd.write_stdin(input).captured_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, expected);
}

#[test]
fn test_center_value() {
    let input = b"\
        1\n\
        2\n\
        3\n\
        4\n\
    ";

    let expected = "\
        1,-1\n\
        2,0\n\
        3,1\n\
        4,2\n\
    ";

    let mut cmd = csvdelta();
    cmd.arg("--column")
        .arg("0")
        .arg("--center-value")
        .arg("2")
        .arg("--no-header");
    let output = cmd.write_stdin(input).captured_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, expected);
}
