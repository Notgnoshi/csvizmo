use std::path::{Component, Path, PathBuf};

use crate::minpath::transform::LocalTransform;

// Implementation taken from Path::normalize_lexically, which is unstable, and converted to use
// eyre::Result.
fn normalize(path: &Path) -> eyre::Result<PathBuf> {
    let mut lexical = PathBuf::new();
    let mut iter = path.components().peekable();

    // Find the root, if any, and add it to the lexical path.
    // Here we treat the Windows path "C:\" as a single "root" even though
    // `components` splits it into two: (Prefix, RootDir).
    let root = match iter.peek() {
        Some(Component::ParentDir) => eyre::bail!("Can't normalize paths starting with ../"),
        Some(p @ Component::RootDir) | Some(p @ Component::CurDir) => {
            lexical.push(p);
            iter.next();
            lexical.as_os_str().len()
        }
        Some(Component::Prefix(prefix)) => {
            lexical.push(prefix.as_os_str());
            iter.next();
            if let Some(p @ Component::RootDir) = iter.peek() {
                lexical.push(p);
                iter.next();
            }
            lexical.as_os_str().len()
        }
        None => return Ok(PathBuf::new()),
        Some(Component::Normal(_)) => 0,
    };

    for component in iter {
        match component {
            Component::RootDir => unreachable!(),
            Component::Prefix(_) => eyre::bail!("Unexpected Windows path prefix"),
            Component::CurDir => continue,
            Component::ParentDir => {
                // It's an error if ParentDir causes us to go above the "root".
                if lexical.as_os_str().len() == root {
                    eyre::bail!("Can't normalize paths that go above the root");
                } else {
                    lexical.pop();
                }
            }
            Component::Normal(path) => lexical.push(path),
        }
    }
    Ok(lexical)
}

pub struct ResolveRelative;
impl LocalTransform for ResolveRelative {
    fn transform(&self, input: &Path) -> PathBuf {
        normalize(input).unwrap_or_else(|_| {
            tracing::warn!("Failed to normalize path {input:?}");
            input.to_path_buf()
        })
    }
}

pub struct RelativeTo {
    base: PathBuf,
}

impl RelativeTo {
    pub fn new<P: AsRef<Path>>(base: P) -> Self {
        Self {
            base: base.as_ref().to_path_buf(),
        }
    }
}

impl LocalTransform for RelativeTo {
    fn transform(&self, input: &Path) -> PathBuf {
        // For relative paths, only compute relative path if input starts with base,
        // otherwise we can't verify the relationship without filesystem access.
        // Absolute paths share a common root so pathdiff can always compute correctly.
        //
        // pathdiff assumes that if both the base and the input are relative, they are siblings of
        // each other.
        if self.base.is_relative() && !input.starts_with(&self.base) {
            return input.to_path_buf();
        }
        pathdiff::diff_paths(input, &self.base).unwrap_or_else(|| {
            tracing::warn!("Failed to resolve {input:?} relative to {:?}", self.base);
            input.to_path_buf()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_relative_normalizes_path() {
        let tr = ResolveRelative;
        let result = tr.transform(Path::new("./foo/../bar"));
        assert_eq!(result, PathBuf::from("./bar"));
    }

    #[test]
    fn resolve_relative_falls_back_on_input() {
        let tr = ResolveRelative;
        let input = Path::new("../invalid");
        let result = tr.transform(input);
        // Can't resolve, so return input path
        assert_eq!(result, input);
    }

    #[test]
    fn relative_to_absolute_shared_prefix() {
        let tr = RelativeTo::new("/home/user/project");
        let result = tr.transform(Path::new("/home/user/project/src/main.rs"));
        assert_eq!(result, Path::new("src/main.rs"));
    }

    #[test]
    fn relative_to_absolute_sibling() {
        let tr = RelativeTo::new("/home/user/project/src");
        let result = tr.transform(Path::new("/home/user/project/tests/test.rs"));
        assert_eq!(result, Path::new("../tests/test.rs"));
    }

    #[test]
    fn relative_to_absolute_relative_input() {
        let tr = RelativeTo::new("/home/user/project/src");
        let result = tr.transform(Path::new("tests/test.rs"));
        // Can't resolve, so return input path
        assert_eq!(result, Path::new("tests/test.rs"));
    }

    #[test]
    fn relative_to_relative_base() {
        let tr = RelativeTo::new("src");
        let result = tr.transform(Path::new("src/main.rs"));
        assert_eq!(result, Path::new("main.rs"));
    }

    #[test]
    fn relative_to_unrelated_non_ancestor() {
        let tr = RelativeTo::new("src");
        let result = tr.transform(Path::new("tests/test.rs"));
        // Can't resolve, so return input path
        assert_eq!(result, Path::new("tests/test.rs"));
    }
}
