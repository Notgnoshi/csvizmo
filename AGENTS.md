This is a Rust virtual workspace project using 2024 edition and the latest stable toolchain. It
contains a number of CLI tools. Each workspace crate is prefixed with `csvizmo-` and is intended to
contain either shared utilities, or a set of related CLI tools. Each crate should have a set of
integration tests using the `csvizmo-test` crate for shared utilities to run the binaries in the
tests. There's a `tool!()` macro to facilitate creating `Command`s to run the CLI tools, and a
`CommandExt` trait to help with capturing stdout / stderr.

Dependencies should be added in the top level `Cargo.toml`, and then imported into each workspace
crate using `<crate>.workspace = true`.

Each CLI tool should nominally follow the Unix philosophy of "do one thing and do it well", and
should read from stdin / file and write to stdout / file when possible. There's a template for
creating a new CLI in `crates/csvizmo-utils/src/bin/template.rs`.

Build, test, and lint are the usual cargo commands:

```sh
cargo build
cargo test
# Normally prefer nextest for faster execution, although both are expected to work
cargo nextest run
cargo clippy --all-targets
```

This project uses special code formatting rules though:

```sh
cargo fmt -- --config group_imports=StdExternalCrate,imports_granularity=Module
```
