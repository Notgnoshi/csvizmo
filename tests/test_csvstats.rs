use pretty_assertions::assert_eq;

use crate::{CommandExt, csvstats};

#[test]
fn test_csvstats_no_plotting() {
    // Real statistics from two sessions of a D&D campain. RIP my character.
    let input = b"\
        rolls-session-1,rolls-session-2\n\
        2,8\n\
        2,7\n\
        19,14\n\
        2,14\n\
        20,15\n\
        2,2\n\
        2,12\n\
        14,12\n\
        19,6\n\
        5,4\n\
        12,3\n\
        11,5\n\
        2,7\n\
        12,6\n\
        8,18\n\
        7,14\n\
        5,13\n\
        19,18\n\
        9,5\n\
        12,3\n\
        9,15\n\
        19,\n\
        11,\n\
        12,\n\
    ";
    // The ragged lengths result in 3 NaNs that get filtered out in the second column

    let expected = "\
Stats for column \"rolls-session-1\":
    count: 24
    Q1: 3.5
    median: 10
    Q3: 13
    min: 2 at index: 0
    max: 20 at index: 23
    mean: 9.791666666666668
    stddev: 6.476315746532907

Stats for column \"rolls-session-2\":
    count: 21
    filtered: 3 (total: 24)
    Q1: 5
    median: 8
    Q3: 14
    min: 2 at index: 0
    max: 18 at index: 19
    mean: 9.571428571428571
    stddev: 5.401964631566671\n\n\
    ";

    let mut cmd = csvstats();
    cmd.arg("--column")
        .arg("rolls-session-1")
        .arg("--column")
        .arg("rolls-session-2");
    let output = cmd.write_stdin(input).captured_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, expected);
}
