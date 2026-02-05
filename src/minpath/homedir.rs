use std::path::{Component, Path, PathBuf};

use crate::minpath::transform::LocalTransform;

/// Replace /home/<user> with ~
pub struct HomeDir;

impl LocalTransform for HomeDir {
    fn transform(&self, input: &Path) -> PathBuf {
        let mut components = input.components();
        if let Some(Component::RootDir) = components.next() {
            if let Some(Component::Normal(home_dir)) = components.next() {
                if home_dir == "home" {
                    if let Some(Component::Normal(_username)) = components.next() {
                        // Collect remaining components
                        let remaining: PathBuf = components.collect();
                        let mut result = PathBuf::from("~");
                        result.push(remaining);
                        return result;
                    }
                }
            }
        }

        input.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use crate::minpath::{PathTransforms, assert_paths_eq};

    #[test]
    fn homedir_transform() {
        let t = PathTransforms::new().home_dir();
        let inputs = [
            "home/<user>/",
            "/home/alice/documents",
            "/home/bob/.local/share",
            "/etc/config",
            "/opt/foo/bar",
        ];

        let output = t.transform(inputs);
        let expected = [
            "home/<user>/", // Relative paths are left unchanged
            "~/documents",
            "~/.local/share",
            "/etc/config",
            "/opt/foo/bar",
        ];
        assert_paths_eq(expected, output);
    }
}
