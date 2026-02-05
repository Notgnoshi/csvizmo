use std::process::Output;

pub use assert_cmd::Command;

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

/// Get a command to run the given tool binary.
///
/// Uses `CARGO_BIN_EXE_<name>` which cargo sets at compile time for
/// integration tests in the same crate as the binary.
///
/// # Example
/// ```ignore
/// use csvizmo_test::{tool, CommandExt};
///
/// let output = tool!("csvcat")
///     .write_stdin("a,b\n")
///     .captured_output()
///     .unwrap();
/// ```
#[macro_export]
macro_rules! tool {
    ($name:literal) => {{
        let mut cmd = $crate::Command::new(env!(concat!("CARGO_BIN_EXE_", $name)));
        cmd.arg("--log-level=TRACE");
        cmd
    }};
}
