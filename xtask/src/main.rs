//! Development tasks for ZENITH, run as `cargo xtask <task>`.
//!
//! | Task | What it does |
//! | ---- | ------------ |
//! | `screenshots [--check]` | The screenshots of the README, drawn from the snapshots |
//!
//! Nothing here is a dependency of the `zenith` binary.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "xtask is a developer tool, and what it prints is its whole interface"
)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod screenshots;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let task = arguments.first().map_or("help", String::as_str);
    let rest: Vec<&str> = arguments.iter().skip(1).map(String::as_str).collect();
    let root = root();
    let outcome = match task {
        "screenshots" => screenshots::run(&root, rest.contains(&"--check")),
        "help" | "--help" | "-h" => {
            println!("{}", HELP.trim());
            Ok(())
        }
        other => Err(format!(
            "there is no task called {other}\n\n{}",
            HELP.trim()
        )),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("xtask: {why}");
            ExitCode::FAILURE
        }
    }
}

const HELP: &str = "
cargo xtask screenshots [--check] the screenshots of the README, drawn from the snapshots
";

/// The root of the repository: the parent of this crate's directory.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}
