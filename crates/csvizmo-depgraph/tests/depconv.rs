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

#[test]
fn depfile_to_dot() {
    let input = "main.o: main.c config.h\n";
    let output = tool!("depconv")
        .args(["--from", "depfile", "--to", "dot"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    \"main.o\";
    \"main.c\";
    \"config.h\";
    \"main.o\" -> \"main.c\";
    \"main.o\" -> \"config.h\";
}
"
    );
}

#[test]
fn depfile_to_tgf() {
    let input = include_str!("../../../data/depconv/small.d");
    let output = tool!("depconv")
        .args(["--from", "depfile", "--to", "tgf"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        normalize_whitespace(&stdout),
        normalize_whitespace(
            "main.o\nmain.c\nconfig.h\nutils.h\nutils.c\nconfig.o\nconfig.c\nutils.o\n\
             #\n\
             main.o main.c\nmain.o config.h\nmain.o utils.h\nmain.o utils.c\n\
             config.o config.c\nconfig.o config.h\nutils.o utils.c\nutils.o utils.h\n"
        )
    );
}

#[test]
fn depfile_auto_detect_content() {
    let input = "main.o: main.c config.h\n";
    let output = tool!("depconv")
        .args(["--to", "tgf"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        normalize_whitespace(&stdout),
        normalize_whitespace("main.o\nmain.c\nconfig.h\n#\nmain.o main.c\nmain.o config.h\n")
    );
}

#[test]
fn depfile_auto_detect_extension() {
    // Path relative to test CWD
    let fixture = "../../data/depconv/small.d";
    let output = tool!("depconv")
        .args(["--to", "tgf", "-i", fixture])
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        normalize_whitespace(&stdout),
        normalize_whitespace(
            "main.o\nmain.c\nconfig.h\nutils.h\nutils.c\nconfig.o\nconfig.c\nutils.o\n\
             #\n\
             main.o main.c\nmain.o config.h\nmain.o utils.h\nmain.o utils.c\n\
             config.o config.c\nconfig.o config.h\nutils.o utils.c\nutils.o utils.h\n"
        )
    );
}

#[test]
fn depfile_multi_target_fixture() {
    let input = include_str!("../../../data/depconv/multi-target.d");
    let output = tool!("depconv")
        .args(["--from", "depfile", "--to", "dot"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    \"src/main.o\";
    \"src/main.c\";
    \"include/config.h\";
    \"include/utils.h\";
    \"src/config.o\";
    \"src/config.c\";
    \"src/utils.o\";
    \"src/utils.c\";
    \"src/main.o\" -> \"src/main.c\";
    \"src/main.o\" -> \"include/config.h\";
    \"src/main.o\" -> \"include/utils.h\";
    \"src/config.o\" -> \"src/config.c\";
    \"src/config.o\" -> \"include/config.h\";
    \"src/utils.o\" -> \"src/utils.c\";
    \"src/utils.o\" -> \"include/utils.h\";
    \"src/utils.o\" -> \"include/config.h\";
}
"
    );
}

#[test]
fn depfile_roundtrip() {
    let input = "main.o: main.c config.h\nutils.o: utils.c utils.h\n";
    let output = tool!("depconv")
        .args(["--from", "depfile", "--to", "depfile"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, input);
}

#[test]
fn tgf_to_depfile() {
    let input = "3\tmyapp\n1\tlibfoo\n2\tlibbar\n#\n3\t1\n3\t2\n1\t2\n";
    let output = tool!("depconv")
        .args(["--from", "tgf", "--to", "depfile"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "3: 1 2\n1: 2\n");
}

#[test]
fn pathlist_to_dot() {
    let input = "src/a.rs\nsrc/b.rs\nREADME.md\n";
    let output = tool!("depconv")
        .args(["--from", "pathlist", "--to", "dot"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    src [label=\"src\"];
    \"src/a.rs\" [label=\"a.rs\"];
    \"src/b.rs\" [label=\"b.rs\"];
    \"README.md\" [label=\"README.md\"];
    src -> \"src/a.rs\";
    src -> \"src/b.rs\";
}
"
    );
}

#[test]
fn pathlist_auto_detect_content() {
    let input = "src/main.rs\nsrc/lib.rs\n";
    let output = tool!("depconv")
        .args(["--to", "tgf"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "src\tsrc\nsrc/main.rs\tmain.rs\nsrc/lib.rs\tlib.rs\n#\nsrc\tsrc/main.rs\nsrc\tsrc/lib.rs\n"
    );
}

#[test]
fn tree_to_dot() {
    let input = "root\n├── a\n│   └── b\n└── c\n";
    let output = tool!("depconv")
        .args(["--from", "tree", "--to", "dot"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    root [label=\"root\"];
    \"root/a\" [label=\"a\"];
    \"root/a/b\" [label=\"b\"];
    \"root/c\" [label=\"c\"];
    root -> \"root/a\";
    \"root/a\" -> \"root/a/b\";
    root -> \"root/c\";
}
"
    );
}

#[test]
fn tree_auto_detect_content() {
    let input = "root\n├── child\n";
    let output = tool!("depconv")
        .args(["--to", "tgf"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "root\troot\nroot/child\tchild\n#\nroot\troot/child\n"
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
