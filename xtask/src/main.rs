//! Development tasks for ZENITH, run as `cargo xtask <task>`.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "xtask is a developer tool, and what it prints is its whole interface"
)]

use std::process::ExitCode;

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    eprintln!("xtask: no task called `{task}` yet");
    ExitCode::FAILURE
}
