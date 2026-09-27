//! `cargo xtask changelog [base]`: code and documentation move together.
//!
//! The rule, fixed for every project of the laboratory on 27 September 2026: every change
//! updates, in the same commit, every document it affects, and an outdated document is a
//! defect, like a failing test. Most of that is judgement, and the pull request template
//! lists it. One part is mechanical, and this is that part: **every commit that changes the
//! code adds at least one line to `CHANGELOG.md`.**
//!
//! Commit by commit, not across the whole range, because the rule says "in the same
//! commit"; and adds, not merely touches, because a commit that only removes lines from the
//! changelog is not an entry. The second half was learnt the hard way: a commit made while
//! this repository was being written staged an older changelog and deleted four entries,
//! which a check of "the file changed" would have passed.
//!
//! Changes not yet committed are held to the same rule, so that the check says so before
//! the commit rather than after it.

use std::path::Path;
use std::process::Command;

/// The base a local run compares with: what has not been pushed yet.
pub(crate) const DEFAULT_BASE: &str = "origin/main";

/// What counts as code: a change to any of these needs a changelog entry.
const CODE: [&str; 4] = ["crates/", "xtask/", "Cargo.toml", "Cargo.lock"];

/// The document that has to move with the code.
const CHANGELOG: &str = "CHANGELOG.md";

/// Checks every commit in `base..HEAD`, and the changes not yet committed.
///
/// # Errors
///
/// Every commit that changed code without adding to the changelog, or why the history
/// could not be read.
pub(crate) fn run(root: &Path, base: &str) -> Result<(), String> {
    let commits = git(root, &["rev-list", "--reverse", &format!("{base}..HEAD")])?;
    let mut offenders = Vec::new();
    let mut checked = 0;
    for commit in commits
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        checked += 1;
        let files = git(
            root,
            &[
                "diff-tree",
                "--no-commit-id",
                "--name-only",
                "-r",
                "--root",
                commit,
            ],
        )?;
        if !touches_code(&files) {
            continue;
        }
        let added = git(
            root,
            &[
                "diff-tree",
                "--no-commit-id",
                "-p",
                "-r",
                "--root",
                commit,
                "--",
                CHANGELOG,
            ],
        )?;
        if !adds_a_line(&added) {
            let subject = git(root, &["log", "-1", "--format=%h %s", commit])?;
            offenders.push(subject.trim().to_owned());
        }
    }
    let pending_files = git(root, &["diff", "HEAD", "--name-only"])?;
    if touches_code(&pending_files) {
        let pending = git(root, &["diff", "HEAD", "--", CHANGELOG])?;
        if !adds_a_line(&pending) {
            offenders.push("the changes not yet committed".to_owned());
        }
    }
    if offenders.is_empty() {
        println!(
            "changelog: {checked} commit(s) since {base}, every one that changed code added to {CHANGELOG}"
        );
        return Ok(());
    }
    Err(format!(
        "{} changed code without adding to {CHANGELOG}:\n  {}\n\nCode and documentation move \
         together: every change updates, in the same commit, every document it affects. Write \
         the entry under `## [Unreleased]`, saying what a reader of ZENITH would notice.",
        if offenders.len() == 1 {
            "this commit"
        } else {
            "these commits"
        },
        offenders.join("\n  ")
    ))
}

fn touches_code(files: &str) -> bool {
    files
        .lines()
        .map(str::trim)
        .any(|file| CODE.iter().any(|code| file.starts_with(code)))
}

/// Whether a diff adds at least one line with something on it.
fn adds_a_line(diff: &str) -> bool {
    diff.lines()
        .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
        .any(|line| !line[1..].trim().is_empty())
}

fn git(root: &Path, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .output()
        .map_err(|error| format!("git could not be started: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_diff_that_only_removes_lines_is_not_an_entry() {
        let removal = "--- a/CHANGELOG.md\n+++ b/CHANGELOG.md\n@@ -1,2 +1 @@\n-- an entry\n";
        assert!(!adds_a_line(removal));
        let addition = "--- a/CHANGELOG.md\n+++ b/CHANGELOG.md\n@@ -1 +1,2 @@\n+- an entry\n";
        assert!(adds_a_line(addition));
        assert!(!adds_a_line("+++ b/CHANGELOG.md\n+   \n"));
    }

    #[test]
    fn code_is_the_crates_the_task_runner_and_the_manifests() {
        assert!(touches_code("crates/zenith/src/app/mod.rs\n"));
        assert!(touches_code("xtask/src/ci.rs"));
        assert!(touches_code("Cargo.lock"));
        assert!(!touches_code("docs/DESIGN.md\nREADME.md"));
    }

    #[test]
    fn a_base_that_does_not_exist_is_reported_rather_than_ignored() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let failure = run(root, "no-such-ref-exists-anywhere").unwrap_err();
        assert!(failure.contains("no-such-ref-exists-anywhere"), "{failure}");
    }
}
