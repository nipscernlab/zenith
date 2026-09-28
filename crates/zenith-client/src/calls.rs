//! The calls that are waiting for SOLAR, and which one each response answers.
//!
//! Responses are matched by `id`, never by order: once SOLAR answers `solar.cancel` ahead
//! of the call it cancels, order is no longer a promise, section 9.1 of SOLAR's contract.
//! ZENITH numbers its requests from one in each connection and never reuses an id while
//! its call is waiting, which section 9.5 asks of a caller that cancels.

use std::collections::VecDeque;
use std::time::Instant;

use serde_json::Value;

use crate::envelope::{Envelope, Id, Message, request_line};

/// What a response to a waiting call looks like.
#[derive(Debug, Clone, PartialEq)]
pub enum Expected {
    /// A single response with this id, which is every call ZENITH builds itself.
    Id(Id),
    /// A single response with `id: null`, for a line sent by hand that has no usable id.
    Null,
    /// An array of responses, for a batch sent by hand; or, when SOLAR refuses the batch
    /// as a whole, a single response with `id: null`.
    Batch,
}

/// A call that has been sent and not answered.
#[derive(Debug, Clone, PartialEq)]
pub struct Waiting {
    /// ZENITH's number for the call, which is also its id when ZENITH built it.
    pub call: u64,
    /// The method, when the line names one.
    pub method: Option<String>,
    /// What its answer will look like.
    pub expected: Expected,
    /// When it was queued to be written.
    pub sent: Instant,
}

/// Which call a line from SOLAR answers.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// The call it answers.
    Call {
        /// The call.
        waiting: Waiting,
        /// Whether it was matched by order rather than by id, which happens only to a
        /// response with `id: null` when no line sent by hand expected one.
        by_order: bool,
    },
    /// No waiting call expects it.
    Unexpected,
}

/// Why a call was not sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// As many calls as ZENITH allows are already waiting.
    Full {
        /// That many.
        limit: usize,
    },
    /// A line sent by hand has an id that a waiting call already has.
    IdInUse {
        /// The id.
        id: String,
    },
    /// The request line is longer than SOLAR reads, as its manifest declares.
    TooLong {
        /// The length of the line, in bytes, without its newline.
        bytes: usize,
        /// The most SOLAR reads.
        limit: usize,
    },
}

impl std::fmt::Display for Refused {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full { limit } => write!(
                formatter,
                "{limit} calls are already waiting for SOLAR, which is as many as ZENITH keeps."
            ),
            Self::IdInUse { id } => write!(
                formatter,
                "A call with the id {id} is already waiting, and neither SOLAR nor ZENITH \
                 could tell the two answers apart."
            ),
            Self::TooLong { bytes, limit } => write!(
                formatter,
                "The request is {bytes} bytes, and SOLAR's manifest says it reads lines of at \
                 most {limit}, so nothing was sent."
            ),
        }
    }
}

impl std::error::Error for Refused {}

/// The waiting calls of one connection.
#[derive(Debug)]
pub struct Tracker {
    next: u64,
    waiting: VecDeque<Waiting>,
    limit: usize,
    longest: Option<usize>,
}

impl Tracker {
    /// No calls, and at most `limit` waiting at once.
    #[must_use]
    pub fn new(limit: usize) -> Self {
        Self {
            next: 1,
            waiting: VecDeque::new(),
            limit,
            longest: None,
        }
    }

    /// Refuses, from now on, a call ZENITH builds whose line is longer than `longest`
    /// bytes, or refuses none with `None`. A line sent by hand is never refused for its
    /// length: SOLAR's own answer to it is the point of sending it.
    pub fn refuse_lines_longer_than(&mut self, longest: Option<usize>) {
        self.longest = longest;
    }

    /// Numbers a call ZENITH builds, records it as waiting, and returns its number and
    /// the line to write.
    ///
    /// # Errors
    ///
    /// [`Refused::Full`] when the limit of waiting calls is reached, and
    /// [`Refused::TooLong`] when the line is longer than the one set with
    /// [`Tracker::refuse_lines_longer_than`]. A refused call takes no number.
    pub fn call(
        &mut self,
        method: &str,
        params: &Value,
        sent: Instant,
    ) -> Result<(u64, String), Refused> {
        self.check_room()?;
        let next = self.next;
        let id = self.next_free_number();
        let line = request_line(id, method, params);
        if let Some(limit) = self.longest
            && line.len() > limit
        {
            self.next = next;
            return Err(Refused::TooLong {
                bytes: line.len(),
                limit,
            });
        }
        self.waiting.push_back(Waiting {
            call: id,
            method: Some(method.to_owned()),
            expected: Expected::Id(Id::Number(id.into())),
            sent,
        });
        Ok((id, line))
    }

    /// Records a line sent by hand, exactly as typed, and returns its number.
    ///
    /// # Errors
    ///
    /// [`Refused::Full`] when the limit is reached, and [`Refused::IdInUse`] when the
    /// line's id is already waiting.
    pub fn raw(&mut self, line: &str, sent: Instant) -> Result<u64, Refused> {
        self.check_room()?;
        let (expected, method) = match serde_json::from_str::<Value>(line) {
            Ok(Value::Array(_)) => (Expected::Batch, None),
            Ok(Value::Object(request)) => {
                let method = request
                    .get("method")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let expected = match request.get("id") {
                    Some(Value::Number(number)) => Expected::Id(Id::Number(number.clone())),
                    Some(Value::String(text)) => Expected::Id(Id::String(text.clone())),
                    _ => Expected::Null,
                };
                (expected, method)
            }
            _ => (Expected::Null, None),
        };
        if let Expected::Id(id) = &expected
            && self
                .waiting
                .iter()
                .any(|waiting| waiting.expected == Expected::Id(id.clone()))
        {
            return Err(Refused::IdInUse {
                id: id.to_value().to_string(),
            });
        }
        let call = self.next_free_number();
        self.waiting.push_back(Waiting {
            call,
            method,
            expected,
            sent,
        });
        Ok(call)
    }

    /// Finds the call a line answers, and stops waiting for it.
    pub fn answer(&mut self, message: &Message) -> Answer {
        let position = match message {
            Message::Batch(_) => self
                .waiting
                .iter()
                .position(|waiting| waiting.expected == Expected::Batch),
            Message::Single(Envelope { id, .. }) if !id.is_null() => {
                self.waiting
                    .iter()
                    .position(|waiting| match &waiting.expected {
                        Expected::Id(expected) => same_id(expected, id),
                        Expected::Null | Expected::Batch => false,
                    })
            }
            Message::Single(_) => {
                let by_hand = self.waiting.iter().position(|waiting| {
                    matches!(waiting.expected, Expected::Null | Expected::Batch)
                });
                if let Some(position) = by_hand {
                    return self.take(position, false);
                }
                return if self.waiting.is_empty() {
                    Answer::Unexpected
                } else {
                    self.take(0, true)
                };
            }
            Message::NotJson { .. } => None,
        };
        match position {
            Some(position) => self.take(position, false),
            None => Answer::Unexpected,
        }
    }

    /// The calls still waiting, oldest first.
    #[must_use]
    pub fn waiting(&self) -> impl ExactSizeIterator<Item = &Waiting> {
        self.waiting.iter()
    }

    /// The call with this number, if it is waiting.
    #[must_use]
    pub fn find(&self, call: u64) -> Option<&Waiting> {
        self.waiting.iter().find(|waiting| waiting.call == call)
    }

    /// Stops waiting for everything, because the connection ended, and returns what was
    /// waiting.
    pub fn close_all(&mut self) -> Vec<Waiting> {
        self.waiting.drain(..).collect()
    }

    fn take(&mut self, position: usize, by_order: bool) -> Answer {
        match self.waiting.remove(position) {
            Some(waiting) => Answer::Call { waiting, by_order },
            None => Answer::Unexpected,
        }
    }

    fn check_room(&self) -> Result<(), Refused> {
        if self.waiting.len() >= self.limit {
            return Err(Refused::Full { limit: self.limit });
        }
        Ok(())
    }

    /// The next number that no waiting call uses as its id, so that a line sent by hand
    /// with a large id is never answered as one of ZENITH's.
    fn next_free_number(&mut self) -> u64 {
        loop {
            let candidate = self.next;
            self.next += 1;
            let taken = self
                .waiting
                .iter()
                .any(|waiting| matches!(&waiting.expected, Expected::Id(id) if id.is(candidate)));
            if !taken {
                return candidate;
            }
        }
    }
}

/// Ids compared as JSON compares them: `1` and `1.0` are not the same id, because the
/// contract echoes an id unchanged and a client matches what it sent.
fn same_id(expected: &Id, received: &Id) -> bool {
    match (expected, received) {
        (Id::Number(a), Id::Number(b)) => a == b,
        (Id::String(a), Id::String(b)) => a == b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::read;
    use serde_json::json;

    fn response(id: &Value) -> Message {
        read(&json!({"jsonrpc": "2.0", "id": id, "result": {"data": {}, "meta": {}, "warnings": []}}).to_string())
    }

    #[test]
    fn calls_are_numbered_from_one_and_answered_by_id_in_any_order() {
        let mut tracker = Tracker::new(8);
        let now = Instant::now();
        let (first, line) = tracker.call("solar.ping", &json!({}), now).unwrap();
        let (second, _) = tracker.call("solar.version", &json!({}), now).unwrap();
        assert_eq!((first, second), (1, 2));
        assert!(line.contains(r#""id":1"#));
        let Answer::Call { waiting, by_order } = tracker.answer(&response(&json!(2))) else {
            panic!("the second call was not found");
        };
        assert_eq!((waiting.call, by_order), (2, false));
        assert!(matches!(
            tracker.answer(&response(&json!(1))),
            Answer::Call { .. }
        ));
        assert_eq!(tracker.waiting().len(), 0);
    }

    #[test]
    fn a_response_nobody_waits_for_is_unexpected() {
        let mut tracker = Tracker::new(8);
        assert_eq!(tracker.answer(&response(&json!(7))), Answer::Unexpected);
        assert_eq!(tracker.answer(&response(&Value::Null)), Answer::Unexpected);
        assert_eq!(tracker.answer(&read("not json")), Answer::Unexpected);
    }

    #[test]
    fn a_null_id_answers_a_broken_line_sent_by_hand_before_anything_else() {
        let mut tracker = Tracker::new(8);
        let now = Instant::now();
        tracker.call("solar.ping", &json!({}), now).unwrap();
        let broken = tracker.raw("{not json", now).unwrap();
        let Answer::Call { waiting, by_order } = tracker.answer(&response(&Value::Null)) else {
            panic!("nothing answered");
        };
        assert_eq!((waiting.call, by_order), (broken, false));
    }

    #[test]
    fn a_null_id_with_nothing_sent_by_hand_answers_the_oldest_call_by_order() {
        let mut tracker = Tracker::new(8);
        tracker
            .call("solar.ping", &json!({}), Instant::now())
            .unwrap();
        let Answer::Call { by_order, .. } = tracker.answer(&response(&Value::Null)) else {
            panic!("nothing answered");
        };
        assert!(by_order);
    }

    #[test]
    fn a_batch_sent_by_hand_is_answered_by_an_array() {
        let mut tracker = Tracker::new(8);
        let batch = tracker
            .raw(
                r#"[{"jsonrpc":"2.0","id":"a","method":"solar.ping"}]"#,
                Instant::now(),
            )
            .unwrap();
        let array = read(&format!(
            "[{}]",
            json!({"jsonrpc": "2.0", "id": "a", "result": {"data": {}, "meta": {}, "warnings": []}})
        ));
        let Answer::Call { waiting, .. } = tracker.answer(&array) else {
            panic!("the batch was not answered");
        };
        assert_eq!(waiting.call, batch);
    }

    #[test]
    fn a_line_sent_by_hand_with_an_id_that_is_waiting_is_refused() {
        let mut tracker = Tracker::new(8);
        tracker
            .call("solar.ping", &json!({}), Instant::now())
            .unwrap();
        let refused = tracker
            .raw(
                r#"{"jsonrpc":"2.0","id":1,"method":"solar.ping"}"#,
                Instant::now(),
            )
            .unwrap_err();
        assert_eq!(refused, Refused::IdInUse { id: "1".to_owned() });
    }

    #[test]
    fn zenith_never_gives_its_own_call_an_id_a_line_sent_by_hand_is_waiting_on() {
        let mut tracker = Tracker::new(8);
        tracker
            .raw(
                r#"{"jsonrpc":"2.0","id":1,"method":"solar.ping"}"#,
                Instant::now(),
            )
            .unwrap();
        let (id, _) = tracker
            .call("solar.ping", &json!({}), Instant::now())
            .unwrap();
        assert_ne!(id, 1);
    }

    #[test]
    fn a_call_longer_than_solar_reads_is_refused_and_takes_no_number() {
        let mut tracker = Tracker::new(8);
        let now = Instant::now();
        let (first, line) = tracker.call("solar.ping", &json!({}), now).unwrap();
        let limit = line.len() + 16;
        tracker.refuse_lines_longer_than(Some(limit));
        let refused = tracker
            .call(
                "solar.ping",
                &json!({"message": "far too long, surely"}),
                now,
            )
            .unwrap_err();
        let Refused::TooLong { bytes, limit: said } = refused.clone() else {
            panic!("{refused:?}");
        };
        assert_eq!(said, limit);
        assert!(bytes > limit);
        assert!(refused.to_string().contains(&format!("at most {limit}")));
        assert_eq!(tracker.waiting().len(), 1);
        // A line exactly as long as the limit is sent, and it takes the next number.
        let (second, exact) = tracker
            .call("solar.ping", &json!({"message": "abcd"}), now)
            .unwrap();
        assert_eq!(exact.len(), limit);
        assert_eq!(second, first + 1);
        // A line sent by hand is never refused for its length.
        let long = format!(
            r#"{{"jsonrpc":"2.0","id":"h","method":"{}"}}"#,
            "x".repeat(64)
        );
        assert!(tracker.raw(&long, now).is_ok());
        tracker.refuse_lines_longer_than(None);
        assert!(
            tracker
                .call(
                    "solar.ping",
                    &json!({"message": "far too long, surely"}),
                    now
                )
                .is_ok()
        );
    }

    #[test]
    fn the_limit_of_waiting_calls_is_kept() {
        let mut tracker = Tracker::new(2);
        let now = Instant::now();
        tracker.call("a.b", &json!({}), now).unwrap();
        tracker.call("a.b", &json!({}), now).unwrap();
        assert_eq!(
            tracker.call("a.b", &json!({}), now).unwrap_err(),
            Refused::Full { limit: 2 }
        );
        assert_eq!(tracker.close_all().len(), 2);
        assert!(tracker.call("a.b", &json!({}), now).is_ok());
    }

    #[test]
    fn a_line_sent_by_hand_with_a_string_id_is_answered_by_that_id_and_no_other() {
        let mut tracker = Tracker::new(8);
        let now = Instant::now();
        let (ping, _) = tracker.call("solar.ping", &json!({}), now).unwrap();
        let by_hand = tracker
            .raw(
                r#"{"jsonrpc":"2.0","id":"by hand","method":"solar.version"}"#,
                now,
            )
            .unwrap();
        assert_eq!(
            tracker
                .find(by_hand)
                .and_then(|waiting| waiting.method.as_deref()),
            Some("solar.version")
        );
        assert_eq!(tracker.find(ping).map(|waiting| waiting.call), Some(ping));
        assert!(tracker.find(by_hand + ping + 10).is_none());
        // An id of other text is not this one.
        assert!(matches!(
            tracker.answer(&response(&json!("by han"))),
            Answer::Unexpected
        ));
        // The answer to the string id is the line's, though the ping was sent first.
        match tracker.answer(&response(&json!("by hand"))) {
            Answer::Call { waiting, by_order } => {
                assert_eq!(waiting.call, by_hand);
                assert!(!by_order);
            }
            other @ Answer::Unexpected => panic!("{other:?}"),
        }
        assert!(tracker.find(by_hand).is_none());
    }

    #[test]
    fn a_refusal_says_why_in_a_sentence() {
        assert_eq!(
            Refused::Full { limit: 256 }.to_string(),
            "256 calls are already waiting for SOLAR, which is as many as ZENITH keeps."
        );
        assert!(
            Refused::IdInUse {
                id: "\"a\"".to_owned()
            }
            .to_string()
            .starts_with("A call with the id \"a\" is already waiting")
        );
    }
}
