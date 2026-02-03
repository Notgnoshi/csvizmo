use std::path::{Path, PathBuf};

#[cfg(test)]
#[track_caller]
pub(crate) fn assert_paths_eq<I1, P1, I2, P2>(expected: I1, actual: I2)
where
    I1: IntoIterator<Item = P1>,
    P1: AsRef<Path>,
    I2: IntoIterator<Item = P2>,
    P2: AsRef<Path>,
{
    for (e, a) in expected.into_iter().zip(actual.into_iter()) {
        pretty_assertions::assert_eq!(e.as_ref(), a.as_ref());
    }
}

/// Simple path transform that doesn't require global knowledge of all input paths
pub trait LocalTransform {
    fn transform(&self, input: &Path) -> PathBuf;
}

/// Complex path transform that requires global knowledge of all input paths
pub trait GlobalTransform {
    fn transform(&self, inputs: &[PathBuf]) -> Vec<PathBuf>;
}

/// A collection of local and global path transforms
#[derive(Default)]
pub struct PathTransforms {
    local: Vec<Box<dyn LocalTransform>>,
    global: Vec<Box<dyn GlobalTransform>>,
}

impl PathTransforms {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_local<T: LocalTransform + 'static>(tr: T) -> Self {
        let mut this = Self::new();
        this.add_local(tr);
        this
    }

    pub fn add_local<T: LocalTransform + 'static>(&mut self, tr: T) {
        self.local.push(Box::new(tr));
    }

    pub fn from_global<T: GlobalTransform + 'static>(tr: T) -> Self {
        let mut this = Self::new();
        this.add_global(tr);
        this
    }

    pub fn add_global<T: GlobalTransform + 'static>(&mut self, tr: T) {
        self.global.push(Box::new(tr));
    }

    // Don't implement LocalTransform or GlobalTransform so that:
    //
    // * it's clear that there's only one transform function that applies both kinds in order
    // * we can use <P: AsRef<Path>> without dealing with object safety
    // * we can avoid forcing users to import LocalTransform and GlobalTransform traits
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
