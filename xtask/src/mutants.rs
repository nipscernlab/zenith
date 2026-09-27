//! `cargo xtask mutants <dir> [--shards N]`: the report of the weekly mutation run, and its
//! gate.
//!
//! Mutation testing runs in CI only (`AGENTS.md`), in `.github/workflows/scheduled.yml`,
//! split into shards that each leave what cargo-mutants wrote in a directory under `<dir>`.
//! This task adds the shards up: how many mutants the tests caught, how many survived, how
//! many timed out and how many could not be built, the score, and every survivor by file.
//! It writes the report to `<dir>/report.md`, and where GitHub shows it on the run when
//! `GITHUB_STEP_SUMMARY` names a file. It fails when fewer shards left results than were
//! run, or when more mutants survived than [`CEILING`] allows.
//!
//! It only reads files, so it also runs on the artefacts of a run downloaded with
//! `gh run download`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// The survivors the weekly run allows. `None` until a run has been measured, and the job
/// then reports without gating.
pub(crate) const CEILING: Option<usize> = None;

/// What happened to a mutant, as cargo-mutants files it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Caught,
    Missed,
    Timeout,
    Unviable,
}

/// The files cargo-mutants writes, one mutant per line, one file per outcome.
const OUTCOMES: [(&str, Outcome); 4] = [
    ("caught.txt", Outcome::Caught),
    ("missed.txt", Outcome::Missed),
    ("timeout.txt", Outcome::Timeout),
    ("unviable.txt", Outcome::Unviable),
];

/// The outcomes of every shard, added up.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Tally {
    /// The shards that left results, by directory, with how many mutants each tested.
    shards: Vec<(PathBuf, usize)>,
    caught: usize,
    missed: Vec<String>,
    timeout: Vec<String>,
    unviable: usize,
}

impl Tally {
    fn mutants(&self) -> usize {
        self.caught + self.missed.len() + self.timeout.len() + self.unviable
    }

    /// The mutants that built, and so could be caught.
    fn viable(&self) -> usize {
        self.caught + self.missed.len() + self.timeout.len()
    }

    /// The share of the viable mutants the tests noticed, in hundredths of a percent. A
    /// timeout counts as noticed: a test that hangs under a mutant did not pass.
    fn score_hundredths(&self) -> usize {
        let noticed = self.caught + self.timeout.len();
        (noticed * 10_000)
            .checked_div(self.viable())
            .unwrap_or(10_000)
    }

    /// The survivors by the file they are in, the most first.
    fn survivors_by_file(&self) -> Vec<(String, usize)> {
        let mut by_file: BTreeMap<String, usize> = BTreeMap::new();
        for mutant in &self.missed {
            *by_file.entry(file_of(mutant).to_owned()).or_default() += 1;
        }
        let mut sorted: Vec<(String, usize)> = by_file.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        sorted
    }
}

/// The file a mutant is in: its name starts with the path, a colon and the line.
fn file_of(mutant: &str) -> &str {
    mutant.split(':').next().unwrap_or(mutant)
}

/// Reads every directory under `dir` that holds an outcome file of cargo-mutants.
fn tally(dir: &Path) -> Result<Tally, String> {
    let mut tally = Tally::default();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        let entries = std::fs::read_dir(&current)
            .map_err(|error| format!("{} could not be read: {error}", current.display()))?;
        let mut here = 0;
        let mut found = false;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let name = entry.file_name();
            let Some((_, outcome)) = OUTCOMES.iter().find(|(file, _)| name == *file) else {
                continue;
            };
            found = true;
            let text = std::fs::read_to_string(&path)
                .map_err(|error| format!("{} could not be read: {error}", path.display()))?;
            for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
                here += 1;
                match outcome {
                    Outcome::Caught => tally.caught += 1,
                    Outcome::Missed => tally.missed.push(line.to_owned()),
                    Outcome::Timeout => tally.timeout.push(line.to_owned()),
                    Outcome::Unviable => tally.unviable += 1,
                }
            }
        }
        if found {
            tally.shards.push((current, here));
        }
    }
    tally.shards.sort();
    tally.missed.sort();
    tally.timeout.sort();
    Ok(tally)
}

/// The report, in Markdown.
fn report(tally: &Tally, ceiling: Option<usize>) -> String {
    let score = tally.score_hundredths();
    let mut out = String::from("## Mutation testing\n\n");
    let _ = writeln!(
        out,
        "{} mutants in {} shards: {} caught, {} survived, {} timed out, {} could not be built.",
        tally.mutants(),
        tally.shards.len(),
        tally.caught,
        tally.missed.len(),
        tally.timeout.len(),
        tally.unviable
    );
    let _ = writeln!(
        out,
        "The tests noticed {}.{:02} % of the {} mutants that built, a timeout counting as noticed.\n",
        score / 100,
        score % 100,
        tally.viable()
    );
    match ceiling {
        Some(ceiling) => {
            let _ = writeln!(out, "The ceiling is {ceiling} survivors.\n");
        }
        None => out.push_str("No ceiling is set yet: this run measures, and does not gate.\n\n"),
    }
    let survivors = tally.survivors_by_file();
    if !survivors.is_empty() {
        out.push_str("| File | Survivors |\n| ---- | --------- |\n");
        for (file, count) in &survivors {
            let _ = writeln!(out, "| `{file}` | {count} |");
        }
        out.push_str("\n<details><summary>Every survivor</summary>\n\n```text\n");
        for mutant in &tally.missed {
            let _ = writeln!(out, "{mutant}");
        }
        out.push_str("```\n\n</details>\n");
    }
    if !tally.timeout.is_empty() {
        out.push_str("\n<details><summary>Every timeout</summary>\n\n```text\n");
        for mutant in &tally.timeout {
            let _ = writeln!(out, "{mutant}");
        }
        out.push_str("```\n\n</details>\n");
    }
    out
}

/// Why the run fails, if it does.
fn verdict(tally: &Tally, shards: Option<usize>, ceiling: Option<usize>) -> Result<(), String> {
    if let Some(expected) = shards
        && tally.shards.len() < expected
    {
        return Err(format!(
            "only {} of the {expected} shards left results; the others failed before \
             cargo-mutants wrote anything",
            tally.shards.len()
        ));
    }
    if let Some((directory, _)) = tally.shards.iter().find(|(_, tested)| *tested == 0) {
        return Err(format!(
            "the shard in {} tested no mutant, which happens when the tests fail before \
             any mutation, so its part of the code was not measured",
            directory.display()
        ));
    }
    if let Some(ceiling) = ceiling
        && tally.missed.len() > ceiling
    {
        return Err(format!(
            "{} mutants survived, and the ceiling is {ceiling}: each new survivor needs a test \
             that notices it, or a record in .cargo/mutants.toml, with its reason, that it \
             changes nothing a test could observe",
            tally.missed.len()
        ));
    }
    Ok(())
}

/// Adds up the shards under the directory given, writes the report, and judges the run.
///
/// # Errors
///
/// A directory that cannot be read, a shard that left no result, or more survivors than
/// the ceiling.
pub(crate) fn run(rest: &[&str]) -> Result<(), String> {
    let Some(dir) = rest.first().filter(|argument| !argument.starts_with('-')) else {
        return Err("give the directory the shards' results are in".to_owned());
    };
    let shards = rest
        .iter()
        .position(|argument| *argument == "--shards")
        .and_then(|at| rest.get(at + 1))
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|_| format!("--shards takes a number, not {value}"))
        })
        .transpose()?;
    let dir = Path::new(dir);
    let tally = tally(dir)?;
    let text = report(&tally, CEILING);
    print!("{text}");
    std::fs::write(dir.join("report.md"), &text)
        .map_err(|error| format!("the report could not be written: {error}"))?;
    if let Some(summary) = std::env::var_os("GITHUB_STEP_SUMMARY") {
        use std::io::Write as _;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(summary)
            .map_err(|error| format!("the job summary could not be opened: {error}"))?;
        file.write_all(text.as_bytes())
            .map_err(|error| format!("the job summary could not be written: {error}"))?;
    }
    verdict(&tally, shards, CEILING)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shard(root: &Path, name: &str, files: &[(&str, &str)]) {
        let directory = root.join(name);
        std::fs::create_dir_all(&directory).unwrap();
        for (file, text) in files {
            std::fs::write(directory.join(file), text).unwrap();
        }
    }

    fn fresh(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("xtask-mutants-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn the_shards_are_added_up_and_the_survivors_grouped_by_file() {
        let root = fresh("tally");
        shard(
            &root,
            "mutants-shard-0",
            &[
                ("caught.txt", "a.rs:1:1: one\na.rs:2:1: two\n"),
                ("missed.txt", "src/b.rs:3:5: replace x with y\n"),
                ("unviable.txt", "a.rs:9:1: nine\n"),
            ],
        );
        shard(
            &root,
            "mutants-shard-1",
            &[
                ("caught.txt", "c.rs:1:1: one\n"),
                ("missed.txt", "src/b.rs:4:5: four\nsrc/c.rs:1:2: five\n"),
                ("timeout.txt", "c.rs:7:1: loops\n"),
            ],
        );
        let tally = tally(&root).unwrap();
        assert_eq!(tally.shards.len(), 2);
        assert_eq!(
            (
                tally.caught,
                tally.missed.len(),
                tally.timeout.len(),
                tally.unviable
            ),
            (3, 3, 1, 1)
        );
        assert_eq!(tally.mutants(), 8);
        assert_eq!(tally.viable(), 7);
        // 4 noticed of 7 viable: 57.14 %.
        assert_eq!(tally.score_hundredths(), 5714);
        assert_eq!(
            tally.survivors_by_file(),
            vec![("src/b.rs".to_owned(), 2), ("src/c.rs".to_owned(), 1)]
        );
        let text = report(&tally, None);
        assert!(text.contains(
            "8 mutants in 2 shards: 3 caught, 3 survived, 1 timed out, 1 could not be built."
        ));
        assert!(text.contains("57.14 %"));
        assert!(text.contains("| `src/b.rs` | 2 |"));
        assert!(text.contains("does not gate"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_missing_shard_or_one_that_tested_nothing_fails_the_run() {
        let root = fresh("missing");
        shard(
            &root,
            "mutants-shard-0",
            &[("caught.txt", "a.rs:1:1: one\n")],
        );
        let tally_one = tally(&root).unwrap();
        assert!(verdict(&tally_one, Some(1), None).is_ok());
        let error = verdict(&tally_one, Some(2), None).unwrap_err();
        assert!(error.contains("only 1 of the 2 shards"), "{error}");
        shard(
            &root,
            "mutants-shard-1",
            &[("caught.txt", ""), ("missed.txt", "")],
        );
        let error = verdict(&tally(&root).unwrap(), Some(2), None).unwrap_err();
        assert!(error.contains("tested no mutant"), "{error}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn survivors_above_the_ceiling_fail_the_run_and_at_it_do_not() {
        let tally = Tally {
            shards: vec![(PathBuf::from("s"), 3)],
            caught: 1,
            missed: vec!["a.rs:1:1: one".to_owned(), "a.rs:2:1: two".to_owned()],
            ..Tally::default()
        };
        assert!(verdict(&tally, None, Some(2)).is_ok());
        let error = verdict(&tally, None, Some(1)).unwrap_err();
        assert!(
            error.contains("2 mutants survived, and the ceiling is 1"),
            "{error}"
        );
        assert!(verdict(&tally, None, None).is_ok());
    }

    #[test]
    fn a_run_with_nothing_viable_scores_everything() {
        assert_eq!(Tally::default().score_hundredths(), 10_000);
    }
}
