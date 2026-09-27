//! ZENITH's own loop, driven by keys, against the SOLAR installed on this machine, and
//! against ZENITH's test double of one.
//!
//! These tests run the same `Runtime` the binary runs, with ratatui's test backend in place
//! of the terminal: keys go in through the loop's channel, and what SOLAR answers comes back
//! through the real pipes. The walk through every API is the proof that a new SOLAR API is
//! usable from ZENITH with no change to ZENITH (ADR 0004).
//!
//! SOLAR is found as ZENITH finds it: `ZENITH_SOLAR`, then the `PATH`. Without one the
//! tests say so and pass, unless `ZENITH_REQUIRE_SOLAR` is set, as it is in CI.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    reason = "a test reports failure by panicking, and says on standard error what it skipped"
)]

mod common;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use zenith::app::apis::ExampleResult;
use zenith::app::history::Summary;
use zenith::app::link::Phase;
use zenith::app::{App, Incoming, Options, Tab};
use zenith::runtime::Runtime;
use zenith_client::locate::{Search, locate};

const PATIENCE: Duration = Duration::from_secs(30);

fn solar_under_test() -> Option<PathBuf> {
    match locate(&Search::from_environment(None)) {
        Ok(found) => Some(found.path),
        Err(error) => {
            assert!(
                std::env::var_os("ZENITH_REQUIRE_SOLAR").is_none(),
                "ZENITH_REQUIRE_SOLAR is set and SOLAR is not there: {error}"
            );
            eprintln!("skipped: {error} Set ZENITH_SOLAR to run this test.");
            None
        }
    }
}

/// ZENITH's test double, which `zenith-client` builds. It sits in the target directory
/// beside this test's own executable, in `deps/`, when the workspace is built, which is
/// how `cargo nextest run --workspace` and CI build it.
fn double() -> PathBuf {
    let test = std::env::current_exe().unwrap();
    let profile = test.parent().and_then(std::path::Path::parent).unwrap();
    let name = format!("zenith-solar-double{}", std::env::consts::EXE_SUFFIX);
    let path = profile.join(name);
    assert!(
        path.is_file(),
        "{} is not built; run the tests of the whole workspace, cargo nextest run --workspace",
        path.display()
    );
    path
}

struct Driven {
    runtime: Runtime<TestBackend>,
}

impl Driven {
    fn start(solar: PathBuf, environment: &[(&str, &str)]) -> Self {
        let options = Options {
            opening: false,
            solar: Some(solar),
            environment: environment
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
            ..Options::default()
        };
        let terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        Self {
            runtime: Runtime::new(terminal, options, None),
        }
    }

    fn connected(solar: PathBuf) -> Self {
        let mut driven = Self::start(solar, &[]);
        driven.until("the handshake", |app| app.link.connected());
        driven
    }

    fn app(&self) -> &App {
        self.runtime.app()
    }

    /// Runs the loop until the condition holds, and fails the test after a while.
    fn until(&mut self, what: &str, condition: impl Fn(&App) -> bool) {
        let deadline = Instant::now() + PATIENCE;
        while !condition(self.app()) {
            assert!(
                Instant::now() < deadline,
                "gave up waiting for {what}; the screen was:\n{}",
                self.screen()
            );
            self.runtime.step(Duration::from_millis(20)).unwrap();
        }
    }

    fn key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        self.runtime
            .sender()
            .send(Incoming::Key(KeyEvent {
                code,
                modifiers,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            }))
            .unwrap();
        self.runtime.step(Duration::from_millis(5)).unwrap();
    }

    fn type_text(&mut self, text: &str) {
        for character in text.chars() {
            self.key(KeyCode::Char(character), KeyModifiers::NONE);
        }
    }

    fn run(&mut self, line: &str) {
        self.type_text(line);
        self.key(KeyCode::Enter, KeyModifiers::NONE);
    }

    fn screen(&self) -> String {
        common::characters(self.runtime.terminal().backend().buffer())
    }

}

/// Whether no call is waiting.
fn idle(app: &App) -> bool {
    app.link.tracker.waiting().next().is_none()
}

#[test]
fn every_api_of_the_installed_solar_is_usable_from_zenith_by_its_keys() {
    let Some(solar) = solar_under_test() else {
        return;
    };
    let mut driven = Driven::connected(solar);
    let catalogue = driven.app().catalogue().unwrap().clone();
    assert!(!catalogue.apis.is_empty());
    driven.key(KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(driven.app().tab, Tab::Apis);
    let mut ran = 0;
    let mut failures = Vec::new();
    for (position, api) in catalogue.apis.iter().enumerate() {
        driven.key(KeyCode::Char('g'), KeyModifiers::NONE);
        for _ in 0..position {
            driven.key(KeyCode::Down, KeyModifiers::NONE);
        }
        let screen = driven.screen();
        assert!(screen.contains(&api.name), "{} is not on the APIs tab:\n{screen}", api.name);
        for (index, _) in api.examples.iter().enumerate().take(9) {
            let digit = char::from_digit(u32::try_from(index + 1).unwrap(), 10).unwrap();
            driven.key(KeyCode::Char(digit), KeyModifiers::NONE);
            let key = (api.name.clone(), index);
            driven.until(&format!("example {} of {}", index + 1, api.name), |app| {
                !matches!(app.apis.results.get(&key), None | Some(ExampleResult::Running))
            });
            match driven.app().apis.results.get(&key) {
                Some(ExampleResult::Matches { .. }) => ran += 1,
                other => failures.push(format!("{} example {}: {other:?}", api.name, index + 1)),
            }
        }
        // The form of every API opens, and closes.
        driven.key(KeyCode::Enter, KeyModifiers::NONE);
        assert!(driven.app().apis.form.is_some(), "{} has no form", api.name);
        assert!(driven.screen().contains("parameters"));
        driven.key(KeyCode::Esc, KeyModifiers::NONE);
    }
    assert!(failures.is_empty(), "{failures:#?}");
    assert!(ran >= catalogue.apis.len());
    // Every call the walk made kept the contract.
    for (_, record) in driven.app().history.calls.iter() {
        let outcome = record.outcome.as_ref().unwrap();
        assert_eq!(outcome.violations, 0, "{:?} broke the contract", record.method);
    }
}

#[test]
fn every_api_of_the_installed_solar_can_be_described_from_the_command_line() {
    let Some(solar) = solar_under_test() else {
        return;
    };
    let mut driven = Driven::connected(solar);
    let names: Vec<String> = driven
        .app()
        .catalogue()
        .unwrap()
        .names()
        .map(str::to_owned)
        .collect();
    for name in &names {
        driven.run(&format!("/describe {name}"));
        driven.until(&format!("the description of {name}"), idle);
        let (_, record) = driven.app().history.calls.last().unwrap();
        let outcome = record.outcome.as_ref().unwrap();
        assert_eq!(outcome.summary, Summary::Ok, "{name}");
        // A long entry scrolls its first lines off the top of the screen, so the entry is
        // checked in the response; the drawing of the card is what the snapshots hold.
        let line = outcome.line.as_deref().unwrap();
        assert!(line.contains(&format!("\"name\":\"{name}\"")), "{line}");
    }
}

#[test]
fn the_commands_of_the_brief_all_work_against_the_installed_solar() {
    let Some(solar) = solar_under_test() else {
        return;
    };
    let mut driven = Driven::connected(solar);
    for line in ["/list", "/ping zenith", "/version", "/call system.info {}"] {
        driven.run(line);
        driven.until(line, idle);
        let (_, record) = driven.app().history.calls.last().unwrap();
        assert_eq!(record.outcome.as_ref().unwrap().summary, Summary::Ok, "{line}");
    }
    driven.run("/theme light");
    driven.run("/help");
    let screen = driven.screen();
    assert!(screen.contains("/describe <api>"), "{screen}");
    driven.run(r#"/call solar.ping {"mesage": "x"}"#);
    assert!(driven.app().session.band.is_some(), "the unknown parameter was not caught");
    driven.key(KeyCode::Char('u'), KeyModifiers::CONTROL);
    driven.run("/quit");
    assert!(driven.app().quitting);
}

#[test]
fn a_raw_batch_is_answered_by_the_installed_solar_in_whichever_way_it_answers_batches() {
    let Some(solar) = solar_under_test() else {
        return;
    };
    let mut driven = Driven::connected(solar);
    driven.run(r#"/raw [{"jsonrpc":"2.0","id":"a","method":"solar.ping"},{"jsonrpc":"2.0","id":"b","method":"solar.version"}]"#);
    driven.until("the batch", idle);
    let (_, record) = driven.app().history.calls.last().unwrap();
    let summary = &record.outcome.as_ref().unwrap().summary;
    // A SOLAR with batches answers with an array; one without, with UNIMPLEMENTED.
    assert!(
        matches!(summary, Summary::Batch { count: 2 })
            || matches!(summary, Summary::Error { status, .. } if status == "UNIMPLEMENTED"),
        "{summary:?}"
    );
}

#[test]
fn a_report_and_an_export_are_written_as_files_that_hold_the_session() {
    let Some(solar) = solar_under_test() else {
        return;
    };
    let directory = std::env::temp_dir().join(format!("zenith-files-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    let mut driven = Driven::connected(solar);
    driven.run("/ping for the report");
    driven.until("the ping", idle);
    let report = directory.join("report.ndjson");
    driven.run(&format!("/report {}", report.display()));
    driven.until("the report", |_| report.exists());
    driven.until("the report to be complete", |app| {
        app.flash.as_ref().is_some_and(|(_, text)| text.starts_with("Wrote the report"))
    });
    let text = std::fs::read_to_string(&report).unwrap();
    let kinds: Vec<String> = text
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap()["kind"].as_str().unwrap().to_owned())
        .collect();
    for kind in ["header", "zenith", "solar", "system", "dropped", "call"] {
        assert!(kinds.iter().any(|k| k == kind), "the report has no {kind}: {kinds:?}");
    }
    assert!(text.contains("for the report"));
    let export = directory.join("history.ndjson");
    driven.run(&format!("/export {}", export.display()));
    driven.until("the export", |_| export.exists());
    // A file that exists is never overwritten.
    driven.run(&format!("/export {}", export.display()));
    driven.until("the refusal", |app| {
        app.flash.as_ref().is_some_and(|(_, text)| text.starts_with("Could not write"))
    });
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn ctrl_r_restarts_solar_and_the_new_connection_answers() {
    let Some(solar) = solar_under_test() else {
        return;
    };
    let mut driven = Driven::connected(solar);
    let first = driven.app().link.pid;
    driven.key(KeyCode::Char('r'), KeyModifiers::CONTROL);
    driven.until("the second handshake", |app| {
        app.link.generation == 2 && app.link.connected()
    });
    assert_ne!(driven.app().link.pid, first);
    driven.run("/ping again");
    driven.until("the ping", idle);
}

#[test]
fn on_windows_zenith_runs_solar_from_a_copy_and_never_where_it_was_found() {
    let Some(solar) = solar_under_test() else {
        return;
    };
    let driven = Driven::connected(solar.clone());
    let binary = driven.app().link.binary.clone().unwrap();
    if cfg!(windows) {
        assert_ne!(binary.runs, solar);
        assert!(binary.runs.starts_with(std::env::temp_dir().join("zenith")));
    } else {
        assert_eq!(binary.runs, solar);
    }
}

#[test]
fn ctrl_c_cancels_a_call_in_flight_when_the_manifest_offers_solar_cancel() {
    let mut driven = Driven::start(double(), &[("ZENITH_DOUBLE", "cancel")]);
    driven.until("the handshake", |app| app.link.connected());
    assert!(driven.app().link.capabilities.cancel);
    driven.run(r#"/call double.slow {"ms": 3000}"#);
    driven.runtime.step(Duration::from_millis(100)).unwrap();
    driven.key(KeyCode::Char('c'), KeyModifiers::CONTROL);
    driven.until("the cancellation and the call", idle);
    let summaries: Vec<(Option<String>, Summary)> = driven
        .app()
        .history
        .calls
        .iter()
        .skip(2)
        .map(|(_, record)| (record.method.clone(), record.outcome.clone().unwrap().summary))
        .collect();
    assert_eq!(summaries.len(), 2, "{summaries:?}");
    assert!(summaries.iter().any(|(method, summary)| {
        method.as_deref() == Some("double.slow")
            && matches!(summary, Summary::Error { status, .. } if status == "CANCELLED")
    }));
    assert!(!driven.app().quitting, "the first Ctrl+C must not quit");
}

#[test]
fn a_solar_that_exits_at_once_is_reported_with_its_code_and_its_last_words() {
    let mut driven = Driven::start(double(), &[("ZENITH_DOUBLE", "exit:3")]);
    driven.until("the failure", |app| {
        app.link.phase == Phase::Down
            && app
                .link
                .failure
                .as_ref()
                .is_some_and(|failure| failure.what().contains("code 3"))
    });
    let failure = driven.app().link.failure.clone().unwrap();
    assert!(
        failure.last_words().iter().any(|line| line.contains("exiting with 3")),
        "{failure:?}"
    );
}

#[test]
fn a_solar_that_speaks_another_protocol_is_refused_with_both_versions() {
    let mut driven = Driven::start(double(), &[("ZENITH_DOUBLE", "protocol:solar/2")]);
    driven.until("the refusal", |app| app.link.phase == Phase::Down);
    let what = driven.app().link.failure.as_ref().unwrap().what();
    assert!(what.contains("speaks solar/2") && what.contains("speaks solar/1"), "{what}");
}

#[test]
fn a_solar_that_is_not_there_is_reported_with_the_path_that_was_given() {
    let mut driven = Driven::start(PathBuf::from("no/such/solar"), &[]);
    driven.until("the failure", |app| app.link.phase == Phase::Down);
    let what = driven.app().link.failure.as_ref().unwrap().what();
    assert!(what.contains("does not exist"), "{what}");
}
