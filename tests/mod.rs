mod test_can2csv;
mod test_canstruct;
mod test_csvdelta;
mod test_csvstats;

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Output;
use std::sync::{LazyLock, Mutex};

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

/// Get a command to run the given tool with Cargo
pub fn tool(name: &'static str) -> Command {
    // XXX: Using nextest somewhat defeats this cache, because it runs each test in a separate
    // process, so the cache has to be rebuilt each time. But having it at least makes me feel
    // like I tried :/
    static TOOL_PATH_CACHE: LazyLock<Mutex<HashMap<&'static str, PathBuf>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    let mut cache = TOOL_PATH_CACHE.lock().unwrap();
    // assert_cmd::cargo::cargo_bin is deprecated but cargo_bin! requires string literal, not &'static str
    #[allow(deprecated)]
    let path = cache
        .entry(name)
        // TODO: Support the various ./scripts/ as well
        .or_insert_with(|| assert_cmd::cargo::cargo_bin(name));

    let mut cmd = Command::new(path);
    cmd.arg("--log-level=TRACE");
    cmd
}
