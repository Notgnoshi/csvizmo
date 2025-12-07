use pretty_assertions::assert_eq;

use crate::{CommandExt, tool};

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
        filename,colname,count,filtered,min,min-index,max,max-index,mean,stddev,Q1,median,Q3\n\
        \"stdin\",\"rolls-session-1\",24,0,2,0,20,4,9.791666666666664,6.93178623865251,3.5,10,13\n\
        \"stdin\",\"rolls-session-2\",21,3,2,5,18,14,9.571428571428573,5.803823252952202,5,8,14\n\
    ";

    let mut cmd = tool("csvstats");
    cmd.arg("--column")
        .arg("rolls-session-1")
        .arg("--column")
        .arg("rolls-session-2");
    let output = cmd.write_stdin(input).captured_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, expected);
}
