//! Development tasks for ZENITH, run as `cargo xtask <task>`.
//!
//! | Task | What it does |
//! | ---- | ------------ |
//! | `ci [--fast]` | What CI runs, in the same order, with the same flags |
//! | `changelog [base]` | Every commit since `base` that changes code adds to `CHANGELOG.md` |
//! | `coverage [--report]` | The coverage of the shipped crates, against its floor |
//! | `screenshots [--check]` | The screenshots of the README, drawn from the snapshots |
//! | `perf` | Startup, a key to its redraw, and the processor at idle, measured |
//! | `soak [--calls N]` | The memory at idle and over a long session, measured |
//! | `walkthrough` | The table of `docs/TESTING_BY_HAND.md`, done in a pseudo-terminal |
//!
//! Nothing here is a dependency of the `zenith` binary.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "xtask is a developer tool, and what it prints is its whole interface"
)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod changelog;
mod ci;
mod coverage;
mod perf;
mod pty;
mod screenshots;
mod soak;
mod walkthrough;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let task = arguments.first().map_or("help", String::as_str);
    let rest: Vec<&str> = arguments.iter().skip(1).map(String::as_str).collect();
    let root = root();
    let outcome = match task {
        "ci" => ci::run(&root, rest.contains(&"--fast")),
        "changelog" => match rest.first() {
            Some(base) if !base.starts_with('-') => changelog::run(&root, base),
            _ => changelog::run(&root, changelog::DEFAULT_BASE),
        },
        "coverage" => coverage::run(&root, rest.contains(&"--report")),
        "screenshots" => screenshots::run(&root, rest.contains(&"--check")),
        "perf" => perf::run(&root, &rest),
        "soak" => soak::run(&root, &rest),
        "walkthrough" => walkthrough::run(&root),
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
cargo xtask ci [--fast]          what CI runs, in the same order, with the same flags
cargo xtask changelog [base]     every commit since base that changes code adds to CHANGELOG.md
cargo xtask coverage [--report]  the coverage of the shipped crates, against its floor
cargo xtask screenshots [--check] the screenshots of the README, drawn from the snapshots
cargo xtask perf                 startup, a key to its redraw, and the processor at idle
cargo xtask soak [--calls N]     the memory at idle and over a long session
cargo xtask walkthrough          the table of docs/TESTING_BY_HAND.md, done by a machine
";

/// The root of the repository: the parent of this crate's directory.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}
