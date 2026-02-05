mod test_can2csv;
mod test_canstruct;
mod test_csvcat;
mod test_csvdelta;
mod test_csvstats;
mod test_minpath;

use std::path::PathBuf;
use std::process::Output;
use std::sync::LazyLock;

use assert_cmd::Command;

pub trait CommandExt {
    /// Same as [Command::output] except with hooks to print stdout/stderr in failed tests
    fn captured_output(&mut self) -> std::io::Result<Output>;
}

impl CommandExt for Command {
    fn captured_output(&mut self) -> std::io::Result<Output> {
        let output = self.output()?;

        // libtest injects magic in print! macros to capture output in tests
        print!("{}", String::from_utf8_lossy(&output.stdout));
        eprint!("{}", String::from_utf8_lossy(&output.stderr));

        Ok(output)
    }
}

/// Get a temporary file with the given contents
pub fn tempfile<S: AsRef<str>>(contents: S) -> eyre::Result<tempfile::NamedTempFile> {
    let mut file = tempfile::NamedTempFile::new()?;
    std::io::Write::write_all(&mut file, contents.as_ref().as_bytes())?;
    Ok(file)
}

/// Get a command to run the given tool
///
/// Automatically builds workspace binaries if needed (once per process).
pub fn tool(name: &str) -> Command {
    // Build workspace binaries (once per process). Cargo is fast when nothing
    // needs rebuilding and handles concurrent invocations gracefully.
    //
    // nextest runs each test in its own process, so we'll always hit this path with nextest, but
    // with regular cargo-test, this will only run once. That's an unfortunate tradeoff, but I
    // think it's necessary. It unfortunately results in cargo-test being faster than cargo-nextest
    static BUILD_ONCE: LazyLock<()> = LazyLock::new(|| {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir.parent().expect("tests dir has no parent");

        let status = std::process::Command::new("cargo")
            .args(["build", "--workspace", "--bins"])
            .current_dir(workspace_root)
            .status()
            .expect("Failed to run cargo build");
        assert!(status.success(), "cargo build --workspace --bins failed");
    });
    *BUILD_ONCE; // dereference to trigger the one-time build

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("tests dir has no parent")
        .to_path_buf();

    let target_dir = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| workspace_root.join("target"));

    let path = target_dir.join("debug").join(name);
    let mut cmd = Command::new(&path);
    cmd.arg("--log-level=TRACE");
    cmd
}
