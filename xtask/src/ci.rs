//! `cargo xtask ci`: what continuous integration runs, on this machine, in that order.
//!
//! A green run here means a green run there. Every step below is the same command, with
//! the same flags, as the step of the same name in `.github/workflows/ci.yml`, and a test
//! holds the two together: the same names, in the same order, and no cargo command in the
//! workflow without `--locked`.
//!
//! Nothing stops at the first failure: a run reports every step, so one command says
//! everything that is wrong rather than the first thing.

use std::path::Path;
use std::process::Command;
use std::time::Instant;

/// What a step runs.
enum Kind {
    /// `cargo` with these arguments.
    Cargo(&'static [&'static str]),
    /// Another program, with the crate that installs it.
    Tool(&'static str, &'static [&'static str], &'static str),
    /// One of xtask's own tasks.
    Task(fn(&Path) -> Result<(), String>),
}

/// One step of the pipeline.
struct Step {
    name: &'static str,
    kind: Kind,
    /// Skipped by `--fast`.
    slow: bool,
}

/// The pipeline, in the order of the workflow.
const STEPS: &[Step] = &[
    Step {
        name: "Formatting",
        kind: Kind::Cargo(&["fmt", "--all", "--", "--check"]),
        slow: false,
    },
    Step {
        name: "TOML formatting",
        kind: Kind::Tool("taplo", &["fmt", "--check"], "taplo-cli"),
        slow: false,
    },
    Step {
        name: "Spelling",
        kind: Kind::Tool("typos", &[], "typos-cli"),
        slow: false,
    },
    Step {
        name: "Lints",
        kind: Kind::Cargo(&[
            "clippy",
            "--workspace",
            "--all-targets",
            "--locked",
            "--",
            "-D",
            "warnings",
        ]),
        slow: false,
    },
    Step {
        name: "Tests",
        kind: Kind::Tool(
            "cargo",
            &["nextest", "run", "--workspace", "--locked"],
            "cargo-nextest",
        ),
        slow: false,
    },
    Step {
        name: "Doctests",
        kind: Kind::Cargo(&["test", "--doc", "--workspace", "--locked"]),
        slow: false,
    },
    Step {
        name: "Documentation",
        kind: Kind::Cargo(&["doc", "--workspace", "--no-deps", "--locked"]),
        slow: false,
    },
    Step {
        name: "Every code change has its changelog entry",
        kind: Kind::Task(changelog),
        slow: false,
    },
    Step {
        name: "The screenshots are the snapshots",
        kind: Kind::Task(screenshots),
        slow: false,
    },
    Step {
        name: "Supply chain",
        kind: Kind::Tool("cargo", &["deny", "check"], "cargo-deny"),
        slow: false,
    },
    Step {
        name: "Coverage",
        kind: Kind::Task(coverage),
        slow: true,
    },
];

fn changelog(root: &Path) -> Result<(), String> {
    crate::changelog::run(root, crate::changelog::DEFAULT_BASE)
}

fn screenshots(root: &Path) -> Result<(), String> {
    crate::screenshots::run(root, true)
}

fn coverage(root: &Path) -> Result<(), String> {
    crate::coverage::run(root, false)
}

/// Runs the pipeline. With `fast`, the coverage step, which runs the whole suite again
/// under instrumentation, is skipped, and the summary says so.
///
/// # Errors
///
/// A sentence naming the steps that failed, after every step has run.
pub(crate) fn run(root: &Path, fast: bool) -> Result<(), String> {
    let mut outcomes: Vec<(&str, bool, f64, Option<String>)> = Vec::new();
    for step in STEPS {
        if fast && step.slow {
            continue;
        }
        println!("ci: {}", step.name);
        let started = Instant::now();
        let (passed, note) = match &step.kind {
            Kind::Cargo(arguments) => external(root, "cargo", arguments, None),
            Kind::Tool(program, arguments, install) => {
                external(root, program, arguments, Some(install))
            }
            Kind::Task(task) => match task(root) {
                Ok(()) => (true, None),
                Err(why) => (
                    false,
                    Some(why.lines().next().unwrap_or_default().to_owned()),
                ),
            },
        };
        if !passed {
            println!("ci: {} FAILED", step.name);
        }
        outcomes.push((step.name, passed, started.elapsed().as_secs_f64(), note));
    }
    println!();
    println!("  {:<44}  result  seconds", "step");
    for (name, passed, seconds, note) in &outcomes {
        println!(
            "  {name:<44}  {:<6}  {seconds:>7.1}",
            if *passed { "ok" } else { "FAILED" }
        );
        if let Some(note) = note {
            println!("      {note}");
        }
    }
    println!();
    if fast {
        println!("ci: SKIPPED the coverage step, because --fast. CI runs it.");
    }
    if std::env::var_os("ZENITH_SOLAR").is_none() {
        println!(
            "ci: ZENITH_SOLAR is not set, so the tests against SOLAR used the PATH, or said \
             they were skipped. CI builds SOLAR and requires them."
        );
    }
    let failed: Vec<&str> = outcomes
        .iter()
        .filter(|(_, passed, _, _)| !passed)
        .map(|(name, _, _, _)| *name)
        .collect();
    if failed.is_empty() {
        println!("ci: every step passed");
        Ok(())
    } else {
        Err(format!(
            "{} of {} steps failed: {}",
            failed.len(),
            outcomes.len(),
            failed.join(", ")
        ))
    }
}

/// Runs a program as a step, in the ordinary target directory. `xtask` has no integration
/// tests, so no nested command links `xtask.exe` while it runs, which Windows would refuse;
/// and a second build tree would not fit on the disk this was written on.
fn external(
    root: &Path,
    program: &str,
    arguments: &[&str],
    install: Option<&str>,
) -> (bool, Option<String>) {
    let status = Command::new(program)
        .args(arguments)
        .current_dir(root)
        .env("RUSTDOCFLAGS", "-D warnings")
        .status();
    match status {
        Ok(status) => (status.success(), None),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (
            false,
            Some(match install {
                Some(name) => format!("{program} is not installed: cargo install {name} --locked"),
                None => format!("{program} was not found"),
            }),
        ),
        Err(error) => (false, Some(error.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORKFLOW: &str = include_str!("../../.github/workflows/ci.yml");

    /// The step names of the workflow, in order.
    fn workflow_steps() -> Vec<String> {
        WORKFLOW
            .lines()
            .filter_map(|line| line.trim().strip_prefix("- name: "))
            .map(|name| name.trim().trim_matches('"').to_owned())
            .collect()
    }

    #[test]
    fn every_step_is_in_the_workflow_in_the_same_order() {
        let workflow = workflow_steps();
        let ours: Vec<&str> = STEPS.iter().map(|step| step.name).collect();
        let theirs: Vec<&str> = workflow
            .iter()
            .map(String::as_str)
            .filter(|name| ours.contains(name))
            .collect();
        // Coverage is a job of its own in the workflow, and comes last there too.
        assert_eq!(theirs, ours, "the workflow and cargo xtask ci disagree");
    }

    #[test]
    fn every_cargo_command_in_the_workflow_is_locked() {
        for line in WORKFLOW.lines() {
            let command = line.trim().trim_start_matches("run: ");
            let builds = [
                "cargo build",
                "cargo test",
                "cargo clippy",
                "cargo nextest",
                "cargo doc",
                "cargo run",
            ];
            if builds.iter().any(|build| command.starts_with(build)) {
                assert!(command.contains("--locked"), "not locked: {command}");
            }
        }
    }
}
