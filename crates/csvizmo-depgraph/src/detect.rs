use clap::ValueEnum;

use crate::InputFormat;

/// Detect input format from content heuristics.
///
/// Returns `None` if no format matches. Variants are tried in enum declaration
/// order (most specific first).
pub fn detect(input: &str) -> Option<InputFormat> {
    InputFormat::value_variants()
        .iter()
        .find(|fmt| fmt.matches_content(input))
        .copied()
}

impl InputFormat {
    /// Content-based detection heuristic. The match is exhaustive so adding a
    /// new variant without a detection rule is a compile error.
    fn matches_content(&self, input: &str) -> bool {
        match self {
            Self::CargoMetadata => is_json(input),
            Self::Mermaid => is_mermaid(input),
            Self::Dot => is_dot(input),
            Self::Tgf => is_tgf(input),
            Self::Depfile => is_depfile(input),
            Self::CargoTree => is_cargo_tree(input),
            Self::Tree => is_tree(input),
            Self::Pathlist => is_pathlist(input),
        }
    }
}

/// First non-blank line starts with `{`.
fn is_json(input: &str) -> bool {
    // TODO: This will need some attention if we add more JSON formats (like conan)
    first_nonblank(input).starts_with('{')
}

/// First non-blank line starts with `flowchart`, or `graph` followed by a
/// direction keyword (`TD`/`TB`/`BT`/`LR`/`RL`).
fn is_mermaid(input: &str) -> bool {
    let first = first_nonblank(input);
    first.starts_with("flowchart")
        || (first.starts_with("graph")
            && matches!(
                first.split_whitespace().nth(1),
                Some("TD" | "TB" | "BT" | "LR" | "RL")
            ))
}

/// First non-blank line starts with `digraph`, `strict graph`, or `graph`.
///
/// Note: `graph` + direction keyword is matched as Mermaid first (higher
/// priority in enum order), so only `graph` + identifier reaches here.
fn is_dot(input: &str) -> bool {
    let first = first_nonblank(input);
    first.starts_with("digraph") || first.starts_with("strict graph") || first.starts_with("graph")
}

/// Any line is exactly `#` (TGF node/edge separator).
fn is_tgf(input: &str) -> bool {
    input.lines().any(|l| l.trim() == "#")
}

/// Any line matches the `target: dep` pattern of a makefile depfile.
fn is_depfile(input: &str) -> bool {
    input.lines().any(|line| {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return false;
        }
        let line = line.strip_suffix('\\').unwrap_or(line).trim();
        match line.find(':') {
            Some(colon) => {
                let target = &line[..colon];
                let deps = line[colon + 1..].trim();
                !target.is_empty()
                    && !target.contains(char::is_whitespace)
                    && !deps.is_empty()
                    && !deps.starts_with(':')
            }
            None => false,
        }
    })
}

/// Tree-drawing decorations + version tokens (e.g. `v1.2.3`).
fn is_cargo_tree(input: &str) -> bool {
    has_tree_drawing(input) && has_version_pattern(input)
}

/// Tree-drawing decorations without version tokens.
///
/// Note: `is_cargo_tree` has higher priority, so if versions are present
/// the input won't reach this check.
fn is_tree(input: &str) -> bool {
    has_tree_drawing(input)
}

/// Every non-blank line contains a `/` path separator.
fn is_pathlist(input: &str) -> bool {
    let mut any = false;
    for line in input.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if !line.contains('/') {
            return false;
        }
        any = true;
    }
    any
}

/// Any line contains tree-drawing characters — Unicode box-drawing or ASCII
/// equivalents (`tree --charset=ascii`, `scons --tree`).
fn has_tree_drawing(input: &str) -> bool {
    input.lines().any(|l| {
        l.contains('├')
            || l.contains('└')
            || l.contains('│')
            || l.contains("|--")
            || l.contains("`--")
            || l.contains("\\--")
            || l.contains("+-")
    })
}

/// Any whitespace-delimited token looks like a version: `v` followed by a digit.
fn has_version_pattern(input: &str) -> bool {
    input.lines().any(|l| {
        l.split_whitespace().any(|tok| {
            // TODO: This is pretty loose
            tok.starts_with('v')
                && tok.len() > 1
                && tok[1..].starts_with(|c: char| c.is_ascii_digit())
        })
    })
}

fn first_nonblank(input: &str) -> &str {
    // TODO: Should this strip out comments?
    input
        .lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim())
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_cargo_metadata() {
        let input = "{\n  \"packages\": [\n";
        assert_eq!(detect(input), Some(InputFormat::CargoMetadata));
    }

    #[test]
    fn detect_mermaid_flowchart() {
        let input = include_str!("../../../data/depconv/flowchart.mmd");
        assert_eq!(detect(input), Some(InputFormat::Mermaid));
    }

    #[test]
    fn detect_mermaid_subgraph() {
        let input = include_str!("../../../data/depconv/subgraph.mmd");
        assert_eq!(detect(input), Some(InputFormat::Mermaid));
    }

    #[test]
    fn detect_mermaid_graph_with_direction() {
        assert_eq!(detect("graph TD\n  A --> B\n"), Some(InputFormat::Mermaid));
        assert_eq!(detect("graph LR\n  A --> B\n"), Some(InputFormat::Mermaid));
        assert_eq!(detect("graph RL\n  A --> B\n"), Some(InputFormat::Mermaid));
        assert_eq!(detect("graph BT\n  A --> B\n"), Some(InputFormat::Mermaid));
        assert_eq!(detect("graph TB\n  A --> B\n"), Some(InputFormat::Mermaid));
    }

    #[test]
    fn detect_dot() {
        let input = include_str!("../../../data/depconv/small.dot");
        assert_eq!(detect(input), Some(InputFormat::Dot));
    }

    #[test]
    fn detect_dot_graph_with_id() {
        assert_eq!(
            detect("graph deps {\n  a -- b;\n}\n"),
            Some(InputFormat::Dot)
        );
    }

    #[test]
    fn detect_dot_strict() {
        assert_eq!(
            detect("strict graph {\n  a -- b;\n}\n"),
            Some(InputFormat::Dot)
        );
    }

    #[test]
    fn detect_tgf() {
        let input = include_str!("../../../data/depconv/small.tgf");
        assert_eq!(detect(input), Some(InputFormat::Tgf));
    }

    #[test]
    fn detect_depfile() {
        let input = include_str!("../../../data/depconv/small.d");
        assert_eq!(detect(input), Some(InputFormat::Depfile));
    }

    #[test]
    fn detect_cargo_tree() {
        let input = include_str!("../../../data/depconv/cargo-tree.txt");
        assert_eq!(detect(input), Some(InputFormat::CargoTree));
    }

    #[test]
    fn detect_cargo_tree_ascii() {
        let input = "+-myapp v1.0.0\n  +-libfoo v0.2.1\n  | +-libbar v0.1.0\n";
        assert_eq!(detect(input), Some(InputFormat::CargoTree));
    }

    #[test]
    fn detect_tree() {
        let input = include_str!("../../../data/depconv/tree.txt");
        assert_eq!(detect(input), Some(InputFormat::Tree));
    }

    #[test]
    fn detect_tree_ascii() {
        let input = include_str!("../../../data/depconv/tree-ascii.txt");
        assert_eq!(detect(input), Some(InputFormat::Tree));
    }

    #[test]
    fn detect_tree_ascii_backslash() {
        let input = "|-- dir1\n|   |-- file1\n|   \\-- file2\n\\-- dir2\n";
        assert_eq!(detect(input), Some(InputFormat::Tree));
    }

    #[test]
    fn detect_pathlist_gitfiles() {
        let input = include_str!("../../../data/depconv/gitfiles.txt");
        assert_eq!(detect(input), Some(InputFormat::Pathlist));
    }

    #[test]
    fn detect_pathlist_find() {
        let input = include_str!("../../../data/depconv/find.txt");
        assert_eq!(detect(input), Some(InputFormat::Pathlist));
    }

    #[test]
    fn detect_plain_names_not_pathlist() {
        assert_eq!(detect("foo\nbar\nbaz\n"), None);
    }

    #[test]
    fn detect_empty() {
        assert_eq!(detect(""), None);
    }
}
