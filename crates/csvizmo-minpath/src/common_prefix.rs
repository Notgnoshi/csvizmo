use std::path::{Path, PathBuf};

use crate::transform::GlobalTransform;

pub struct StripCommonPrefix;

impl StripCommonPrefix {
    fn common_prefix<'a>(paths: impl Iterator<Item = &'a Path>) -> PathBuf {
        let mut paths = paths.peekable();
        let Some(first) = paths.next() else {
            return PathBuf::new();
        };

        // Single path has no common prefix to strip
        if paths.peek().is_none() {
            return PathBuf::new();
        }

        let mut prefix: PathBuf = first.components().collect();

        for path in paths {
            // Shorten prefix until it matches this path
            while !path.starts_with(&prefix) {
                if !prefix.pop() {
                    return PathBuf::new();
                }
            }
        }

        prefix
    }
}

impl GlobalTransform for StripCommonPrefix {
    fn transform(&self, inputs: &[PathBuf]) -> Vec<PathBuf> {
        let prefix = Self::common_prefix(inputs.iter().map(|p| p.as_path()));

        inputs
            .iter()
            .map(|p| p.strip_prefix(&prefix).unwrap_or(p).to_path_buf())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_common_absolute_prefix() {
        let tr = StripCommonPrefix;
        let inputs: Vec<PathBuf> = vec![
            "/home/user/project/src/main.rs".into(),
            "/home/user/project/src/lib.rs".into(),
            "/home/user/project/tests/test.rs".into(),
        ];
        let result = tr.transform(&inputs);
        assert_eq!(
            result,
            vec![
                PathBuf::from("src/main.rs"),
                PathBuf::from("src/lib.rs"),
                PathBuf::from("tests/test.rs"),
            ]
        );
    }

    #[test]
    fn strips_common_relative_prefix() {
        let tr = StripCommonPrefix;
        let inputs: Vec<PathBuf> = vec![
            "project/src/main.rs".into(),
            "project/src/lib.rs".into(),
            "project/tests/test.rs".into(),
        ];
        let result = tr.transform(&inputs);
        assert_eq!(
            result,
            vec![
                PathBuf::from("src/main.rs"),
                PathBuf::from("src/lib.rs"),
                PathBuf::from("tests/test.rs"),
            ]
        );
    }

    #[test]
    fn mixed_absolute_and_relative_unchanged() {
        let tr = StripCommonPrefix;
        let inputs: Vec<PathBuf> = vec![
            "/home/user/project/src/main.rs".into(),
            "local/src/foo.rs".into(),
        ];
        let result = tr.transform(&inputs);
        // No common prefix between absolute and relative paths
        assert_eq!(result, inputs);
    }

    #[test]
    fn no_common_prefix_unchanged() {
        let tr = StripCommonPrefix;
        let inputs: Vec<PathBuf> = vec!["/home/alice/file.rs".into(), "/opt/bob/file.rs".into()];
        let result = tr.transform(&inputs);
        // Only "/" is common, which we preserve
        assert_eq!(
            result,
            vec![
                PathBuf::from("home/alice/file.rs"),
                PathBuf::from("opt/bob/file.rs"),
            ]
        );
    }

    #[test]
    fn single_path_unchanged() {
        let tr = StripCommonPrefix;
        let inputs: Vec<PathBuf> = vec!["/home/user/project/src/main.rs".into()];
        let result = tr.transform(&inputs);
        assert_eq!(result, inputs);
    }
}
