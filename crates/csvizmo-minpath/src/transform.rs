use std::path::{Path, PathBuf};

use indexmap::IndexMap;

use super::abbreviate::SmartAbbreviate;
use super::common_prefix::StripCommonPrefix;
use super::homedir::HomeDir;
use super::normalize::{RelativeTo, ResolveRelative};
use super::prefix::StripPrefix;
use super::single_letter::SingleLetter;
use super::unique_suffix::MinimalUniqueSuffix;

/// Transform that operates on each path independently
pub(crate) trait LocalTransform {
    fn transform(&self, input: &Path) -> PathBuf;
}

/// Transform that requires knowledge of all paths
pub(crate) trait GlobalTransform {
    fn transform(&self, inputs: &[PathBuf]) -> Vec<PathBuf>;
}

/// A transform that can be either local or global
enum Transform {
    Local(Box<dyn LocalTransform>),
    Global(Box<dyn GlobalTransform>),
}

/// A mapping from original paths to their shortened forms
///
/// Created by [`PathTransforms::build`]. Provides O(1) lookup by original path
/// while preserving the original input order when iterating. Duplicate input
/// paths are deduplicated (only the first occurrence is kept).
///
/// ```
/// use csvizmo_minpath::PathTransforms;
///
/// let paths = vec![
///     "/home/alice/project/src/main.rs",
///     "/home/alice/project/src/lib.rs",
/// ];
///
/// let shortened = PathTransforms::new()
///     .home_dir(true)
///     .minimal_unique_suffix(true)
///     .build(&paths);
///
/// // Look up individual paths
/// assert_eq!(shortened.shorten("/home/alice/project/src/main.rs").to_str(), Some("main.rs"));
///
/// // Iterate in original order
/// for (original, short) in shortened.iter() {
///     println!("{} -> {}", original.display(), short.display());
/// }
/// ```
pub struct ShortenedPaths {
    mapping: IndexMap<PathBuf, PathBuf>,
}

impl ShortenedPaths {
    fn new(originals: Vec<PathBuf>, shortened: Vec<PathBuf>) -> Self {
        debug_assert_eq!(originals.len(), shortened.len());
        let mapping = originals.into_iter().zip(shortened).collect();
        Self { mapping }
    }

    /// Returns the shortened form of a path, or the original if not registered
    ///
    /// This is the primary lookup method. It never fails - if the path wasn't
    /// in the original input set, it returns the path unchanged.
    pub fn shorten<'a, P: AsRef<Path> + ?Sized>(&'a self, path: &'a P) -> &'a Path {
        let path = path.as_ref();
        self.mapping.get(path).map(|p| p.as_path()).unwrap_or(path)
    }

    /// Returns the shortened form of a path if it was registered
    pub fn get<P: AsRef<Path>>(&self, path: P) -> Option<&Path> {
        self.mapping.get(path.as_ref()).map(|p| p.as_path())
    }

    /// Iterate over (original, shortened) pairs in input order (duplicates removed)
    pub fn iter(&self) -> impl Iterator<Item = (&Path, &Path)> {
        self.mapping.iter().map(|(k, v)| (k.as_path(), v.as_path()))
    }

    /// Iterate over original paths in input order (duplicates removed)
    pub fn originals(&self) -> impl Iterator<Item = &Path> {
        self.mapping.keys().map(|p| p.as_path())
    }

    /// Iterate over shortened paths in input order (duplicates removed)
    pub fn shortened(&self) -> impl Iterator<Item = &Path> {
        self.mapping.values().map(|p| p.as_path())
    }

    /// Returns the number of unique paths
    pub fn len(&self) -> usize {
        self.mapping.len()
    }

    /// Returns true if there are no paths
    pub fn is_empty(&self) -> bool {
        self.mapping.is_empty()
    }
}

/// A collection of path transforms for shortening file paths.
///
/// Transforms execute in the order they are added to the builder.
///
/// ## Available transforms
///
/// - [`home_dir`](Self::home_dir) - replace `/home/<user>/...` with `~/...`
/// - [`resolve_relative`](Self::resolve_relative) - normalize `.` and `..` components
/// - [`relative_to`](Self::relative_to) - make paths relative to a base
/// - [`strip_prefix`](Self::strip_prefix) - remove specified path prefixes
/// - [`smart_abbreviate`](Self::smart_abbreviate) - abbreviate `Documents` → `docs`, etc.
/// - [`strip_common_prefix`](Self::strip_common_prefix) - remove prefix shared by all paths
/// - [`minimal_unique_suffix`](Self::minimal_unique_suffix) - shorten to unique suffix
/// - [`single_letter`](Self::single_letter) - abbreviate directories to single letters
///
/// ## Example
///
/// ```
/// use csvizmo_minpath::PathTransforms;
///
/// let paths = vec![
///     "/home/alice/project/src/main.rs",
///     "/home/alice/project/src/lib.rs",
/// ];
///
/// let shortened = PathTransforms::new()
///     .home_dir(true)
///     .strip_common_prefix(true)
///     .minimal_unique_suffix(true)
///     .build(&paths);
///
/// // Query individual paths
/// println!("{}", shortened.shorten("/home/alice/project/src/main.rs").display());
/// ```
#[derive(Default)]
pub struct PathTransforms {
    transforms: Vec<Transform>,
}

impl PathTransforms {
    pub fn new() -> Self {
        Self::default()
    }

    fn add_local<T: LocalTransform + 'static>(&mut self, tr: T) {
        self.transforms.push(Transform::Local(Box::new(tr)));
    }

    fn add_global<T: GlobalTransform + 'static>(&mut self, tr: T) {
        self.transforms.push(Transform::Global(Box::new(tr)));
    }

    /// Replace `/home/<user>/...` paths with `~/...`
    pub fn home_dir(mut self, enabled: bool) -> Self {
        if enabled {
            self.add_local(HomeDir);
        }
        self
    }

    /// Normalize paths by resolving `.` and `..` components without filesystem access
    pub fn resolve_relative(mut self, enabled: bool) -> Self {
        if enabled {
            self.add_local(ResolveRelative);
        }
        self
    }

    /// Make paths relative to the given base path (no-op if `None`)
    pub fn relative_to<P: AsRef<Path>>(mut self, base: Option<P>) -> Self {
        if let Some(base) = base {
            self.add_local(RelativeTo::new(base));
        }
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
        if !prefixes.is_empty() {
            self.add_local(StripPrefix::new(prefixes));
        }
        self
    }

    /// Abbreviate common directory names (e.g., `Documents` → `docs`, `source` → `src`)
    pub fn smart_abbreviate(mut self, enabled: bool) -> Self {
        if enabled {
            self.add_local(SmartAbbreviate::new());
        }
        self
    }

    /// Remove the common prefix shared by all paths
    pub fn strip_common_prefix(mut self, enabled: bool) -> Self {
        if enabled {
            self.add_global(StripCommonPrefix);
        }
        self
    }

    /// Shorten paths to the minimal unique suffix (filename, or more if needed to disambiguate)
    pub fn minimal_unique_suffix(mut self, enabled: bool) -> Self {
        if enabled {
            self.add_global(MinimalUniqueSuffix);
        }
        self
    }

    /// Abbreviate directory names to single letters (e.g., `src/utils/parse.rs` → `s/u/parse.rs`)
    pub fn single_letter(mut self, enabled: bool) -> Self {
        if enabled {
            self.add_global(SingleLetter);
        }
        self
    }

    /// Apply all configured transforms and return a lookup structure
    ///
    /// This is the primary entry point for library users. It computes the
    /// shortened forms for all input paths and returns a [`ShortenedPaths`]
    /// that supports O(1) lookup while preserving input order for iteration.
    ///
    /// Transforms are applied in the order they were added to the builder.
    pub fn build<I, P>(&self, inputs: I) -> ShortenedPaths
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let inputs: Vec<PathBuf> = inputs
            .into_iter()
            .map(|p| p.as_ref().to_path_buf())
            .collect();

        let shortened = self.apply(&inputs);
        ShortenedPaths::new(inputs, shortened)
    }

    fn apply(&self, inputs: &[PathBuf]) -> Vec<PathBuf> {
        let mut current: Vec<PathBuf> = inputs.to_vec();

        for transform in &self.transforms {
            current = match transform {
                Transform::Local(tr) => current.iter().map(|p| tr.transform(p)).collect(),
                Transform::Global(tr) => tr.transform(&current),
            };
        }

        current
    }
}
