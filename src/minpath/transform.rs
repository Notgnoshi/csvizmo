use std::path::{Path, PathBuf};

use super::abbreviate::SmartAbbreviate;
use super::common_prefix::StripCommonPrefix;
use super::homedir::HomeDir;
use super::normalize::{RelativeTo, ResolveRelative};
use super::prefix::StripPrefix;
use super::single_letter::SingleLetter;
use super::unique_suffix::MinimalUniqueSuffix;

/// Simple path transform that doesn't require global knowledge of all input paths
pub trait LocalTransform {
    fn transform(&self, input: &Path) -> PathBuf;
}

/// Complex path transform that requires global knowledge of all input paths
pub trait GlobalTransform {
    fn transform(&self, inputs: &[PathBuf]) -> Vec<PathBuf>;
}

/// A collection of local and global path transforms
///
/// Use the fluent builder methods to configure which transforms to apply:
///
/// ```
/// use csvizmo::minpath::PathTransforms;
///
/// let paths = vec![
///     "/home/alice/project/src/main.rs",
///     "/home/alice/project/src/lib.rs",
/// ];
///
/// let shortened = PathTransforms::new()
///     .home_dir()
///     .strip_common_prefix()
///     .minimal_unique_suffix()
///     .transform(&paths);
/// ```
#[derive(Default)]
pub struct PathTransforms {
    local: Vec<Box<dyn LocalTransform>>,
    global: Vec<Box<dyn GlobalTransform>>,
}

impl PathTransforms {
    pub fn new() -> Self {
        Self::default()
    }

    fn add_local<T: LocalTransform + 'static>(&mut self, tr: T) {
        self.local.push(Box::new(tr));
    }

    fn add_global<T: GlobalTransform + 'static>(&mut self, tr: T) {
        self.global.push(Box::new(tr));
    }

    // -------------------------------------------------------------------------
    // Local transforms (applied per-path, before global transforms)
    // -------------------------------------------------------------------------

    /// Replace `/home/<user>/...` paths with `~/...`
    pub fn home_dir(mut self) -> Self {
        self.add_local(HomeDir);
        self
    }

    /// Normalize paths by resolving `.` and `..` components without filesystem access
    pub fn resolve_relative(mut self) -> Self {
        self.add_local(ResolveRelative);
        self
    }

    /// Make paths relative to the given base path
    pub fn relative_to<P: AsRef<Path>>(mut self, base: P) -> Self {
        self.add_local(RelativeTo::new(base));
        self
    }

    /// Strip the given prefixes from paths (first matching prefix wins)
    pub fn strip_prefix<I, P>(mut self, prefixes: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let prefixes: Vec<PathBuf> = prefixes
            .into_iter()
            .map(|p| p.as_ref().to_path_buf())
            .collect();
        self.add_local(StripPrefix::new(prefixes));
        self
    }

    /// Abbreviate common directory names (e.g., `Documents` → `docs`, `source` → `src`)
    pub fn smart_abbreviate(mut self) -> Self {
        self.add_local(SmartAbbreviate::new());
        self
    }

    // -------------------------------------------------------------------------
    // Global transforms (require knowledge of all paths, applied after local)
    // -------------------------------------------------------------------------

    /// Remove the common prefix shared by all paths
    pub fn strip_common_prefix(mut self) -> Self {
        self.add_global(StripCommonPrefix);
        self
    }

    /// Shorten paths to the minimal unique suffix (filename, or more if needed to disambiguate)
    pub fn minimal_unique_suffix(mut self) -> Self {
        self.add_global(MinimalUniqueSuffix);
        self
    }

    /// Abbreviate directory names to single letters (e.g., `src/utils/parse.rs` → `s/u/parse.rs`)
    pub fn single_letter(mut self) -> Self {
        self.add_global(SingleLetter);
        self
    }

    // -------------------------------------------------------------------------
    // Execution
    // -------------------------------------------------------------------------

    /// Apply all configured transforms to the input paths
    ///
    /// Local transforms are applied first (in the order they were added),
    /// then global transforms (in the order they were added).
    pub fn transform<I, P>(&self, inputs: I) -> Vec<PathBuf>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let inputs: Vec<PathBuf> = inputs
            .into_iter()
            .map(|p| p.as_ref().to_path_buf())
            .collect();

        // Apply local transforms first
        let mut current: Vec<_> = inputs
            .iter()
            .map(|p| {
                let mut result = p.clone();
                for tr in &self.local {
                    result = tr.transform(&result);
                }
                result
            })
            .collect();

        // Then apply global transforms
        for tr in &self.global {
            current = tr.transform(&current);
        }

        current
    }
}
