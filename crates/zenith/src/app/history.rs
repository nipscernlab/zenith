//! The History: every request and response, with its timing, in a ring buffer.

use std::io::{self, Write};
use std::time::{Instant, SystemTime};

use serde_json::{Value, json};
use zenith_client::envelope::{Body, Message};

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

    /// Writes the History as NDJSON, one call a line, as `docs/DESIGN.md` section 12
    /// specifies, straight to `out` as it walks the buffer. Returns how many calls it
    /// wrote.
    ///
    /// # Errors
    ///
    /// What writing returns.
    pub fn export(&self, out: &mut impl Write, now: SystemTime) -> io::Result<usize> {
        writeln!(
            out,
            "{}",
            json!({
                "kind": "header",
                "format": "zenith-history",
                "format_version": "1.0.0",
                "zenith_version": crate::VERSION,
                "written_at": Utc::of(now).rfc3339(),
            })
        )?;
        self.write_calls(out)
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

    #[test]
    fn the_export_is_a_header_what_was_dropped_and_one_line_per_call() {
        let mut history = History::new(2, usize::MAX);
        history
            .calls
            .push(record(1, Some(r#"{"jsonrpc":"2.0","id":1}"#)));
        history.calls.push(record(2, Some("not json")));
        history.calls.push(record(3, None));
        let mut out = Vec::new();
        let written = history.export(&mut out, SystemTime::UNIX_EPOCH).unwrap();
        assert_eq!(written, 2);
        let lines: Vec<Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(lines[0]["format"], json!("zenith-history"));
        assert_eq!(
            lines[1],
            json!({"kind": "dropped", "calls": 1, "bytes": record(1, Some(r#"{"jsonrpc":"2.0","id":1}"#)).bytes()})
        );
        assert_eq!(lines[2]["response"], json!("not json"));
        assert_eq!(lines[2]["round_trip_us"], json!(520));
        assert_eq!(lines[3]["response"], Value::Null);
        assert_eq!(lines[3]["request"]["id"], json!(3));
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
