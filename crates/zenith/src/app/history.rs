//! The History: every request and response, with its timing, in a ring buffer.

use std::io::{self, Write};
use std::time::{Instant, SystemTime};

use serde_json::{Value, json};
use zenith_client::envelope::{Body, Message};
use zenith_client::handshake::PROTOCOL;

use crate::clock::Utc;
use crate::limits;
use crate::ring::{Measured, Ring};

/// Why a call was made, which decides what happens when it is answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// The two calls of the handshake.
    Handshake,
    /// A command typed on the command line.
    Command,
    /// An example run from the APIs tab.
    Example {
        /// The API.
        api: String,
        /// Which example, from zero.
        index: usize,
    },
    /// The parameter form of the APIs tab.
    Form,
    /// A line sent with `/raw`.
    Raw,
    /// `solar.cancel`, sent by `Ctrl+C`.
    Cancel,
    /// A request sent again from the History.
    Again,
    /// The `system.info` a `/report` asks for.
    Report {
        /// Where the report goes.
        path: std::path::PathBuf,
    },
    /// `R` on the APIs tab.
    Reload,
    /// A key of the Log tab, asking SOLAR to log at another level.
    LogLevel,
}

impl Origin {
    /// The word the export writes.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Self::Handshake => "handshake",
            Self::Command => "command",
            Self::Example { .. } => "example",
            Self::Form => "form",
            Self::Raw => "raw",
            Self::Cancel => "cancel",
            Self::Again => "again",
            Self::Report { .. } => "report",
            Self::Reload => "reload",
            Self::LogLevel => "log_level",
        }
    }
}

/// What a response said, in the few words the lists show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Summary {
    /// A result.
    Ok,
    /// An error, with its status and reason.
    Error {
        /// `error.data.status`.
        status: String,
        /// `error.data.reason`.
        reason: String,
    },
    /// A batch of responses.
    Batch {
        /// How many.
        count: usize,
    },
    /// A line that is not JSON.
    NotJson,
    /// A line too long to keep.
    TooLong {
        /// Its length.
        bytes: usize,
    },
}

impl Summary {
    /// The summary of a message.
    #[must_use]
    pub fn of(message: &Message) -> Self {
        match message {
            Message::Single(envelope) => match &envelope.body {
                Body::Success { .. } => Self::Ok,
                Body::Failure(failure) => Self::Error {
                    status: failure.status.clone().unwrap_or_else(|| "?".to_owned()),
                    reason: failure.reason.clone().unwrap_or_else(|| "?".to_owned()),
                },
                Body::Unrecognised => Self::Error {
                    status: "?".to_owned(),
                    reason: "not a response".to_owned(),
                },
            },
            Message::Batch(items) => Self::Batch { count: items.len() },
            Message::NotJson { .. } => Self::NotJson,
        }
    }

    /// Whether this is a success.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok | Self::Batch { .. })
    }
}

/// How a call ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// The response line, or `None` when it was too long to keep.
    pub line: Option<String>,
    /// When it was read.
    pub received: Instant,
    /// The same moment on the wall clock.
    pub received_wall: SystemTime,
    /// What it said.
    pub summary: Summary,
    /// SOLAR's own time, `meta.duration_us`.
    pub duration_us: Option<u64>,
    /// How many contract violations it has.
    pub violations: usize,
    /// Whether it was matched by order rather than by id.
    pub by_order: bool,
}

/// One call: its request, and its response once there is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallRecord {
    /// The connection it was made on.
    pub connection: u64,
    /// Its number on that connection, which is its id when ZENITH built it.
    pub call: u64,
    /// The method.
    pub method: Option<String>,
    /// Why it was made.
    pub origin: Origin,
    /// The request line as written.
    pub request: String,
    /// When it was queued.
    pub sent: Instant,
    /// The same moment on the wall clock.
    pub sent_wall: SystemTime,
    /// How it ended.
    pub outcome: Option<Outcome>,
    /// Why it ended without a response, when it did.
    pub closed: Option<String>,
}

impl CallRecord {
    /// The round trip, when there was a response.
    #[must_use]
    pub fn round_trip(&self) -> Option<std::time::Duration> {
        self.outcome
            .as_ref()
            .map(|outcome| outcome.received.saturating_duration_since(self.sent))
    }

    /// Whether it is still waiting.
    #[must_use]
    pub fn waiting(&self) -> bool {
        self.outcome.is_none() && self.closed.is_none()
    }

    /// The request parsed, when it is JSON.
    #[must_use]
    pub fn request_value(&self) -> Option<Value> {
        serde_json::from_str(&self.request).ok()
    }
}

impl Measured for CallRecord {
    fn bytes(&self) -> usize {
        let response = self
            .outcome
            .as_ref()
            .and_then(|outcome| outcome.line.as_ref())
            .map_or(0, String::len);
        self.request.len()
            + response
            + self.method.as_ref().map_or(0, String::len)
            + self.closed.as_ref().map_or(0, String::len)
            + limits::ENTRY_OVERHEAD
    }
}

/// The version of SOLAR's recording format that `/export` writes, `docs/RECORDING.md` in
/// SOLAR's repository.
pub const RECORDING_FORMAT: &str = "1.0.0";

/// What a recording holds, and what it left out.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Recorded {
    /// The calls written.
    pub calls: usize,
    /// The calls of earlier connections, each of which was a session of its own.
    pub earlier: usize,
    /// The calls whose response was too long for ZENITH to keep, left out whole.
    pub too_long: usize,
    /// The calls the History had already dropped, of any connection.
    pub dropped: u64,
}

/// The History tab: the calls, and which one is selected.
#[derive(Debug, Clone)]
pub struct History {
    /// The calls.
    pub calls: Ring<CallRecord>,
    /// The selected call, or `None` to follow the newest.
    pub selected: Option<u64>,
}

impl Default for History {
    fn default() -> Self {
        Self::new(limits::HISTORY_ENTRIES, limits::HISTORY_BYTES)
    }
}

impl History {
    /// An empty History with these limits.
    #[must_use]
    pub fn new(entries: usize, bytes: usize) -> Self {
        Self {
            calls: Ring::new(entries, bytes),
            selected: None,
        }
    }

    /// The call that is selected: the chosen one, or the newest.
    #[must_use]
    pub fn current(&self) -> Option<u64> {
        self.selected
            .filter(|number| self.calls.get(*number).is_some())
            .or_else(|| self.calls.last().map(|(number, _)| number))
    }

    /// Writes the calls of one connection in SOLAR's recording format, version
    /// [`RECORDING_FORMAT`], as `docs/DESIGN.md` section 12 says: a header that names
    /// ZENITH as the writer, then every line that crossed, in the order it crossed, exactly
    /// as it crossed. A request that was never answered is written without an answer,
    /// which is what happened; a call whose answer was too long to keep is left out whole,
    /// because its answer cannot be written as it crossed.
    ///
    /// # Errors
    ///
    /// What writing returns.
    pub fn record(
        &self,
        out: &mut impl Write,
        connection: u64,
        now: SystemTime,
    ) -> io::Result<Recorded> {
        let mut recorded = Recorded {
            dropped: self.calls.dropped(),
            ..Recorded::default()
        };
        // When each line crossed, requests before answers at the same instant, and the
        // line itself.
        let mut lines: Vec<(Instant, bool, SystemTime, &str, &str)> = Vec::new();
        for (_, record) in self.calls.iter() {
            if record.connection != connection {
                recorded.earlier += 1;
                continue;
            }
            let outcome = record.outcome.as_ref();
            if outcome.is_some_and(|outcome| outcome.line.is_none()) {
                recorded.too_long += 1;
                continue;
            }
            lines.push((record.sent, false, record.sent_wall, "in", &record.request));
            if let Some(outcome) = outcome
                && let Some(line) = &outcome.line
            {
                lines.push((outcome.received, true, outcome.received_wall, "out", line));
            }
            recorded.calls += 1;
        }
        lines.sort_by_key(|(when, answer, ..)| (*when, *answer));
        let started = lines.first().map_or(now, |(_, _, wall, _, _)| *wall);
        writeln!(
            out,
            "{}",
            json!({
                "solar_recording": RECORDING_FORMAT,
                "at": Utc::of(started).rfc3339(),
                "solar_version": format!("ZENITH {}", crate::VERSION),
                "protocol": PROTOCOL,
            })
        )?;
        for (_, _, wall, direction, line) in lines {
            writeln!(
                out,
                "{}",
                json!({"at": Utc::of(wall).rfc3339(), "direction": direction, "line": line})
            )?;
        }
        Ok(recorded)
    }

    /// Writes the `dropped` line and one `call` line per call, which a report shares.
    ///
    /// # Errors
    ///
    /// What writing returns.
    pub fn write_calls(&self, out: &mut impl Write) -> io::Result<usize> {
        writeln!(
            out,
            "{}",
            json!({
                "kind": "dropped",
                "calls": self.calls.dropped(),
                "bytes": self.calls.dropped_bytes(),
            })
        )?;
        let mut written = 0;
        for (_, record) in self.calls.iter() {
            writeln!(out, "{}", call_line(record))?;
            written += 1;
        }
        Ok(written)
    }
}

/// A line as JSON when it is JSON, and as a string when it is not, so that the export of
/// a broken line keeps the line exactly.
fn as_json(line: &str) -> Value {
    serde_json::from_str(line).unwrap_or_else(|_| Value::String(line.to_owned()))
}

fn call_line(record: &CallRecord) -> Value {
    let outcome = record.outcome.as_ref();
    json!({
        "kind": "call",
        "connection": record.connection,
        "id": record.call,
        "method": record.method,
        "origin": record.origin.word(),
        "sent_at": Utc::of(record.sent_wall).rfc3339(),
        "received_at": outcome.map(|outcome| Utc::of(outcome.received_wall).rfc3339()),
        "round_trip_us": record.round_trip().map(|trip| u64::try_from(trip.as_micros()).unwrap_or(u64::MAX)),
        "request": as_json(&record.request),
        "response": outcome.and_then(|outcome| outcome.line.as_deref()).map(as_json),
        "closed": record.closed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn record(call: u64, response: Option<&str>) -> CallRecord {
        let sent = Instant::now();
        CallRecord {
            connection: 1,
            call,
            method: Some("solar.ping".to_owned()),
            origin: Origin::Command,
            request: format!(
                r#"{{"jsonrpc":"2.0","id":{call},"method":"solar.ping","params":{{}}}}"#
            ),
            sent,
            sent_wall: SystemTime::UNIX_EPOCH,
            outcome: response.map(|line| Outcome {
                line: Some(line.to_owned()),
                received: sent + Duration::from_micros(520),
                received_wall: SystemTime::UNIX_EPOCH + Duration::from_micros(520),
                summary: Summary::Ok,
                duration_us: Some(170),
                violations: 0,
                by_order: false,
            }),
            closed: None,
        }
    }

    fn lines_of(out: Vec<u8>) -> Vec<Value> {
        String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn the_calls_of_a_report_are_what_was_dropped_and_one_line_per_call() {
        let mut history = History::new(2, usize::MAX);
        history
            .calls
            .push(record(1, Some(r#"{"jsonrpc":"2.0","id":1}"#)));
        history.calls.push(record(2, Some("not json")));
        history.calls.push(record(3, None));
        let mut out = Vec::new();
        let written = history.write_calls(&mut out).unwrap();
        assert_eq!(written, 2);
        let lines = lines_of(out);
        assert_eq!(
            lines[0],
            json!({"kind": "dropped", "calls": 1, "bytes": record(1, Some(r#"{"jsonrpc":"2.0","id":1}"#)).bytes()})
        );
        assert_eq!(lines[1]["response"], json!("not json"));
        assert_eq!(lines[1]["round_trip_us"], json!(520));
        assert_eq!(lines[2]["response"], Value::Null);
        assert_eq!(lines[2]["request"]["id"], json!(3));
    }

    /// The timestamp of SOLAR's recording format: RFC 3339, UTC, microseconds, `Z`.
    fn is_a_recording_time(value: &Value) -> bool {
        let text = value.as_str().unwrap_or_default();
        let digits = |range: std::ops::Range<usize>| {
            text.get(range)
                .is_some_and(|part| part.chars().all(|c| c.is_ascii_digit()))
        };
        text.len() == 27
            && digits(0..4)
            && digits(5..7)
            && digits(8..10)
            && digits(11..13)
            && digits(14..16)
            && digits(17..19)
            && digits(20..26)
            && text.get(19..20) == Some(".")
            && text.ends_with('Z')
    }

    #[test]
    fn a_recording_is_a_header_then_every_line_in_the_order_it_crossed() {
        let mut history = History::default();
        let start = Instant::now();
        // Two calls in flight at once, the second answered first, and one never answered.
        let mut first = record(1, Some(r#"{"jsonrpc":"2.0","id":1,"result":{}}"#));
        let mut second = record(2, Some(r#"{"jsonrpc":"2.0","id":2,"result":{}}"#));
        let mut third = record(3, None);
        first.sent = start;
        second.sent = start + Duration::from_micros(10);
        third.sent = start + Duration::from_micros(40);
        first.sent_wall = SystemTime::UNIX_EPOCH + Duration::from_secs(1_790_000_000);
        second.sent_wall = first.sent_wall + Duration::from_micros(10);
        third.sent_wall = first.sent_wall + Duration::from_micros(40);
        if let Some(outcome) = first.outcome.as_mut() {
            outcome.received = start + Duration::from_micros(30);
            outcome.received_wall = first.sent_wall + Duration::from_micros(30);
        }
        if let Some(outcome) = second.outcome.as_mut() {
            outcome.received = start + Duration::from_micros(20);
            outcome.received_wall = first.sent_wall + Duration::from_micros(20);
        }
        history.calls.push(first);
        history.calls.push(second);
        history.calls.push(third);
        let mut out = Vec::new();
        let recorded = history.record(&mut out, 1, SystemTime::UNIX_EPOCH).unwrap();
        assert_eq!(
            recorded,
            Recorded {
                calls: 3,
                ..Recorded::default()
            }
        );
        let lines = lines_of(out);
        assert_eq!(
            lines[0],
            json!({
                "solar_recording": "1.0.0",
                "at": "2026-09-21T14:13:20.000000Z",
                "solar_version": format!("ZENITH {}", crate::VERSION),
                "protocol": "solar/1",
            })
        );
        let order: Vec<(String, u64)> = lines[1..]
            .iter()
            .map(|line| {
                let crossed: Value = serde_json::from_str(line["line"].as_str().unwrap()).unwrap();
                (
                    line["direction"].as_str().unwrap().to_owned(),
                    crossed["id"].as_u64().unwrap(),
                )
            })
            .collect();
        let expected = [("in", 1), ("in", 2), ("out", 2), ("out", 1), ("in", 3)];
        assert_eq!(
            order,
            expected
                .iter()
                .map(|(direction, id)| ((*direction).to_owned(), *id))
                .collect::<Vec<_>>()
        );
        for line in &lines[1..] {
            let members: Vec<&str> = line
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect();
            assert_eq!(members, vec!["at", "direction", "line"]);
            assert!(is_a_recording_time(&line["at"]), "{line}");
        }
        assert!(is_a_recording_time(&lines[0]["at"]));
        assert_eq!(lines[4]["at"], json!("2026-09-21T14:13:20.000030Z"));
    }

    #[test]
    fn a_line_is_recorded_exactly_as_it_crossed_even_when_it_is_not_json() {
        let mut history = History::default();
        let mut broken = record(1, Some("not json at all"));
        broken.request = "{\"jsonrpc\": \"2.0\",   \"id\":1, \"method\":\"x\"".to_owned();
        let request = broken.request.clone();
        history.calls.push(broken);
        let mut out = Vec::new();
        history.record(&mut out, 1, SystemTime::UNIX_EPOCH).unwrap();
        let lines = lines_of(out);
        assert_eq!(lines[1]["line"], json!(request));
        assert_eq!(lines[2]["line"], json!("not json at all"));
    }

    #[test]
    fn a_recording_leaves_out_other_connections_and_answers_too_long_to_keep() {
        let mut history = History::new(3, usize::MAX);
        history.calls.push(record(1, Some("{}")));
        let mut earlier = record(1, Some("{}"));
        earlier.connection = 1;
        history.calls.push(earlier);
        let mut long = record(1, Some("{}"));
        long.connection = 2;
        if let Some(outcome) = long.outcome.as_mut() {
            outcome.line = None;
            outcome.summary = Summary::TooLong { bytes: 1 << 25 };
        }
        history.calls.push(long);
        let mut kept = record(2, Some("{}"));
        kept.connection = 2;
        history.calls.push(kept);
        let mut out = Vec::new();
        let recorded = history
            .record(&mut out, 2, SystemTime::UNIX_EPOCH + Duration::from_secs(9))
            .unwrap();
        assert_eq!(
            recorded,
            Recorded {
                calls: 1,
                earlier: 1,
                too_long: 1,
                dropped: 1,
            }
        );
        let lines = lines_of(out);
        assert_eq!(lines.len(), 3);
        // A recording with nothing in it starts when it was written.
        let mut out = Vec::new();
        let recorded = History::default()
            .record(&mut out, 1, SystemTime::UNIX_EPOCH + Duration::from_secs(9))
            .unwrap();
        assert_eq!(recorded, Recorded::default());
        let lines = lines_of(out);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["at"], json!("1970-01-01T00:00:09.000000Z"));
    }

    #[test]
    fn the_current_call_follows_the_newest_until_one_is_chosen() {
        let mut history = History::default();
        history.calls.push(record(1, None));
        let second = history.calls.push(record(2, None));
        assert_eq!(history.current(), Some(second));
        history.selected = Some(0);
        assert_eq!(history.current(), Some(0));
    }
}
