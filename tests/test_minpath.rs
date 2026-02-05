use pretty_assertions::assert_eq;

use crate::{CommandExt, tool};

#[test]
fn empty_input() {
    let output = tool("minpath").captured_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert_eq!(stdout, "");
}

#[test]
fn single_path_from_stdin() {
    let input = "/home/user/project/src/main.rs\n";
    let output = tool("minpath")
        .write_stdin(input)
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert_eq!(stdout, "main.rs\n");
}

#[test]
fn single_path_from_args() {
    let output = tool("minpath")
        .arg("/home/user/project/src/main.rs")
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert_eq!(stdout, "main.rs\n");
}

#[test]
fn mix_stdin_and_args() {
    let input = "/home/user/from_stdin.rs\n";
    let output = tool("minpath")
        .arg("/home/user/from_args.rs")
        .write_stdin(input)
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    // When args are provided, stdin should be ignored
    assert_eq!(stdout, "from_args.rs\n");

    // Unless '-' is given as an argument
    let output = tool("minpath")
        .arg("/home/user/from_args.rs")
        .arg("-")
        .write_stdin(input)
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert_eq!(stdout, "from_args.rs\nfrom_stdin.rs\n");
}

#[test]
fn multiple_paths_no_duplicates() {
    let input = "/home/user/project/src/main.rs\n/home/user/project/src/util.rs\n";
    let output = tool("minpath")
        .write_stdin(input)
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert_eq!(stdout, "main.rs\nutil.rs\n");
}

#[test]
fn duplicate_filenames_minimal_unique() {
    let input = "/home/user/project/src/utils/parse.rs\n/home/user/project/tests/utils/parse.rs\n";
    let output = tool("minpath")
        .write_stdin(input)
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert_eq!(stdout, "src/utils/parse.rs\ntests/utils/parse.rs\n");
}

#[test]
fn no_minimal_suffix() {
    let input = "/home/user/project/src/utils/parse.rs\n/home/user/project/tests/utils/parse.rs\n";
    let output = tool("minpath")
        .arg("--no-minimal-suffix")
        .write_stdin(input)
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    // Without minimal suffix, both should just show the path with common prefix removed
    assert_eq!(stdout, "src/utils/parse.rs\ntests/utils/parse.rs\n");
}

#[test]
fn prefix_removal() {
    let input = "/home/user/project/src/main.rs\n/home/user/project/lib/util.rs\n";
    let output = tool("minpath")
        .arg("--no-minimal-suffix")
        .arg("--prefix")
        .arg("/home/user/")
        .write_stdin(input)
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert_eq!(stdout, "src/main.rs\nlib/util.rs\n");
}

#[test]
fn relative_to_base() {
    let input = "/home/user/project/src/main.rs\n/home/user/project/lib/util.rs\n";
    let output = tool("minpath")
        .arg("--no-minimal-suffix")
        .arg("--relative-to")
        .arg("/home/user/project")
        .write_stdin(input)
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert_eq!(stdout, "src/main.rs\nlib/util.rs\n");
}

#[test]
fn single_letter_abbreviation() {
    let input = "/home/user/project/src/utils/parse.rs\n/home/user/project/tests/utils/parse.rs";
    let output = tool("minpath")
        .arg("--single-letter")
        .write_stdin(input)
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert_eq!(stdout, "s/u/parse.rs\nt/u/parse.rs\n");
}

#[test]
fn smart_abbreviation() {
    let input = "/home/user/Documents/project/Source/main.rs\n";
    let output = tool("minpath")
        .arg("--no-minimal-suffix")
        .arg("--smart-abbreviate")
        .write_stdin(input)
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert_eq!(stdout, "~/docs/project/src/main.rs\n");
}

#[test]
fn sort_and_unique() {
    let input = "/home/user/b.rs\n/home/user/a.rs\n/home/user/b.rs\n";
    let output = tool("minpath")
        .arg("--sort")
        .write_stdin(input)
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    // Sorted with duplicates removed
    assert_eq!(stdout, "a.rs\nb.rs\n");
}

#[test]
fn preserve_input_order() {
    let input = "/home/user/c.rs\n/home/user/b.rs\n/home/user/a.rs\n/home/user/b.rs\n";
    let output = tool("minpath")
        .write_stdin(input)
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    // Unsorted, duplicates removed (first occurrence kept)
    assert_eq!(stdout, "c.rs\nb.rs\na.rs\n");
}

#[test]
fn select_specific_paths() {
    let input = "/home/user/src/main.rs\n/home/user/tests/test.rs\n/home/user/lib/util.rs\n";
    let output = tool("minpath")
        .arg("--select")
        .arg("**/src/**")
        .arg("--select")
        .arg("**/lib/**")
        .write_stdin(input)
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    // Test select behavior, not exact transform output
    assert!(stdout.contains("main.rs"));
    assert!(stdout.contains("util.rs"));
    assert!(!stdout.contains("test.rs"));
    assert_eq!(stdout.lines().count(), 2);
}

#[test]
fn exclude_patterns() {
    let input = "/home/user/src/main.rs\n/home/user/tests/test.rs\n/home/user/lib/util.rs\n";
    let output = tool("minpath")
        .arg("--exclude")
        .arg("**/tests/**")
        .write_stdin(input)
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    // Test exclude behavior, not exact transform output
    assert!(stdout.contains("main.rs"));
    assert!(stdout.contains("util.rs"));
    assert!(!stdout.contains("test.rs"));
    assert_eq!(stdout.lines().count(), 2);
}
