//! `cargo xtask coverage`: how much of the shipped code the tests really run.
//!
//! Coverage is not a score to admire. It is a list of the lines nobody has ever executed,
//! and the value of the number is that it cannot fall: [`FLOOR`] is what was measured when
//! it was set, rounded down, and CI refuses a change that goes below it.
//!
//! What is measured is the two crates that ship, `zenith-client` and `zenith`. The test
//! double and `main.rs`, which only the pseudo-terminal harness of `cargo xtask perf`
//! reaches, are left out, as is `xtask`, which is the harness rather than the product.
//! The tests against SOLAR count only when SOLAR is there, so CI measures with SOLAR built.

use std::path::Path;
use std::process::Command;

/// The floor, in percent of lines, and it only ever goes up. Set on 27 September 2026, when
/// the tests ran 88.46 % of the lines with SOLAR 0.2.0 present, rounded down.
pub(crate) const FLOOR: u32 = 88;

/// The seed of the property tests' random cases in a coverage run. Their cases reach code
/// that no other test reaches, so with a new seed each run the figure moved with chance:
/// 88.15 % and 87.87 % on two runs of CI with the same tests. Every other run of the tests
/// draws new cases.
const PROPTEST_SEED: &str = "20260927";

/// The crates that ship, and therefore the crates that are measured.
const MEASURED: [&str; 2] = ["zenith-client", "zenith"];

/// What is not measured, as a regular expression over file paths.
const IGNORED: &str = r"xtask|zenith-solar-double|zenith[/\\]src[/\\]main\.rs";

/// Runs the tests under instrumentation and reports the coverage.
///
/// # Errors
///
/// Why the run failed, or that the coverage fell below [`FLOOR`].
pub(crate) fn run(root: &Path, write_reports: bool) -> Result<(), String> {
    let reports = root.join("target").join("coverage");
    let mut arguments: Vec<String> = vec![
        "llvm-cov".to_owned(),
        "nextest".to_owned(),
        "--locked".to_owned(),
        "--workspace".to_owned(),
        "--ignore-filename-regex".to_owned(),
        IGNORED.to_owned(),
        "--fail-under-lines".to_owned(),
        FLOOR.to_string(),
    ];
    arguments.push("--exclude".to_owned());
    arguments.push("xtask".to_owned());
    if write_reports {
        std::fs::create_dir_all(&reports)
            .map_err(|error| format!("{} could not be created: {error}", reports.display()))?;
        arguments.push("--lcov".to_owned());
        arguments.push("--output-path".to_owned());
        arguments.push(reports.join("lcov.info").display().to_string());
    } else {
        arguments.push("--summary-only".to_owned());
    }
    let status = Command::new("cargo")
        .args(&arguments)
        .current_dir(root)
        .env("PROPTEST_RNG_SEED", PROPTEST_SEED)
        .status()
        .map_err(|error| {
            format!(
                "cargo llvm-cov could not be started: {error}. Install it with \
                 `cargo install cargo-llvm-cov --locked`."
            )
        })?;
    if !status.success() {
        return Err(format!(
            "the coverage of {} is below the floor of {FLOOR}% of lines, or the tests failed. \
             Either the change needs tests, or the floor is wrong, and lowering it is a \
             decision, not a fix.",
            MEASURED.join(" and ")
        ));
    }
    if write_reports {
        println!("coverage: lcov in {}", reports.join("lcov.info").display());
    }
    Ok(())
}
