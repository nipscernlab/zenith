//! The client against a real SOLAR, and against ZENITH's test double of one.
//!
//! The tests that need SOLAR find it the way ZENITH does: `ZENITH_SOLAR`, then the `PATH`.
//! When there is none they say so and pass, unless `ZENITH_REQUIRE_SOLAR` is set, which
//! CI sets, because in CI a skipped test against SOLAR is a failure.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    reason = "a test reports failure by panicking, and says on standard error what it skipped"
)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use zenith_client::calls::{Answer, Tracker};
use zenith_client::connection::{Connection, Event, Settings, describe_exit};
use zenith_client::envelope::{Body, Message, read};
use zenith_client::example::compare;
use zenith_client::handshake::{HandshakeError, check_version};
use zenith_client::locate::{Found, Origin, Placement, Search, locate, prepare};
use zenith_client::manifest::Catalogue;

const WAIT: Duration = Duration::from_secs(20);

struct Session {
    connection: Connection,
    events: Receiver<(u64, Event)>,
    tracker: Tracker,
    stderr: Vec<String>,
}

impl Session {
    fn start(path: PathBuf, environment: &[(&str, &str)]) -> Self {
        let found = Found {
            path,
            origin: Origin::Flag,
        };
        let prepared = prepare(found, &Placement::for_this_system()).unwrap();
        let (sender, events) = mpsc::sync_channel(1024);
        let sink = Arc::new(move |generation, event| sender.send((generation, event)).is_ok());
        let settings = Settings {
            environment: environment
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
            ..Settings::default()
        };
        let connection = Connection::start(prepared, &settings, 7, sink).unwrap();
        Self {
            connection,
            events,
            tracker: Tracker::new(256),
            stderr: Vec::new(),
        }
    }

    fn call(&mut self, method: &str, params: &Value) -> u64 {
        let (id, line) = self.tracker.call(method, params, Instant::now()).unwrap();
        self.connection.send(line).unwrap();
        id
    }

    fn raw(&mut self, line: &str) -> u64 {
        let call = self.tracker.raw(line, Instant::now()).unwrap();
        self.connection.send(line.to_owned()).unwrap();
        call
    }

    /// The next event that is not a line of standard error, which is kept.
    fn next(&mut self) -> Event {
        let deadline = Instant::now() + WAIT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let (generation, event) = self
                .events
                .recv_timeout(left)
                .expect("SOLAR said nothing in time");
            assert_eq!(
                generation, 7,
                "an event carried another connection's generation"
            );
            match event {
                Event::Stderr { text, .. } => self.stderr.push(text),
                Event::StderrClosed => {}
                other => return other,
            }
        }
    }

    fn line(&mut self) -> String {
        match self.next() {
            Event::Line {
                text, invalid_utf8, ..
            } => {
                assert!(!invalid_utf8, "SOLAR wrote bytes that are not UTF-8");
                text
            }
            other => panic!("expected a line, got {other:?}"),
        }
    }

    /// The response to a call, matched by the tracker.
    fn answer(&mut self) -> (u64, Message) {
        let message = read(&self.line());
        match self.tracker.answer(&message) {
            Answer::Call { waiting, .. } => (waiting.call, message),
            Answer::Unexpected => panic!("a response nobody waited for: {message:?}"),
        }
    }

    fn data(&mut self, method: &str, params: &Value) -> Value {
        let id = self.call(method, params);
        let (answered, message) = self.answer();
        assert_eq!(answered, id);
        let Message::Single(envelope) = message else {
            panic!("not a single response");
        };
        assert!(envelope.violations.is_empty(), "{:?}", envelope.violations);
        let Body::Success { data, .. } = envelope.body else {
            panic!("{method} failed: {:?}", envelope.body);
        };
        data
    }

    fn handshake(&mut self) -> Catalogue {
        let version = self.data("solar.version", &json!({}));
        check_version(&version).unwrap();
        let manifest = self.data("solar.manifest", &json!({}));
        Catalogue::from_data(&manifest).unwrap()
    }
}

fn solar_under_test() -> Option<PathBuf> {
    match locate(&Search::from_environment(None)) {
        Ok(found) => Some(found.path),
        Err(error) => {
            assert!(
                std::env::var_os("ZENITH_REQUIRE_SOLAR").is_none(),
                "ZENITH_REQUIRE_SOLAR is set and SOLAR is not there: {error}"
            );
            eprintln!("skipped: {error} Set ZENITH_SOLAR to the solar binary to run this test.");
            None
        }
    }
}

fn double() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_zenith-solar-double"))
}

#[test]
fn the_handshake_with_the_installed_solar_passes_its_checks() {
    let Some(solar) = solar_under_test() else {
        return;
    };
    let mut session = Session::start(solar.clone(), &[]);
    let catalogue = session.handshake();
    assert!(catalogue.api("solar.ping").is_some());
    let prepared = session.connection.prepared();
    if cfg!(windows) {
        assert_ne!(
            prepared.runs, solar,
            "on Windows SOLAR must run from a copy"
        );
        assert!(
            prepared
                .runs
                .starts_with(std::env::temp_dir().join("zenith"))
        );
        assert!(prepared.sha256.is_some());
    } else {
        assert_eq!(prepared.runs, solar);
    }
}

#[test]
fn every_example_of_every_api_of_the_installed_solar_returns_what_it_declares() {
    let Some(solar) = solar_under_test() else {
        return;
    };
    let mut session = Session::start(solar, &[]);
    let catalogue = session.handshake();
    let mut ran = 0;
    for api in &catalogue.apis {
        for example in &api.examples {
            let report = api.params().validate(&example.params);
            assert!(
                report.is_valid(),
                "{} example {} does not pass ZENITH's validation: {:?}",
                api.name,
                example.name,
                report.failures
            );
            let data = session.data(&api.name, &example.params);
            if let Err(difference) = compare(&example.response, &data, &example.matching) {
                panic!(
                    "{} example {} {}",
                    api.name,
                    example.name,
                    difference.sentence()
                );
            }
            ran += 1;
        }
    }
    assert!(
        ran >= catalogue.apis.len(),
        "every API has at least one example"
    );
}

#[test]
fn solar_writes_its_trace_to_standard_error_as_json() {
    let Some(solar) = solar_under_test() else {
        return;
    };
    let mut session = Session::start(solar, &[]);
    session.data("solar.ping", &json!({"message": "log me"}));
    session.connection.close_input();
    while !matches!(session.next(), Event::OutputClosed { .. }) {}
    let traced = session.stderr.iter().any(|line| {
        serde_json::from_str::<Value>(line).is_ok_and(|entry| {
            entry["level"] == json!("trace")
                && entry["message"]
                    .as_str()
                    .is_some_and(|m| m.contains("log me"))
        })
    });
    assert!(traced, "no trace line named the ping: {:?}", session.stderr);
}

#[test]
fn a_batch_sent_by_hand_is_answered_as_one_line() {
    let Some(solar) = solar_under_test() else {
        return;
    };
    let mut session = Session::start(solar, &[]);
    let batch = session.raw(r#"[{"jsonrpc":"2.0","id":"a","method":"solar.ping"}]"#);
    let (answered, message) = session.answer();
    assert_eq!(answered, batch);
    match message {
        // A SOLAR with batches, section 3.2 of its draft contract.
        Message::Batch(items) => assert_eq!(items.len(), 1),
        // A SOLAR without them, section 3.1 of its published contract.
        Message::Single(envelope) => assert_eq!(envelope.status(), Some("UNIMPLEMENTED")),
        Message::NotJson { error } => panic!("SOLAR answered a batch with {error}"),
    }
}

#[test]
fn a_line_that_is_not_json_is_answered_with_a_null_id_and_matched_to_it() {
    let Some(solar) = solar_under_test() else {
        return;
    };
    let mut session = Session::start(solar, &[]);
    let broken = session.raw("{this is not json");
    let (answered, message) = session.answer();
    assert_eq!(answered, broken);
    let Message::Single(envelope) = message else {
        panic!("not a single response");
    };
    assert!(envelope.id.is_null());
    assert_eq!(envelope.status(), Some("INVALID_ARGUMENT"));
    assert!(envelope.violations.is_empty(), "{:?}", envelope.violations);
}

#[test]
fn on_windows_the_copy_is_what_answers() {
    let Some(solar) = solar_under_test() else {
        return;
    };
    let mut session = Session::start(solar, &[]);
    let info = session.data("system.info", &json!({}));
    let reported = PathBuf::from(info["executable"].as_str().unwrap());
    let runs = session.connection.prepared().runs.clone();
    assert_eq!(
        std::fs::canonicalize(reported).unwrap(),
        std::fs::canonicalize(runs).unwrap()
    );
}

#[test]
fn closing_the_input_ends_the_session_cleanly() {
    let Some(solar) = solar_under_test() else {
        return;
    };
    let session = Session::start(solar, &[]);
    let status = session.connection.shutdown(Duration::from_secs(5)).unwrap();
    assert!(status.success(), "{}", describe_exit(status));
}

#[test]
fn a_solar_that_exits_at_once_is_seen_to_exit_with_its_code_and_its_last_words() {
    let mut session = Session::start(double(), &[("ZENITH_DOUBLE", "exit:3")]);
    assert!(matches!(
        session.next(),
        Event::OutputClosed { error: None, .. }
    ));
    let deadline = Instant::now() + WAIT;
    let status = loop {
        if let Some(status) = session.connection.try_exit().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "the double did not exit");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(describe_exit(status), "exited with code 3");
    while let Ok((_, event)) = session.events.recv_timeout(Duration::from_secs(2)) {
        if let Event::Stderr { text, .. } = event {
            session.stderr.push(text);
        }
    }
    assert!(
        session
            .stderr
            .iter()
            .any(|line| line.contains("exiting with 3"))
    );
}

#[test]
fn another_protocol_is_refused_with_both_versions() {
    let mut session = Session::start(double(), &[("ZENITH_DOUBLE", "protocol:solar/2")]);
    let version = session.data("solar.version", &json!({}));
    let error = check_version(&version).unwrap_err();
    assert!(matches!(error, HandshakeError::Protocol { .. }));
    assert!(error.to_string().contains("0.0.0-double speaks solar/2"));
}

#[test]
fn another_manifest_layout_is_refused() {
    let mut session = Session::start(double(), &[("ZENITH_DOUBLE", "layout:3.1.0")]);
    let version = session.data("solar.version", &json!({}));
    assert!(matches!(
        check_version(&version),
        Err(HandshakeError::Layout { .. })
    ));
}

#[test]
fn a_line_that_is_not_json_answers_nothing() {
    let mut session = Session::start(double(), &[("ZENITH_DOUBLE", "garbage")]);
    session.call("solar.ping", &json!({}));
    let message = read(&session.line());
    assert!(matches!(message, Message::NotJson { .. }));
    assert_eq!(session.tracker.answer(&message), Answer::Unexpected);
    assert_eq!(session.tracker.waiting().len(), 1);
}

#[test]
fn a_response_too_long_to_keep_is_dropped_and_still_matched_by_its_id() {
    let mut session = Session::start(double(), &[("ZENITH_DOUBLE", "huge")]);
    let id = session.call("solar.ping", &json!({}));
    let Event::LineTooLong {
        bytes, id: found, ..
    } = session.next()
    else {
        panic!("the long line was not reported as too long");
    };
    assert!(bytes > 16 * 1024 * 1024);
    assert!(found.unwrap().is(id));
}

#[test]
fn a_cancellation_is_answered_before_the_call_it_cancels() {
    let mut session = Session::start(double(), &[("ZENITH_DOUBLE", "cancel")]);
    let catalogue = session.handshake();
    assert_eq!(
        catalogue.capabilities().cancel.as_deref(),
        Some("solar.cancel")
    );
    let slow = session.call("double.slow", &json!({"ms": 2000}));
    std::thread::sleep(Duration::from_millis(100));
    let cancel = session.call("solar.cancel", &json!({"id": slow}));
    let (first, message) = session.answer();
    assert_eq!(first, cancel, "the cancellation was not answered first");
    let Message::Single(envelope) = message else {
        panic!("not a single response");
    };
    let Body::Success { data, .. } = envelope.body else {
        panic!("the cancellation failed");
    };
    assert_eq!(data["outcome"], json!("cancellation_requested"));
    let (second, message) = session.answer();
    assert_eq!(second, slow);
    let Message::Single(envelope) = message else {
        panic!("not a single response");
    };
    assert_eq!(envelope.status(), Some("CANCELLED"));
    assert!(envelope.violations.is_empty(), "{:?}", envelope.violations);
}

#[test]
fn a_solar_that_dies_after_connecting_closes_its_output_and_exits_101() {
    let mut session = Session::start(double(), &[("ZENITH_DOUBLE", "die-after:2")]);
    session.handshake();
    assert!(matches!(session.next(), Event::OutputClosed { .. }));
    let deadline = Instant::now() + WAIT;
    loop {
        if let Some(status) = session.connection.try_exit().unwrap() {
            assert_eq!(status.code(), Some(101));
            break;
        }
        assert!(Instant::now() < deadline, "the double did not exit");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_silent_solar_answers_nothing_and_the_call_stays_waiting() {
    let mut session = Session::start(double(), &[("ZENITH_DOUBLE", "silent")]);
    session.call("solar.ping", &json!({}));
    let quiet = loop {
        match session.events.recv_timeout(Duration::from_millis(300)) {
            Ok((_, Event::Stderr { .. })) => {}
            Ok((_, other)) => break Some(other),
            Err(_) => break None,
        }
    };
    assert!(quiet.is_none(), "the silent double said {quiet:?}");
    assert_eq!(session.tracker.waiting().len(), 1);
}
