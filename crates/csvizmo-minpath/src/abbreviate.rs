use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::transform::LocalTransform;

pub struct SmartAbbreviate {
    abbreviations: HashMap<&'static str, &'static str>,
}

impl Default for SmartAbbreviate {
    fn default() -> Self {
        Self::new()
    }
}

impl SmartAbbreviate {
    pub fn new() -> Self {
        Self {
            abbreviations: HashMap::from([
                ("application", "app"),
                ("configuration", "config"),
                ("configurations", "configs"),
                ("dependencies", "deps"),
                ("documents", "docs"),
                ("downloads", "dl"),
                ("libraries", "libs"),
                ("library", "lib"),
                ("pictures", "pics"),
                ("production", "prod"),
                ("repository", "repo"),
                ("source", "src"),
                ("sources", "src"),
            ]),
        }
    }

    fn abbreviate_component(&self, component: &str) -> Option<&str> {
        self.abbreviations
            .get(component.to_lowercase().as_str())
            .copied()
    }
}

impl LocalTransform for SmartAbbreviate {
    fn transform(&self, input: &Path) -> PathBuf {
        input
            .iter()
            .map(|component| {
                let s = component.to_string_lossy();
                match self.abbreviate_component(&s) {
                    Some(abbrev) => abbrev.into(),
                    None => component.to_os_string(),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abbreviates_documents() {
        let tr = SmartAbbreviate::new();
        let result = tr.transform(Path::new("/home/user/documents/file.txt"));
        assert_eq!(result, Path::new("/home/user/docs/file.txt"));
    }

    #[test]
    fn abbreviates_case_insensitive() {
        let tr = SmartAbbreviate::new();
        let result = tr.transform(Path::new("/home/user/DOCUMENTS/file.txt"));
        assert_eq!(result, Path::new("/home/user/docs/file.txt"));
    }

    #[test]
    fn abbreviates_multiple_components() {
        let tr = SmartAbbreviate::new();
        let result = tr.transform(Path::new("/home/user/Documents/Source/lib.rs"));
        assert_eq!(result, Path::new("/home/user/docs/src/lib.rs"));
    }

    #[test]
    fn leaves_unknown_components_unchanged() {
        let tr = SmartAbbreviate::new();
        let result = tr.transform(Path::new("/home/user/projects/foo/bar.rs"));
        assert_eq!(result, Path::new("/home/user/projects/foo/bar.rs"));
    }
}
