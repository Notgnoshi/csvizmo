use std::path::{Path, PathBuf};

use crate::minpath::transform::LocalTransform;

pub struct StripPrefix {
    prefixes: Vec<PathBuf>,
}

impl StripPrefix {
    pub fn new(prefixes: Vec<PathBuf>) -> Self {
        Self { prefixes }
    }
}

impl LocalTransform for StripPrefix {
    fn transform(&self, input: &Path) -> PathBuf {
        for prefix in &self.prefixes {
            if let Ok(stripped) = input.strip_prefix(prefix) {
                return stripped.to_path_buf();
            }
        }
        input.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_matching_prefix() {
        let tr = StripPrefix::new(vec![PathBuf::from("/home/user")]);
        let result = tr.transform(Path::new("/home/user/project/src/main.rs"));
        assert_eq!(result, Path::new("project/src/main.rs"));
    }

    #[test]
    fn strip_first_matching_prefix() {
        let tr = StripPrefix::new(vec![PathBuf::from("/home"), PathBuf::from("/home/user")]);
        let result = tr.transform(Path::new("/home/user/project/src/main.rs"));
        assert_eq!(result, Path::new("user/project/src/main.rs"));
    }

    #[test]
    fn no_matching_prefix() {
        let tr = StripPrefix::new(vec![PathBuf::from("/opt")]);
        let result = tr.transform(Path::new("/home/user/project/src/main.rs"));
        assert_eq!(result, Path::new("/home/user/project/src/main.rs"));
    }
}
