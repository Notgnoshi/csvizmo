mod test_can2csv;
mod test_canstruct;
mod test_csvdelta;
mod test_csvstats;

use std::path::PathBuf;
use std::process::Output;
use std::sync::LazyLock;

use assert_cmd::Command;

// Do the expensive cargo invocation to find the path to the binary once, and then cache it for
// future lookups.
static CAN2CSV: LazyLock<PathBuf> = LazyLock::new(|| assert_cmd::cargo::cargo_bin("can2csv"));
static CANSTRUCT: LazyLock<PathBuf> = LazyLock::new(|| assert_cmd::cargo::cargo_bin("canstruct"));
static CSVDELTA: LazyLock<PathBuf> = LazyLock::new(|| assert_cmd::cargo::cargo_bin("csvdelta"));
static CSVSTATS: LazyLock<PathBuf> = LazyLock::new(|| assert_cmd::cargo::cargo_bin("csvstats"));

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

pub fn can2csv() -> Command {
    let mut cmd = Command::new(&*CAN2CSV);
    cmd.arg("--log-level=TRACE");
    cmd
}

pub fn canstruct() -> Command {
    let mut cmd = Command::new(&*CANSTRUCT);
    cmd.arg("--log-level=TRACE");
    cmd
}

pub fn csvdelta() -> Command {
    let mut cmd = Command::new(&*CSVDELTA);
    cmd.arg("--log-level=TRACE");
    cmd
}

pub fn csvstats() -> Command {
    let mut cmd = Command::new(&*CSVSTATS);
    cmd.arg("--log-level=TRACE");
    cmd
}
