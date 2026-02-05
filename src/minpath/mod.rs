mod abbreviate;
mod common_prefix;
mod homedir;
mod normalize;
mod prefix;
mod single_letter;
mod transform;
mod unique_suffix;

pub use transform::PathTransforms;

#[cfg(test)]
#[track_caller]
pub fn assert_paths_eq<I1, P1, I2, P2>(expected: I1, actual: I2)
where
    I1: IntoIterator<Item = P1>,
    P1: AsRef<std::path::Path>,
    I2: IntoIterator<Item = P2>,
    P2: AsRef<std::path::Path>,
{
    for (e, a) in expected.into_iter().zip(actual.into_iter()) {
        pretty_assertions::assert_eq!(e.as_ref(), a.as_ref());
    }
}
