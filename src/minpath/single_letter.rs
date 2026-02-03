use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::minpath::transform::GlobalTransform;

pub struct SingleLetter;

impl SingleLetter {
    fn transform_one(&self, input: &Path) -> PathBuf {
        let components: Vec<_> = input.iter().collect();
        if components.is_empty() {
            return PathBuf::new();
        }

        let last = components.len() - 1;
        components
            .into_iter()
            .enumerate()
            .map(|(i, c)| {
                if i < last {
                    // Abbreviate directory to first character
                    let first = c.to_string_lossy().chars().next().unwrap_or_default();
                    OsString::from(first.to_string())
                } else {
                    // Keep filename as-is
                    c.to_os_string()
                }
            })
            .collect()
    }
}

impl GlobalTransform for SingleLetter {
    fn transform(&self, inputs: &[PathBuf]) -> Vec<PathBuf> {
        inputs.iter().map(|p| self.transform_one(p)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abbreviates_directories() {
        let tr = SingleLetter;
        assert_eq!(
            tr.transform_one(Path::new("src/utils/parse.rs")),
            PathBuf::from("s/u/parse.rs")
        );
    }

    #[test]
    fn preserves_filename() {
        let tr = SingleLetter;
        assert_eq!(
            tr.transform_one(Path::new("src/main.rs")),
            PathBuf::from("s/main.rs")
        );
    }

    #[test]
    fn single_component_unchanged() {
        let tr = SingleLetter;
        assert_eq!(
            tr.transform_one(Path::new("main.rs")),
            PathBuf::from("main.rs")
        );
    }

    #[test]
    fn handles_absolute_path() {
        let tr = SingleLetter;
        assert_eq!(
            tr.transform_one(Path::new("/home/user/src/main.rs")),
            PathBuf::from("/h/u/s/main.rs")
        );
    }
}
