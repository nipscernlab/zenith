//! The state machine, driven by scripted events: no terminal, no SOLAR, no clock.

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use serde_json::{Value, json};
use zenith_client::connection::Event as ConnectionEvent;
use zenith_client::locate::{Found, Origin as FoundBy, Prepared};

use super::*;

const MANIFEST: &str =
    include_str!("../../../zenith-client/tests/fixtures/solar-0.1.0-manifest.json");

/// 27 September 2026, 15:47:00 UTC.
const SECONDS_SINCE_THE_EPOCH: u64 = 1_790_524_020;

fn wall() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(SECONDS_SINCE_THE_EPOCH)
}

fn version_data(protocol: &str, layout: &str) -> Value {
    json!({
        "solar_version": "0.1.0",
        "protocol": protocol,
        "manifest_schema_version": layout,
        "build": {"profile": "release", "target": "x86_64-pc-windows-msvc"}
    })
}

fn meta(id: &Value, method: &str) -> Value {
    json!({
        "request_id": id, "method": method, "api_version": "1.0.0", "solar_version": "0.1.0",
        "protocol": "solar/1", "started_at": "2026-09-27T15:47:00.000000Z", "duration_us": 170,
        "os": "windows", "arch": "x86_64"
    })
}

fn success(id: u64, method: &str, data: &Value) -> String {
    let id = json!(id);
    json!({"jsonrpc": "2.0", "id": id, "result": {"data": data, "meta": meta(&id, method), "warnings": []}})
        .to_string()
}

fn failure(id: u64, method: &str, status: &str, code: i64, reason: &str, message: &str) -> String {
    let id = json!(id);
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message, "data": {
        "status": status, "reason": reason,
        "details": [{"field": null, "expected": null, "received": null, "hint": null,
                     "docs": format!("docs/ERRORS.md#{}", status.to_lowercase())}],
        "meta": meta(&id, method)}}})
    .to_string()
}

struct Harness {
    app: App,
    now: Instant,
    effects: Vec<Effect>,
}

impl Harness {
    fn new() -> Self {
        let now = Instant::now();
        let options = Options {
            opening: false,
            ..Options::default()
        };
        let (app, effects) = App::new(options, now, wall());
        Self { app, now, effects }
    }

    fn connected() -> Self {
        let mut harness = Self::new();
        harness.start();
        harness.answer(
            1,
            &success(1, "solar.version", &version_data("solar/1", "2.0.0")),
        );
        let manifest: Value = serde_json::from_str(MANIFEST).unwrap();
        harness.answer(2, &success(2, "solar.manifest", &manifest));
        assert_eq!(harness.app.link.phase, Phase::Connected);
        harness.effects.clear();
        harness
    }

    fn feed(&mut self, incoming: Incoming) {
        let effects = self.app.handle(incoming, self.now);
        self.effects.extend(effects);
    }

    fn later(&mut self, by: Duration) {
        self.now += by;
    }

    fn start(&mut self) {
        let prepared = Prepared {
            found: Found {
                path: PathBuf::from("solar"),
                origin: FoundBy::Path,
            },
            runs: PathBuf::from("solar"),
            sha256: None,
        };
        let generation = self.app.link.generation;
        self.feed(Incoming::Started {
            generation,
            result: Ok(Started {
                prepared,
                pid: 42,
                steps: StartSteps::default(),
            }),
        });
    }

    fn answer(&mut self, _id: u64, line: &str) {
        self.later(Duration::from_micros(500));
        let at = self.now;
        let generation = self.app.link.generation;
        self.feed(Incoming::Connection {
            generation,
            event: ConnectionEvent::Line {
                text: line.to_owned(),
                invalid_utf8: false,
                at,
            },
        });
    }

    fn key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        self.feed(Incoming::Key(KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }));
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

    fn written(&self) -> Vec<Value> {
        self.effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::Write { line, .. } => serde_json::from_str(line).ok(),
                _ => None,
            })
            .collect()
    }

    fn band(&self) -> Option<&Band> {
        self.app.session.band.as_ref()
    }

    fn notices(&self) -> Vec<String> {
        self.app
            .session
            .transcript
            .iter()
            .filter_map(|(_, entry)| match entry {
                Entry::Notice { lines, .. } => Some(lines.join(" ")),
                _ => None,
            })
            .collect()
    }
}

#[test]
fn zenith_starts_by_asking_for_solar_at_trace() {
    let harness = Harness::new();
    assert_eq!(
        harness.effects,
        vec![Effect::Start {
            generation: 1,
            flag: None,
            settings: Settings {
                log_level: "trace".to_owned(),
                environment: Vec::new(),
            },
        }]
    );
}

#[test]
fn the_handshake_asks_for_the_version_and_the_manifest_at_once() {
    let mut harness = Harness::new();
    harness.start();
    let methods: Vec<String> = harness
        .written()
        .iter()
        .map(|request| request["method"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(methods, vec!["solar.version", "solar.manifest"]);
    assert_eq!(harness.app.link.phase, Phase::Handshaking);
}

#[test]
fn a_good_handshake_connects_and_says_what_answered() {
    let harness = Harness::connected();
    let notices = harness.notices();
    assert!(
        notices[0].starts_with("Connected to SOLAR 0.1.0 in "),
        "{notices:?}"
    );
    assert!(notices[0].contains("protocol solar/1, manifest 2.0.0, 5 APIs"));
    assert!(notices[0].contains("This SOLAR has no solar.cancel."));
    assert_eq!(harness.app.history.calls.len(), 2);
}

#[test]
fn another_protocol_stops_the_handshake_with_both_versions() {
    let mut harness = Harness::new();
    harness.start();
    harness.answer(
        1,
        &success(1, "solar.version", &version_data("solar/2", "2.0.0")),
    );
    assert_eq!(harness.app.link.phase, Phase::Down);
    let what = harness.app.link.failure.as_ref().unwrap().what();
    assert!(what.contains("SOLAR 0.1.0 speaks solar/2"), "{what}");
    assert!(what.contains(&format!("ZENITH {} speaks solar/1", crate::VERSION)));
}

#[test]
fn another_manifest_layout_stops_the_handshake() {
    let mut harness = Harness::new();
    harness.start();
    harness.answer(
        1,
        &success(1, "solar.version", &version_data("solar/1", "3.0.0")),
    );
    let failure = harness.app.link.failure.clone().unwrap();
    assert!(matches!(failure, Failure::Handshake(_)));
    assert!(failure.what().contains("layout 3.0.0"));
}

#[test]
fn a_start_that_fails_says_why_and_the_tabs_stay_usable() {
    let mut harness = Harness::new();
    harness.feed(Incoming::Started {
        generation: 1,
        result: Err(Failure::Locate(
            zenith_client::locate::LocateError::NotOnPath { directories: 12 },
        )),
    });
    assert_eq!(harness.app.link.phase, Phase::Down);
    assert!(harness.notices()[0].contains("which has 12 directories"));
    harness.key(KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(harness.app.tab, Tab::Apis);
}

#[test]
fn a_handshake_without_an_answer_gives_up_after_fifteen_seconds() {
    let mut harness = Harness::new();
    harness.start();
    harness.later(Duration::from_secs(14));
    harness.feed(Incoming::Tick);
    assert_eq!(harness.app.link.phase, Phase::Handshaking);
    harness.later(Duration::from_secs(2));
    harness.feed(Incoming::Tick);
    assert_eq!(harness.app.link.failure, Some(Failure::Timeout));
}

#[test]
fn garbage_during_the_handshake_is_a_failure_of_the_contract() {
    let mut harness = Harness::new();
    harness.start();
    harness.answer(1, "this is not json");
    assert!(matches!(
        harness.app.link.failure,
        Some(Failure::NotContract { .. })
    ));
}

#[test]
fn ping_sends_a_request_and_the_answer_lands_in_the_history_with_its_timing() {
    let mut harness = Harness::connected();
    harness.run("/ping hello");
    let written = harness.written();
    assert_eq!(written[0]["method"], json!("solar.ping"));
    assert_eq!(written[0]["params"], json!({"message": "hello"}));
    let id = written[0]["id"].as_u64().unwrap();
    harness.answer(
        id,
        &success(id, "solar.ping", &json!({"pong": true, "echo": "hello"})),
    );
    let (_, record) = harness.app.history.calls.last().unwrap();
    let outcome = record.outcome.as_ref().unwrap();
    assert_eq!(outcome.summary, Summary::Ok);
    assert_eq!(outcome.duration_us, Some(170));
    assert_eq!(record.round_trip(), Some(Duration::from_micros(500)));
    assert_eq!(
        harness.app.link.last_round_trip,
        Some(Duration::from_micros(500))
    );
    assert!(harness.app.session.editor.is_empty());
}

#[test]
fn an_unknown_parameter_is_refused_before_sending_and_its_key_is_underlined() {
    let mut harness = Harness::connected();
    let line = r#"/call solar.ping {"mesage": "hi"}"#;
    harness.run(line);
    assert!(harness.written().is_empty(), "something was sent");
    let band = harness.band().unwrap();
    assert_eq!(
        band.lines[0],
        "There is no parameter mesage. Did you mean message?"
    );
    assert_eq!(&line[band.underline.clone().unwrap()], r#""mesage""#);
    assert_eq!(
        harness.app.session.editor.text(),
        line,
        "the line was not kept"
    );
}

#[test]
fn json_that_does_not_parse_is_refused_with_the_place_it_stopped() {
    let mut harness = Harness::connected();
    let line = r#"/call solar.ping {"message": }"#;
    harness.run(line);
    assert!(harness.written().is_empty());
    let band = harness.band().unwrap();
    assert!(
        band.lines[0].starts_with("The parameters are not JSON: "),
        "{:?}",
        band.lines
    );
    assert_eq!(&line[band.underline.clone().unwrap()], "}");
}

#[test]
fn parameters_that_are_not_an_object_are_refused_with_the_contract_section() {
    let mut harness = Harness::connected();
    harness.run("/call solar.ping [1]");
    let band = harness.band().unwrap();
    assert!(band.lines[0].contains("section 3"));
}

#[test]
fn a_wrong_type_is_underlined_at_the_value() {
    let mut harness = Harness::connected();
    let line = r#"/call solar.ping {"message": 42}"#;
    harness.run(line);
    let band = harness.band().unwrap();
    assert_eq!(&line[band.underline.clone().unwrap()], "42");
    assert_eq!(
        band.lines[0],
        "/message should be a string or null, and an integer arrived."
    );
}

#[test]
fn a_call_to_an_api_the_manifest_does_not_have_is_sent_so_solar_can_answer() {
    let mut harness = Harness::connected();
    harness.run("/call solar.pnig");
    assert_eq!(harness.written()[0]["method"], json!("solar.pnig"));
}

#[test]
fn a_line_without_a_slash_is_not_run_and_the_band_says_what_was_meant() {
    let mut harness = Harness::connected();
    harness.run("solar.ping");
    assert!(harness.written().is_empty());
    assert_eq!(
        harness.band().unwrap().lines[0],
        "Commands start with a slash. Did you mean /call solar.ping?"
    );
}

#[test]
fn a_slash_typed_with_altgr_starts_a_command() {
    let mut harness = Harness::connected();
    harness.key(
        KeyCode::Char('/'),
        KeyModifiers::CONTROL | KeyModifiers::ALT,
    );
    harness.type_text("ping");
    harness.key(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(harness.written()[0]["method"], json!("solar.ping"));
}

#[test]
fn tab_completes_a_command_then_an_api_name() {
    let mut harness = Harness::connected();
    harness.type_text("/desc");
    harness.key(KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(harness.app.session.editor.text(), "/describe ");
    harness.type_text("solar.p");
    harness.key(KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(harness.app.session.editor.text(), "/describe solar.ping");
    assert!(harness.app.session.completion.is_none());
}

#[test]
fn tab_on_an_empty_line_goes_to_the_next_tab_and_a_question_mark_opens_the_help() {
    let mut harness = Harness::connected();
    harness.key(KeyCode::Char('?'), KeyModifiers::SHIFT);
    assert_eq!(harness.app.overlay, Some(Overlay::Help { scroll: 0 }));
    harness.key(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(harness.app.overlay, None);
    harness.key(KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(harness.app.tab, Tab::Apis);
    harness.key(KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(harness.app.tab, Tab::Session);
}

#[test]
fn ctrl_c_twice_quits_and_once_does_not() {
    let mut harness = Harness::connected();
    harness.key(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert!(!harness.app.quitting);
    assert_eq!(
        harness.app.flash.as_ref().map(|(_, text)| text.as_str()),
        Some("Press Ctrl+C again to quit.")
    );
    harness.key(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert!(harness.app.quitting);
    assert!(harness.effects.contains(&Effect::Quit));
}

#[test]
fn another_key_between_two_ctrl_c_disarms_the_second() {
    let mut harness = Harness::connected();
    harness.key(KeyCode::Char('c'), KeyModifiers::CONTROL);
    harness.type_text("x");
    harness.key(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert!(!harness.app.quitting);
}

#[test]
fn ctrl_c_on_a_call_in_flight_without_solar_cancel_says_how_long_it_may_take() {
    let mut harness = Harness::connected();
    harness.run("/call system.info");
    harness.key(KeyCode::Char('c'), KeyModifiers::CONTROL);
    let (_, text) = harness.app.flash.clone().unwrap();
    assert!(text.contains("offers no solar.cancel"), "{text}");
    assert!(text.contains("2000 ms"), "{text}");
}

#[test]
fn ctrl_c_on_a_call_in_flight_sends_solar_cancel_when_the_manifest_has_it() {
    let mut harness = Harness::new();
    harness.start();
    harness.answer(
        1,
        &success(1, "solar.version", &version_data("solar/1", "2.0.0")),
    );
    let mut manifest: Value = serde_json::from_str(MANIFEST).unwrap();
    manifest["apis"].as_array_mut().unwrap().push(json!({
        "name": "solar.cancel",
        "params_schema": {"type": "object", "properties": {"id": {}}, "required": ["id"]}
    }));
    harness.answer(2, &success(2, "solar.manifest", &manifest));
    assert!(harness.app.link.capabilities.cancel);
    harness.effects.clear();
    harness.run("/call system.info");
    let slow = harness.written()[0]["id"].as_u64().unwrap();
    harness.key(KeyCode::Char('c'), KeyModifiers::CONTROL);
    let written = harness.written();
    assert_eq!(written[1]["method"], json!("solar.cancel"));
    assert_eq!(written[1]["params"], json!({"id": slow}));
}

#[test]
fn solar_dying_after_connecting_closes_what_was_waiting_and_ctrl_r_starts_again() {
    let mut harness = Harness::connected();
    harness.run("/ping");
    harness.feed(Incoming::Connection {
        generation: 1,
        event: ConnectionEvent::OutputClosed {
            error: None,
            at: harness.now,
        },
    });
    assert!(
        harness
            .effects
            .contains(&Effect::CheckExit { generation: 1 })
    );
    harness.feed(Incoming::Connection {
        generation: 1,
        event: ConnectionEvent::StderrClosed,
    });
    harness.feed(Incoming::Exited {
        generation: 1,
        how: "exited with code 101".to_owned(),
    });
    let failure = harness.app.link.failure.clone().unwrap();
    assert!(
        failure
            .what()
            .starts_with("SOLAR exited with code 101 after "),
        "{}",
        failure.what()
    );
    let (_, record) = harness.app.history.calls.last().unwrap();
    assert_eq!(
        record.closed.as_deref(),
        Some("the connection ended before SOLAR answered")
    );
    harness.effects.clear();
    harness.key(KeyCode::Char('r'), KeyModifiers::CONTROL);
    assert!(harness.effects.contains(&Effect::Stop { generation: 1 }));
    assert!(matches!(
        harness.effects.last(),
        Some(Effect::Start { generation: 2, .. })
    ));
}

/// SOLAR's output closes and its exit is seen, and its standard error is not read to its
/// end yet.
fn exit_before_the_last_words(harness: &mut Harness) {
    harness.feed(Incoming::Connection {
        generation: 1,
        event: ConnectionEvent::OutputClosed {
            error: None,
            at: harness.now,
        },
    });
    harness.feed(Incoming::Exited {
        generation: 1,
        how: "exited with code 3".to_owned(),
    });
}

#[test]
fn an_exit_seen_before_the_last_words_waits_for_them() {
    // Found flaky on Linux in CI: the exit was reported before the thread that reads
    // standard error had delivered the line that said why SOLAR ended.
    let mut harness = Harness::connected();
    exit_before_the_last_words(&mut harness);
    assert!(harness.app.link.failure.is_none());
    assert!(
        harness
            .app
            .next_deadline()
            .is_some_and(|deadline| deadline <= harness.now + link::LAST_WORDS_GRACE)
    );
    for event in [
        ConnectionEvent::Stderr {
            text: "solar: exiting with 3".to_owned(),
            cut: 0,
            at: harness.now,
        },
        ConnectionEvent::StderrClosed,
    ] {
        harness.feed(Incoming::Connection {
            generation: 1,
            event,
        });
    }
    let failure = harness.app.link.failure.clone().unwrap();
    assert!(failure.what().starts_with("SOLAR exited with code 3"));
    assert!(
        failure
            .last_words()
            .iter()
            .any(|line| line.contains("exiting with 3"))
    );
    assert_eq!(harness.app.link.phase, Phase::Down);
}

#[test]
fn an_exit_is_reported_after_a_moment_when_standard_error_stays_open() {
    let mut harness = Harness::connected();
    exit_before_the_last_words(&mut harness);
    harness.later(link::LAST_WORDS_GRACE);
    harness.feed(Incoming::Tick);
    let failure = harness.app.link.failure.clone().unwrap();
    assert!(failure.what().starts_with("SOLAR exited with code 3"));
}

#[test]
fn events_of_an_older_connection_are_ignored() {
    let mut harness = Harness::connected();
    harness.key(KeyCode::Char('r'), KeyModifiers::CONTROL);
    harness.feed(Incoming::Connection {
        generation: 1,
        event: ConnectionEvent::OutputClosed {
            error: None,
            at: harness.now,
        },
    });
    assert!(!harness.app.link.output_closed);
}

#[test]
fn an_example_runs_with_one_key_and_is_judged_against_what_it_declares() {
    let mut harness = Harness::connected();
    harness.key(KeyCode::Tab, KeyModifiers::NONE);
    // solar.ping is the third API of the manifest.
    harness.key(KeyCode::Down, KeyModifiers::NONE);
    harness.key(KeyCode::Down, KeyModifiers::NONE);
    harness.key(KeyCode::Char('2'), KeyModifiers::NONE);
    let written = harness.written();
    assert_eq!(written[0]["method"], json!("solar.ping"));
    assert_eq!(written[0]["params"], json!({"message": "hi"}));
    let id = written[0]["id"].as_u64().unwrap();
    harness.answer(
        id,
        &success(
            id,
            "solar.ping",
            &json!({"echo": "hi", "pong": true, "received_at": "x"}),
        ),
    );
    assert!(matches!(
        harness.app.apis.results.get(&("solar.ping".to_owned(), 1)),
        Some(ExampleResult::Matches { .. })
    ));
}

#[test]
fn an_example_whose_answer_differs_says_where() {
    let mut harness = Harness::connected();
    harness.key(KeyCode::Tab, KeyModifiers::NONE);
    harness.key(KeyCode::Down, KeyModifiers::NONE);
    harness.key(KeyCode::Down, KeyModifiers::NONE);
    harness.key(KeyCode::Char('2'), KeyModifiers::NONE);
    let id = harness.written()[0]["id"].as_u64().unwrap();
    harness.answer(
        id,
        &success(
            id,
            "solar.ping",
            &json!({"echo": null, "pong": true, "received_at": "x"}),
        ),
    );
    let Some(ExampleResult::Differs { sentence }) =
        harness.app.apis.results.get(&("solar.ping".to_owned(), 1))
    else {
        panic!("the difference was not seen");
    };
    assert_eq!(sentence, "differs at /echo: expected \"hi\", received null");
}

#[test]
fn the_form_builds_the_call_and_refuses_a_missing_required_field() {
    let mut harness = Harness::connected();
    harness.key(KeyCode::Tab, KeyModifiers::NONE);
    // solar.describe is the first API.
    harness.key(KeyCode::Enter, KeyModifiers::NONE);
    assert!(harness.app.apis.form.is_some());
    harness.key(KeyCode::Enter, KeyModifiers::NONE);
    assert!(harness.written().is_empty());
    assert_eq!(
        harness.app.apis.form.as_ref().unwrap().errors,
        vec![(Some(0), "api is required.".to_owned())]
    );
    harness.type_text("solar.ping");
    harness.key(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(harness.written()[0]["params"], json!({"api": "solar.ping"}));
    assert!(harness.app.apis.form.is_none());
}

#[test]
fn the_log_keeps_standard_error_and_filters_it_by_level() {
    let mut harness = Harness::connected();
    for line in [
        r#"{"level":"trace","message":"--> {}","time":"t"}"#,
        r#"{"level":"info","message":"the session ended after 1 calls","time":"t"}"#,
        "thread 'main' panicked",
    ] {
        harness.feed(Incoming::Connection {
            generation: 1,
            event: ConnectionEvent::Stderr {
                text: line.to_owned(),
                cut: 0,
                at: harness.now,
            },
        });
    }
    assert_eq!(harness.app.log.entries.len(), 3);
    assert_eq!(harness.app.log.visible().len(), 2);
    harness.app.tab = Tab::Log;
    harness.key(KeyCode::Char('t'), KeyModifiers::NONE);
    assert_eq!(harness.app.log.visible().len(), 3);
}

#[test]
fn a_raw_line_is_sent_exactly_and_its_answer_is_matched() {
    let mut harness = Harness::connected();
    harness.run("/raw {not json");
    let Effect::Write { line, .. } = harness.effects.last().unwrap() else {
        panic!("nothing was written");
    };
    assert_eq!(line, "{not json");
    let refusal = json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": "Parse error.",
        "data": {"status": "INVALID_ARGUMENT", "reason": "PARSE_ERROR",
                 "details": [{"field": null, "expected": "a JSON object", "received": "{not json",
                              "hint": null, "docs": "docs/ERRORS.md#invalid_argument"}],
                 "meta": meta(&Value::Null, "")}}});
    harness.answer(0, &refusal.to_string());
    let (_, record) = harness.app.history.calls.last().unwrap();
    assert_eq!(
        record.outcome.as_ref().unwrap().summary,
        Summary::Error {
            status: "INVALID_ARGUMENT".to_owned(),
            reason: "PARSE_ERROR".to_owned()
        }
    );
}

#[test]
fn an_error_from_solar_is_recorded_with_its_status_and_reason() {
    let mut harness = Harness::connected();
    harness.run("/describe solar.pign");
    let id = harness.written()[0]["id"].as_u64().unwrap();
    harness.answer(
        id,
        &failure(
            id,
            "solar.describe",
            "NOT_FOUND",
            -32601,
            "API_NOT_FOUND",
            "No API is registered.",
        ),
    );
    let (_, record) = harness.app.history.calls.last().unwrap();
    assert!(matches!(
        &record.outcome.as_ref().unwrap().summary,
        Summary::Error { reason, .. } if reason == "API_NOT_FOUND"
    ));
}

#[test]
fn a_response_nobody_waited_for_is_shown_as_unexpected() {
    let mut harness = Harness::connected();
    harness.answer(99, &success(99, "solar.ping", &json!({})));
    assert!(
        harness
            .app
            .session
            .transcript
            .iter()
            .any(|(_, entry)| matches!(entry, Entry::Unexpected { .. }))
    );
}

#[test]
fn an_idle_zenith_connected_for_minutes_wakes_once_a_minute() {
    let mut harness = Harness::connected();
    harness.later(Duration::from_secs(125));
    harness.feed(Incoming::Tick);
    let deadline = harness.app.next_deadline().unwrap();
    let wait = deadline.saturating_duration_since(harness.now);
    assert!(
        wait > Duration::from_secs(1) && wait <= Duration::from_mins(1),
        "{wait:?}"
    );
}

#[test]
fn the_export_and_the_report_are_asked_for_and_the_report_asks_solar_first() {
    let mut harness = Harness::connected();
    harness.run("/export");
    assert!(harness.effects.contains(&Effect::Export { path: None }));
    harness.effects.clear();
    harness.run("/report out.ndjson");
    let written = harness.written();
    assert_eq!(written[0]["method"], json!("system.info"));
    let id = written[0]["id"].as_u64().unwrap();
    harness.answer(id, &success(id, "system.info", &json!({"os": "windows"})));
    assert!(harness.effects.iter().any(|effect| matches!(
        effect,
        Effect::Report { path: Some(path), system: Some(_), .. } if path == &PathBuf::from("out.ndjson")
    )));
}

#[test]
fn the_report_names_the_versions_the_system_the_log_and_the_calls() {
    let mut harness = Harness::connected();
    harness.feed(Incoming::Connection {
        generation: 1,
        event: ConnectionEvent::Stderr {
            text: r#"{"level":"info","message":"hello"}"#.to_owned(),
            cut: 0,
            at: harness.now,
        },
    });
    let mut out = Vec::new();
    let calls = report::write(
        &harness.app,
        &mut out,
        Some(&json!({"os": "windows"})),
        None,
        wall(),
        |name| (name == "TERM").then(|| "xterm-256color".to_owned()),
    )
    .unwrap();
    assert_eq!(calls, 2);
    let lines: Vec<Value> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let kinds: Vec<&str> = lines
        .iter()
        .map(|line| line["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        vec![
            "header", "zenith", "solar", "system", "log", "dropped", "call", "call"
        ]
    );
    assert_eq!(lines[1]["environment"]["TERM"], json!("xterm-256color"));
    assert_eq!(lines[1]["environment"]["NO_COLOR"], Value::Null);
    assert_eq!(lines[2]["version"]["protocol"], json!("solar/1"));
    assert_eq!(lines[3]["data"]["os"], json!("windows"));
}

#[test]
fn twenty_thousand_calls_never_hold_more_than_the_declared_limits() {
    let mut harness = Harness::connected();
    for _ in 0..20_000 {
        harness.run("/ping");
        let id = harness
            .app
            .link
            .tracker
            .waiting()
            .last()
            .map(|waiting| waiting.call)
            .unwrap();
        harness.answer(id, &success(id, "solar.ping", &json!({"pong": true})));
        harness.effects.clear();
        assert!(harness.app.history.calls.len() <= crate::limits::HISTORY_ENTRIES);
        assert!(harness.app.history.calls.bytes() <= crate::limits::HISTORY_BYTES);
        assert!(harness.app.session.transcript.len() <= crate::limits::TRANSCRIPT_ENTRIES);
    }
    assert_eq!(
        harness.app.history.calls.len(),
        crate::limits::HISTORY_ENTRIES
    );
    assert!(harness.app.history.calls.dropped() > 0);
    assert_eq!(harness.app.link.tracker.waiting().len(), 0);
}

/// Draws the application at 80 × 24 and returns the characters of the screen.
fn screen_text(app: &App) -> String {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
    terminal.draw(|frame| crate::ui::draw(frame, app)).unwrap();
    let buffer = terminal.backend().buffer().clone();
    buffer
        .content()
        .chunks(80)
        .map(|row| {
            row.iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `Tab` on an empty command line, until `tab` is the one on screen.
fn go_to_tab(harness: &mut Harness, tab: Tab) {
    for _ in 0..4 {
        if harness.app.tab == tab {
            return;
        }
        harness.key(KeyCode::Tab, KeyModifiers::NONE);
    }
    assert_eq!(harness.app.tab, tab);
}

/// Moves the selection of the APIs tab to `name`, from the top.
fn select_api(harness: &mut Harness, name: &str) {
    for _ in 0..64 {
        if harness.app.selected_api().map(|api| api.name.as_str()) == Some(name) {
            return;
        }
        harness.key(KeyCode::Down, KeyModifiers::NONE);
    }
    panic!(
        "{name} is not in the APIs tab: tab {:?}, selected {}, catalogue {}, filter {:?}",
        harness.app.tab,
        harness.app.apis.selected,
        harness.app.catalogue().is_some(),
        harness.app.apis.filter.text()
    );
}

/// SOLAR's output and standard error close, and it exits.
fn solar_goes_down(harness: &mut Harness) {
    for event in [
        ConnectionEvent::OutputClosed {
            error: None,
            at: harness.now,
        },
        ConnectionEvent::StderrClosed,
    ] {
        harness.feed(Incoming::Connection {
            generation: 1,
            event,
        });
    }
    harness.feed(Incoming::Exited {
        generation: 1,
        how: "exited with code 0".to_owned(),
    });
    assert_eq!(harness.app.link.phase, Phase::Down);
    harness.effects.clear();
}

#[test]
fn the_log_moves_by_the_page_shows_a_line_whole_and_clears() {
    let mut harness = Harness::connected();
    harness.feed(Incoming::Resize {
        width: 80,
        height: 24,
    });
    for index in 0..40 {
        harness.feed(Incoming::Connection {
            generation: 1,
            event: ConnectionEvent::Stderr {
                text: format!(
                    r#"{{"level":"info","message":"line {index} of the log","time":"t"}}"#
                ),
                cut: 0,
                at: harness.now,
            },
        });
    }
    go_to_tab(&mut harness, Tab::Log);
    let visible = harness.app.log.visible();
    harness.key(KeyCode::Char('g'), KeyModifiers::NONE);
    assert_eq!(harness.app.log.selected, visible.first().copied());
    harness.key(KeyCode::PageDown, KeyModifiers::NONE);
    let paged = harness.app.log.selected.unwrap();
    assert!(paged > visible[0], "PageDown stayed at {paged}");
    harness.key(KeyCode::Up, KeyModifiers::NONE);
    assert!(harness.app.log.selected.unwrap() < paged);
    harness.key(KeyCode::Enter, KeyModifiers::NONE);
    assert!(harness.app.log.expanded);
    let screen = screen_text(&harness.app);
    assert!(screen.contains("of the log"), "{screen}");
    harness.key(KeyCode::Esc, KeyModifiers::NONE);
    assert!(!harness.app.log.expanded);
    harness.key(KeyCode::PageDown, KeyModifiers::NONE);
    harness.key(KeyCode::PageDown, KeyModifiers::NONE);
    assert_eq!(
        harness.app.log.selected, None,
        "the end of the Log follows it again"
    );
    harness.key(KeyCode::Char('c'), KeyModifiers::NONE);
    assert!(harness.app.log.visible().is_empty());
}

#[test]
fn a_call_of_the_history_is_sent_again_or_put_back_on_the_command_line() {
    let mut harness = Harness::connected();
    harness.run(r#"/call solar.ping {"message": "again"}"#);
    let sent = harness.written()[0]["id"].as_u64().unwrap();
    harness.answer(
        sent,
        &success(sent, "solar.ping", &json!({"pong": true, "echo": "again"})),
    );
    harness.run(r#"/raw {"jsonrpc":"2.0","id":"by hand","method":"solar.version"}"#);
    harness.answer(
        0,
        &success(0, "solar.version", &json!({})).replace("0,", "\"by hand\","),
    );
    harness.effects.clear();

    go_to_tab(&mut harness, Tab::History);
    // The newest call is the raw one, which goes back as it was typed.
    harness.key(KeyCode::Char('e'), KeyModifiers::NONE);
    assert_eq!(harness.app.tab, Tab::Session);
    assert_eq!(
        harness.app.session.editor.text(),
        r#"/raw {"jsonrpc":"2.0","id":"by hand","method":"solar.version"}"#
    );
    harness.app.session.editor.clear();

    // The call before it is sent again, with the same parameters.
    go_to_tab(&mut harness, Tab::History);
    harness.key(KeyCode::Up, KeyModifiers::NONE);
    harness.key(KeyCode::Char('r'), KeyModifiers::NONE);
    let written = harness.written();
    assert_eq!(written.len(), 1);
    assert_eq!(written[0]["method"], json!("solar.ping"));
    assert_eq!(written[0]["params"], json!({"message": "again"}));
    harness.key(KeyCode::Char('e'), KeyModifiers::NONE);
    assert_eq!(
        harness.app.session.editor.text(),
        r#"/call solar.ping {"message":"again"}"#
    );
}

#[test]
fn the_form_moves_between_its_fields_and_along_the_text_of_one() {
    let mut harness = Harness::connected();
    go_to_tab(&mut harness, Tab::Apis);
    harness.key(KeyCode::Char('g'), KeyModifiers::NONE);
    select_api(&mut harness, "solar.ping");
    harness.key(KeyCode::Enter, KeyModifiers::NONE);
    let fields = harness.app.apis.form.as_ref().unwrap().fields.len();
    assert_eq!(fields, 1, "solar.ping takes one parameter");
    harness.key(KeyCode::Down, KeyModifiers::NONE);
    harness.key(KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(harness.app.apis.form.as_ref().unwrap().focus, 0);
    harness.type_text("ac");
    harness.key(KeyCode::Left, KeyModifiers::NONE);
    harness.type_text("b");
    harness.key(KeyCode::Right, KeyModifiers::NONE);
    harness.type_text("d");
    let form = harness.app.apis.form.as_ref().unwrap();
    assert_eq!(form.fields[0].editor.text(), "abcd");
}

#[test]
fn nothing_is_sent_while_solar_is_down_and_both_places_say_so() {
    let mut harness = Harness::connected();
    solar_goes_down(&mut harness);
    let sentence = "SOLAR is not connected, so nothing was sent. Ctrl+R starts it.";

    harness.run("/ping");
    assert!(harness.written().is_empty());
    assert!(
        harness
            .band()
            .is_some_and(|band| band.lines.iter().any(|line| line == sentence)),
        "{:?}",
        harness.band()
    );
    assert_eq!(
        harness.app.session.editor.text(),
        "/ping",
        "kept, to be run again"
    );

    harness.key(KeyCode::Char('u'), KeyModifiers::CONTROL);
    go_to_tab(&mut harness, Tab::Apis);
    harness.key(KeyCode::Char('g'), KeyModifiers::NONE);
    select_api(&mut harness, "solar.ping");
    harness.key(KeyCode::Enter, KeyModifiers::NONE);
    harness.key(KeyCode::Enter, KeyModifiers::NONE);
    assert!(harness.written().is_empty());
    let errors = &harness.app.apis.form.as_ref().unwrap().errors;
    assert!(
        errors.iter().any(|(_, error)| error == sentence),
        "{errors:?}"
    );
}
