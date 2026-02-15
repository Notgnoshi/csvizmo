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
fn tgf_to_dot() {
    let input = include_str!("../../../data/depconv/small.tgf");
    let output = tool!("depconv")
        .args(["--input-format", "tgf", "--output-format", "dot"])
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
        .args(["--input-format", "dot", "--output-format", "tgf"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "libbar\nlibfoo\nmyapp\tMy Application\n#\nmyapp\tlibfoo\nmyapp\tlibbar\nlibfoo\tlibbar\n"
    );
}

#[cfg(feature = "dot")]
#[test]
fn tgf_to_dot_to_tgf_roundtrip() {
    let input = "a\tAlpha\nb\tBravo\n#\na\tb\tuses\n";
    // TGF → DOT
    let dot_output = tool!("depconv")
        .args(["--input-format", "tgf", "--output-format", "dot"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(dot_output.status.success());
    let dot = String::from_utf8_lossy(&dot_output.stdout);
    // DOT → TGF
    let tgf_output = tool!("depconv")
        .args(["--input-format", "dot", "--output-format", "tgf"])
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
        .args(["--input-format", "depfile", "--output-format", "dot"])
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
        .args(["--input-format", "depfile", "--output-format", "tgf"])
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
        .args(["--output-format", "tgf"])
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
        .args(["--output-format", "tgf", "-i", fixture])
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
        .args(["--input-format", "depfile", "--output-format", "dot"])
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
        .args(["--input-format", "depfile", "--output-format", "depfile"])
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
        .args(["--input-format", "tgf", "--output-format", "depfile"])
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
        .args(["--input-format", "pathlist", "--output-format", "dot"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    src;
    \"src/a.rs\" [label=\"a.rs\"];
    \"src/b.rs\" [label=\"b.rs\"];
    \"README.md\";
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
        .args(["--output-format", "tgf"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "src\nsrc/main.rs\tmain.rs\nsrc/lib.rs\tlib.rs\n#\nsrc\tsrc/main.rs\nsrc\tsrc/lib.rs\n"
    );
}

#[test]
fn tree_to_dot() {
    let input = "root\n├── a\n│   └── b\n└── c\n";
    let output = tool!("depconv")
        .args(["--input-format", "tree", "--output-format", "dot"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    root;
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
        .args(["--output-format", "tgf"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "root\nroot/child\tchild\n#\nroot\troot/child\n");
}

#[test]
fn tgf_to_tree() {
    // a -> b -> c, a -> d (diamond-like with branching at root)
    let input = "a\tAlpha\nb\tBravo\nc\tCharlie\nd\tDelta\n#\na\tb\na\td\nb\tc\n";
    let output = tool!("depconv")
        .args(["--input-format", "tgf", "--output-format", "tree"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
Alpha
├── Bravo
│   └── Charlie
└── Delta
"
    );
}

#[test]
fn pathlist_roundtrip() {
    let input = "src/a.rs\nsrc/b.rs\nREADME.md\n";
    let output = tool!("depconv")
        .args(["--input-format", "pathlist", "--output-format", "pathlist"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, input);
}

#[test]
fn tgf_to_pathlist() {
    // a -> b -> c, a -> c (diamond: c is shared)
    let input = "a\tAlpha\nb\tBravo\nc\tCharlie\n#\na\tb\na\tc\nb\tc\n";
    let output = tool!("depconv")
        .args(["--input-format", "tgf", "--output-format", "pathlist"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // DFS: Alpha -> Bravo -> Charlie (leaf), Alpha -> Charlie (leaf again, no subtree suppressed)
    assert_eq!(stdout, "Alpha/Bravo/Charlie\nAlpha/Charlie\n");
}

#[test]
fn pathlist_to_pathlist_fixture() {
    let input = include_str!("../../../data/depconv/gitfiles.txt");
    let output = tool!("depconv")
        .args(["--input-format", "pathlist", "--output-format", "pathlist"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, input);
}

#[cfg(feature = "dot")]
#[test]
fn dot_to_dot() {
    let input = include_str!("../../../data/depconv/small.dot");
    let output = tool!("depconv")
        .args(["--input-format", "dot", "--output-format", "dot"])
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
    libbar;
    libfoo;
    myapp [label=\"My Application\", shape=\"box\"];
    myapp -> libfoo;
    myapp -> libbar;
    libfoo -> libbar;
}
"
    );
}

#[cfg(feature = "dot")]
#[test]
fn dot_subgraph_to_depfile() {
    let input = "\
digraph {
    top -> a;
    subgraph cluster0 {
        a -> b;
        b -> c;
    }
}
";
    let output = tool!("depconv")
        .args(["--input-format", "dot", "--output-format", "depfile"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "top: a\na: b\nb: c\n");
}

#[cfg(feature = "dot")]
#[test]
fn cmake_dot_preserves_subgraph() {
    let input = include_str!("../../../data/depconv/cmake.geos.dot");

    // Parse -> emit -> re-parse.
    let output1 = tool!("depconv")
        .args(["--input-format", "dot", "--output-format", "dot"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output1.status.success());
    let dot1 = String::from_utf8_lossy(&output1.stdout);

    let output2 = tool!("depconv")
        .args(["--input-format", "dot", "--output-format", "dot"])
        .write_stdin(dot1.as_ref())
        .captured_output()
        .unwrap();
    assert!(output2.status.success());
    let dot2 = String::from_utf8_lossy(&output2.stdout);

    // Round-trip should be stable: emit(parse(emit(parse(input)))) == emit(parse(input)).
    assert_eq!(dot1, dot2);

    // Structural checks on the output: subgraph present, legend nodes inside.
    assert!(
        dot1.contains("subgraph clusterLegend {"),
        "output should contain subgraph header"
    );
    assert!(
        dot1.contains("legendNode0"),
        "legend nodes should be in output"
    );

    // Legend attrs should be inside the subgraph, not at top level.
    // Find the subgraph block and verify label is inside it.
    let sg_start = dot1.find("subgraph clusterLegend {").unwrap();
    let sg_end = dot1[sg_start..].find('}').unwrap() + sg_start;
    let sg_block = &dot1[sg_start..=sg_end];
    assert!(
        sg_block.contains("label=\"Legend\""),
        "legend label should be inside subgraph block"
    );

    // Top-level graph name preserved.
    assert!(
        dot1.starts_with("digraph GEOS {"),
        "graph name GEOS should be preserved"
    );
}

#[test]
fn cargo_tree_to_dot() {
    let input = "\
myapp v1.0.0
├── libfoo v0.2.1
│   └── shared v1.0.0
└── libbar v0.1.0 (proc-macro)
    └── shared v1.0.0 (*)
";
    let output = tool!("depconv")
        .args(["--input-format", "cargo-tree", "--output-format", "dot"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    \"myapp v1.0.0\" [label=\"myapp\", version=\"v1.0.0\"];
    \"libfoo v0.2.1\" [label=\"libfoo\", version=\"v0.2.1\"];
    \"shared v1.0.0\" [label=\"shared\", version=\"v1.0.0\"];
    \"libbar v0.1.0\" [label=\"libbar\", type=\"proc-macro\", version=\"v0.1.0\", shape=\"diamond\"];
    \"myapp v1.0.0\" -> \"libfoo v0.2.1\";
    \"libfoo v0.2.1\" -> \"shared v1.0.0\";
    \"myapp v1.0.0\" -> \"libbar v0.1.0\";
    \"libbar v0.1.0\" -> \"shared v1.0.0\";
}
"
    );
}

#[test]
fn cargo_tree_auto_detect() {
    let input = include_str!("../../../data/depconv/cargo-tree.txt");
    let output = tool!("depconv")
        .args(["--output-format", "tgf"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Split TGF into node and edge sections
    let sections: Vec<&str> = stdout.splitn(2, "\n#\n").collect();
    assert_eq!(
        sections.len(),
        2,
        "TGF output should have node and edge sections"
    );
    let node_lines: Vec<&str> = sections[0].lines().collect();
    let edge_lines: Vec<&str> = sections[1].lines().filter(|l| !l.is_empty()).collect();

    // Verify node and edge counts
    assert_eq!(node_lines.len(), 69);
    assert_eq!(edge_lines.len(), 111);

    // Root node should be first (spaces in IDs become underscores in TGF)
    assert!(
        node_lines[0].starts_with("csvizmo-depgraph_v0.5.0\t"),
        "root node should be first, got: {}",
        node_lines[0]
    );

    // A known dependency should appear as a node
    assert!(
        node_lines.iter().any(|l| l.starts_with("clap_v4.5.57\t")),
        "clap should be in node list"
    );

    // A known edge should exist
    assert!(
        edge_lines.contains(&"csvizmo-depgraph_v0.5.0\tclap_v4.5.57"),
        "root -> clap edge should exist"
    );
}

#[test]
fn cargo_metadata_to_dot() {
    // Test with the real cargo-metadata.json fixture
    let input = include_str!("../../../data/depconv/cargo-metadata.json");
    let output = tool!("depconv")
        .args(["--input-format", "cargo-metadata", "--output-format", "dot"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Count nodes and edges for structural verification
    let node_count = stdout.lines().filter(|l| l.contains("[label=")).count();
    let edge_count = stdout.lines().filter(|l| l.contains("->")).count();
    assert_eq!(node_count, 143);
    assert_eq!(edge_count, 292);

    // Verify DOT wrapper
    assert!(stdout.starts_with("digraph {\n"));
    assert!(stdout.ends_with("}\n"));

    // Verify csvizmo-depgraph node with exact attribute line
    assert_eq!(
        stdout
            .lines()
            .find(|l| l.contains("csvizmo-depgraph 0.5.0") && l.contains("[label="))
            .unwrap()
            .trim(),
        "\"csvizmo-depgraph 0.5.0\" [label=\"csvizmo-depgraph\", type=\"lib\", \
         version=\"0.5.0\", features=\"default,dot\", shape=\"ellipse\"];"
    );

    // Verify optional dependencies have exact edge attributes
    assert_eq!(
        stdout
            .lines()
            .find(|l| l.contains("dot-parser 0.6.1") && l.contains("->"))
            .unwrap()
            .trim(),
        "\"csvizmo-depgraph 0.5.0\" -> \"dot-parser 0.6.1\" [kind=\"normal\", optional=\"dot\"];"
    );

    // Verify proc-macro node with exact attribute line
    assert_eq!(
        stdout
            .lines()
            .find(|l| l.contains("clap_derive 4.5.55") && l.contains("[label="))
            .unwrap()
            .trim(),
        "\"clap_derive 4.5.55\" [label=\"clap_derive\", type=\"proc-macro\", \
         version=\"4.5.55\", features=\"default\", shape=\"diamond\"];"
    );

    // Verify dev dependency edge has styling
    assert_eq!(
        stdout
            .lines()
            .find(|l| l.contains("csvizmo-depgraph")
                && l.contains("csvizmo-test")
                && l.contains("->"))
            .unwrap()
            .trim(),
        "\"csvizmo-depgraph 0.5.0\" -> \"csvizmo-test 0.5.0\" \
         [kind=\"dev\", style=\"dashed\", color=\"gray60\"];"
    );

    // Verify regular dependency edge (no optional, no styling)
    assert_eq!(
        stdout
            .lines()
            .find(|l| l.contains("csvizmo-depgraph") && l.contains("-> \"eyre"))
            .unwrap()
            .trim(),
        "\"csvizmo-depgraph 0.5.0\" -> \"eyre 0.6.12\" [kind=\"normal\"];"
    );
}

#[cfg(feature = "dot")]
#[test]
fn dot_roundtrip_with_type() {
    let input = r#"digraph {
    a [label="A", type="lib"];
    b [label="B", type="proc-macro"];
    a -> b;
}
"#;

    let expected = "\
digraph {
    a [label=\"A\", type=\"lib\", shape=\"ellipse\"];
    b [label=\"B\", type=\"proc-macro\", shape=\"diamond\"];
    a -> b;
}
";

    // Parse DOT -> emit DOT
    let output1 = tool!("depconv")
        .args(["--input-format", "dot", "--output-format", "dot"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output1.status.success());
    let stdout1 = String::from_utf8_lossy(&output1.stdout);
    assert_eq!(stdout1, expected);

    // Parse again to verify round-trip stability
    let output2 = tool!("depconv")
        .args(["--input-format", "dot", "--output-format", "dot"])
        .write_stdin(stdout1.as_ref())
        .captured_output()
        .unwrap();
    assert!(output2.status.success());
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert_eq!(stdout2, expected);
}

#[test]
fn tgf_to_mermaid() {
    let input = "a\talpha\nb\tbravo\nc\n#\na\tb\tdepends\nb\tc\na\tc\n";
    let output = tool!("depconv")
        .args(["--input-format", "tgf", "--output-format", "mermaid"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
flowchart LR
    a[\"alpha\"]
    b[\"bravo\"]
    c[\"c\"]
    a -->|\"depends\"| b
    b --> c
    a --> c
"
    );
}

#[cfg(feature = "dot")]
#[test]
fn mermaid_node_types() {
    let input = r#"digraph {
    lib1 [label="Library", type="lib"];
    bin1 [label="Binary", type="bin"];
    pm1 [label="Proc Macro", type="proc-macro"];
    bs1 [label="Build Script", type="build-script"];
    test1 [label="Test", type="test"];
    lib1 -> bin1;
}
"#;
    let output = tool!("depconv")
        .args(["--input-format", "dot", "--output-format", "mermaid"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
flowchart LR
    bin1[\"Binary\"]
    bs1[/\"Build Script\"/]
    lib1([\"Library\"])
    pm1{\"Proc Macro\"}
    test1{{\"Test\"}}
    lib1 --> bin1
"
    );
}

#[cfg(feature = "dot")]
#[test]
fn dot_to_mermaid_with_subgraphs() {
    let input = r#"digraph deps {
    rankdir=TB;
    subgraph cluster_backend {
        label="Backend";
        api [label="API Server"];
        db [label="Database"];
    }
    subgraph cluster_frontend {
        label="Frontend";
        web [label="Web App"];
        mobile [label="Mobile App"];
    }
    web -> api;
    mobile -> api;
    api -> db [label="queries"];
}
"#;
    let output = tool!("depconv")
        .args(["--input-format", "dot", "--output-format", "mermaid"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Verify flowchart direction from rankdir
    assert!(stdout.starts_with("flowchart TB\n"));
    // Verify subgraphs are preserved
    assert!(stdout.contains("subgraph cluster_backend"));
    assert!(stdout.contains("subgraph cluster_frontend"));
    // Verify nodes are in subgraphs
    assert!(stdout.contains("api[\"API Server\"]"));
    assert!(stdout.contains("db[\"Database\"]"));
    assert!(stdout.contains("web[\"Web App\"]"));
    assert!(stdout.contains("mobile[\"Mobile App\"]"));
    // Verify edge labels
    assert!(stdout.contains("api -->|\"queries\"| db"));
}

#[test]
fn mermaid_special_chars() {
    let input = "a\tLabel [with] \"quotes\"\nb\tOther{label}\n#\na\tb\tuses|pipes\n";
    let output = tool!("depconv")
        .args(["--input-format", "tgf", "--output-format", "mermaid"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
flowchart LR
    a[\"Label [with] &quot;quotes&quot;\"]
    b[\"Other{label}\"]
    a -->|\"uses|pipes\"| b
"
    );
}

#[test]
fn depfile_to_mermaid() {
    let input = "main.o: main.c config.h\n";
    let output = tool!("depconv")
        .args(["--input-format", "depfile", "--output-format", "mermaid"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
flowchart LR
    main.o[\"main.o\"]
    main.c[\"main.c\"]
    config.h[\"config.h\"]
    main.o --> main.c
    main.o --> config.h
"
    );
}

#[test]
fn mermaid_to_tgf() {
    let input = "flowchart LR\n    A[myapp] --> B[libfoo]\n    A --> C[libbar]\n    B --> C\n";
    let output = tool!("depconv")
        .args(["--input-format", "mermaid", "--output-format", "tgf"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "A\tmyapp\nB\tlibfoo\nC\tlibbar\n#\nA\tB\nA\tC\nB\tC\n"
    );
}

#[test]
fn mermaid_to_dot() {
    let input = "flowchart LR\n    A[myapp] --> B[libfoo]\n    A --> C[libbar]\n    B --> C\n";
    let output = tool!("depconv")
        .args(["--input-format", "mermaid", "--output-format", "dot"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    direction=\"LR\";
    A [label=\"myapp\"];
    B [label=\"libfoo\"];
    C [label=\"libbar\"];
    A -> B;
    A -> C;
    B -> C;
}
"
    );
}

#[test]
fn mermaid_auto_detect() {
    let input = "flowchart LR\n    A[myapp] --> B[libfoo]\n";
    let output = tool!("depconv")
        .args(["--output-format", "tgf"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "A\tmyapp\nB\tlibfoo\n#\nA\tB\n");
}

#[test]
fn mermaid_roundtrip() {
    let input = "flowchart LR\n    A[myapp] --> B[libfoo]\n    A --> C[libbar]\n    B --> C\n";
    // parse mermaid -> emit mermaid
    let output1 = tool!("depconv")
        .args(["--input-format", "mermaid", "--output-format", "mermaid"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output1.status.success());
    let mmd1 = String::from_utf8_lossy(&output1.stdout);

    // parse again -> emit again, should be stable
    let output2 = tool!("depconv")
        .args(["--input-format", "mermaid", "--output-format", "mermaid"])
        .write_stdin(mmd1.as_ref())
        .captured_output()
        .unwrap();
    assert!(output2.status.success());
    let mmd2 = String::from_utf8_lossy(&output2.stdout);

    assert_eq!(mmd1, mmd2);
}

#[test]
fn mermaid_subgraph_to_dot() {
    let input = include_str!("../../../data/depconv/subgraph.mmd");
    let output = tool!("depconv")
        .args(["--input-format", "mermaid", "--output-format", "dot"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "\
digraph {
    direction=\"TD\";
    subgraph cluster_backend {
        label=\"backend\";
        api [label=\"API Server\"];
        db [label=\"Database\"];
        cache [label=\"Redis Cache\"];
    }
    subgraph cluster_frontend {
        label=\"frontend\";
        web [label=\"Web App\"];
        mobile [label=\"Mobile App\"];
    }
    web -> api;
    mobile -> api;
    api -> db;
    api -> cache;
}
"
    );
}

#[test]
fn mermaid_edge_labels_to_tgf() {
    let input = include_str!("../../../data/depconv/flowchart.mmd");
    let output = tool!("depconv")
        .args(["--input-format", "mermaid", "--output-format", "tgf"])
        .write_stdin(input)
        .captured_output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "A\tmyapp\nB\tlibfoo\nC\tlibbar\n#\nA\tB\tstatic\nA\tC\tdynamic\nB\tC\n"
    );
}
