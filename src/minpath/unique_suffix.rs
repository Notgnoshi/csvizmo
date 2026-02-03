use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::minpath::transform::GlobalTransform;

pub struct MinimalUniqueSuffix;

impl MinimalUniqueSuffix {
    /// Returns the last `n` components of a path as a borrowed slice.
    fn suffix(path: &Path, n: usize) -> &Path {
        let total = path.components().count();
        if n >= total {
            return path;
        }

        let prefix: PathBuf = path.components().take(total - n).collect();
        path.strip_prefix(&prefix).unwrap_or(path)
    }
}

impl GlobalTransform for MinimalUniqueSuffix {
    // Start with the filename only, and extend the suffix component-by-component until there are
    // no collisions.
    fn transform(&self, inputs: &[PathBuf]) -> Vec<PathBuf> {
        if inputs.is_empty() {
            return vec![];
        }

        // Track how many components from the end each path needs
        let mut suffix_len: Vec<usize> = vec![1; inputs.len()];
        let max_components: Vec<usize> = inputs.iter().map(|p| p.components().count()).collect();

        loop {
            // Group paths by their current suffix
            let mut groups: HashMap<&Path, Vec<usize>> = HashMap::new();
            for (i, path) in inputs.iter().enumerate() {
                let suffix = Self::suffix(path, suffix_len[i]);
                groups.entry(suffix).or_default().push(i);
            }

            // Extend suffix for any paths that collide
            let mut had_collision = false;
            for indices in groups.into_values() {
                if indices.len() > 1 {
                    for i in indices {
                        if suffix_len[i] < max_components[i] {
                            suffix_len[i] += 1;
                            had_collision = true;
                        }
                    }
                }
            }

            if !had_collision {
                break;
            }
        }

        inputs
            .iter()
            .enumerate()
            .map(|(i, path)| Self::suffix(path, suffix_len[i]).to_path_buf())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_paths_reduce_to_filename() {
        let tr = MinimalUniqueSuffix;

        // Single path
        assert_eq!(
            tr.transform(&[PathBuf::from("src/main.rs")]),
            [PathBuf::from("main.rs")]
        );

        // Multiple unique paths
        assert_eq!(
            tr.transform(&[
                PathBuf::from("src/main.rs"),
                PathBuf::from("src/lib.rs"),
                PathBuf::from("tests/test.rs"),
            ]),
            [
                PathBuf::from("main.rs"),
                PathBuf::from("lib.rs"),
                PathBuf::from("test.rs"),
            ]
        );
    }

    #[test]
    fn collisions_extend_until_unique() {
        let tr = MinimalUniqueSuffix;

        // Simple collision
        assert_eq!(
            tr.transform(&[PathBuf::from("src/main.rs"), PathBuf::from("tests/main.rs"),]),
            [PathBuf::from("src/main.rs"), PathBuf::from("tests/main.rs")]
        );

        // Mixed: collision + unique
        assert_eq!(
            tr.transform(&[
                PathBuf::from("src/main.rs"),
                PathBuf::from("tests/main.rs"),
                PathBuf::from("src/lib.rs"),
            ]),
            [
                PathBuf::from("src/main.rs"),
                PathBuf::from("tests/main.rs"),
                PathBuf::from("lib.rs"),
            ]
        );

        // Deep collision requiring multiple iterations
        assert_eq!(
            tr.transform(&[
                PathBuf::from("a/utils/parse.rs"),
                PathBuf::from("b/utils/parse.rs"),
            ]),
            [
                PathBuf::from("a/utils/parse.rs"),
                PathBuf::from("b/utils/parse.rs"),
            ]
        );

        // Three-way collision
        assert_eq!(
            tr.transform(&[
                PathBuf::from("a/main.rs"),
                PathBuf::from("b/main.rs"),
                PathBuf::from("c/main.rs"),
            ]),
            [
                PathBuf::from("a/main.rs"),
                PathBuf::from("b/main.rs"),
                PathBuf::from("c/main.rs"),
            ]
        );

        // Asymmetric depth: one path needs more extension than the other
        assert_eq!(
            tr.transform(&[PathBuf::from("a/b/c.rs"), PathBuf::from("d/c.rs"),]),
            [PathBuf::from("b/c.rs"), PathBuf::from("d/c.rs")]
        );
    }

    #[test]
    fn identical_paths_stay_full() {
        let tr = MinimalUniqueSuffix;

        // Two identical paths
        assert_eq!(
            tr.transform(&[PathBuf::from("a/b.rs"), PathBuf::from("a/b.rs")]),
            [PathBuf::from("a/b.rs"), PathBuf::from("a/b.rs")]
        );

        // Identical + unique
        assert_eq!(
            tr.transform(&[
                PathBuf::from("a/b.rs"),
                PathBuf::from("a/b.rs"),
                PathBuf::from("c.rs"),
            ]),
            [
                PathBuf::from("a/b.rs"),
                PathBuf::from("a/b.rs"),
                PathBuf::from("c.rs"),
            ]
        );
    }

    #[test]
    fn empty_input() {
        let tr = MinimalUniqueSuffix;
        assert!(tr.transform(&[]).is_empty());
    }
}
