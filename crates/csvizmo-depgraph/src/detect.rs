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
            Self::Tgf => input.lines().any(|l| l.trim() == "#"),
            Self::Dot => false,
            Self::Mermaid => false,
            Self::Depfile => false,
            Self::CargoMetadata => false,
            Self::CargoTree => false,
            Self::Tree => false,
            Self::Pathlist => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_tgf() {
        let input = include_str!("../../../data/depconv/small.tgf");
        assert_eq!(detect(input), Some(InputFormat::Tgf));
    }

    #[test]
    fn detect_empty() {
        assert_eq!(detect(""), None);
    }
}
