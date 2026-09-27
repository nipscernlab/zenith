//! Responses as they come back from SOLAR, read and checked against the contract.
//!
//! ZENITH is how SOLAR is tested by hand, so a response that breaks the contract must not
//! look like one that keeps it. Every line SOLAR writes is read here into its parts, and
//! everything about its shape that sections 5, 6 and 7 of the contract require is checked
//! on the way. What does not hold becomes a [`Violation`] that the screen shows beside the
//! response, naming the section.
//!
//! The checks are of what the contract states for `solar/1`. A status the contract does
//! not list is not a violation, because a later SOLAR may add one without changing the
//! protocol, and SOLAR's draft of its third stage does exactly that with `CANCELLED`.

use serde_json::{Map, Number, Value};

/// The `id` of a message.
#[derive(Debug, Clone, PartialEq)]
pub enum Id {
    /// A number.
    Number(Number),
    /// A string.
    String(String),
    /// `null`.
    Null,
    /// No `id` member at all.
    Absent,
}

impl Id {
    fn of(value: Option<&Value>) -> Self {
        match value {
            None => Self::Absent,
            Some(Value::Null) => Self::Null,
            Some(Value::Number(number)) => Self::Number(number.clone()),
            Some(Value::String(text)) => Self::String(text.clone()),
            Some(other) => Self::String(other.to_string()),
        }
    }

    /// The id as a JSON value, as it would be written.
    #[must_use]
    pub fn to_value(&self) -> Value {
        match self {
            Self::Number(number) => Value::Number(number.clone()),
            Self::String(text) => Value::String(text.clone()),
            Self::Null | Self::Absent => Value::Null,
        }
    }

    /// Whether this is the integer `id`, which is how ZENITH numbers its own requests.
    #[must_use]
    pub fn is(&self, id: u64) -> bool {
        matches!(self, Self::Number(number) if number.as_u64() == Some(id))
    }

    /// Whether this is `null` or absent.
    #[must_use]
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null | Self::Absent)
    }
}

/// One line SOLAR wrote, read.
#[derive(Debug, Clone)]
#[allow(
    clippy::large_enum_variant,
    reason = "a message lives while one line is handled, one at a time, so its size is               never multiplied"
)]
pub enum Message {
    /// A single response.
    Single(Envelope),
    /// A batch of responses, section 3.2 of SOLAR's draft contract.
    Batch(Vec<Envelope>),
    /// A line that is not JSON at all.
    NotJson {
        /// What the JSON parser said.
        error: String,
    },
}

/// One response, read into its parts.
#[derive(Debug, Clone)]
pub struct Envelope {
    /// The `id`.
    pub id: Id,
    /// What the response says.
    pub body: Body,
    /// `meta`, from `result` or from `error.data`.
    pub meta: Option<Meta>,
    /// What about its shape breaks the contract.
    pub violations: Vec<Violation>,
}

/// The two kinds of response, and the third that is neither.
#[derive(Debug, Clone)]
pub enum Body {
    /// `result`.
    Success {
        /// `result.data`.
        data: Value,
        /// `result.warnings`.
        warnings: Vec<Warning>,
    },
    /// `error`.
    Failure(Failure),
    /// Neither `result` nor `error`, or both.
    Unrecognised,
}

/// An error response.
#[derive(Debug, Clone, Default)]
pub struct Failure {
    /// `error.code`.
    pub code: Option<i64>,
    /// `error.message`.
    pub message: Option<String>,
    /// `error.data.status`.
    pub status: Option<String>,
    /// `error.data.reason`.
    pub reason: Option<String>,
    /// `error.data.details`.
    pub details: Vec<Detail>,
}

/// One entry of `error.data.details`, section 6 of the contract.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Detail {
    /// Where the problem is.
    pub field: Option<String>,
    /// What that place should have held.
    pub expected: Option<String>,
    /// What arrived there.
    pub received: Value,
    /// What to do about it.
    pub hint: Option<String>,
    /// A link into SOLAR's error catalogue.
    pub docs: Option<String>,
}

/// One entry of `result.warnings`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Warning {
    /// `code`.
    pub code: Option<String>,
    /// `message`.
    pub message: Option<String>,
}

/// The execution metadata of section 7 of the contract.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Meta {
    /// `request_id`.
    pub request_id: Value,
    /// `method`.
    pub method: Option<String>,
    /// `api_version`.
    pub api_version: Option<String>,
    /// `solar_version`.
    pub solar_version: Option<String>,
    /// `protocol`.
    pub protocol: Option<String>,
    /// `started_at`.
    pub started_at: Option<String>,
    /// `duration_us`, SOLAR's own time for the call.
    pub duration_us: Option<u64>,
    /// `os`.
    pub os: Option<String>,
    /// `arch`.
    pub arch: Option<String>,
}

/// Something about a response that the contract does not allow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// What is wrong, as a sentence.
    pub what: String,
    /// The section of SOLAR's contract that requires otherwise.
    pub section: &'static str,
}

/// The canonical statuses of `solar/1` and their JSON-RPC codes, section 6.1.
pub const STATUS_CODES: [(&str, i64); 11] = [
    ("INVALID_ARGUMENT", -32602),
    ("NOT_FOUND", -32601),
    ("ALREADY_EXISTS", -32001),
    ("FAILED_PRECONDITION", -32002),
    ("PERMISSION_DENIED", -32003),
    ("RESOURCE_EXHAUSTED", -32004),
    ("DEADLINE_EXCEEDED", -32005),
    ("UNAVAILABLE", -32006),
    ("UNIMPLEMENTED", -32007),
    ("INTERNAL", -32603),
    ("UNKNOWN", -32099),
];

const META_MEMBERS: [&str; 9] = [
    "request_id",
    "method",
    "api_version",
    "solar_version",
    "protocol",
    "started_at",
    "duration_us",
    "os",
    "arch",
];

const DETAIL_MEMBERS: [&str; 5] = ["field", "expected", "received", "hint", "docs"];

/// Reads one line SOLAR wrote.
#[must_use]
pub fn read(line: &str) -> Message {
    match serde_json::from_str::<Value>(line) {
        Ok(Value::Array(items)) => Message::Batch(items.iter().map(Envelope::of).collect()),
        Ok(value) => Message::Single(Envelope::of(&value)),
        Err(error) => Message::NotJson {
            error: error.to_string(),
        },
    }
}

struct Checker {
    violations: Vec<Violation>,
}

impl Checker {
    fn fail(&mut self, section: &'static str, what: impl Into<String>) {
        self.violations.push(Violation {
            what: what.into(),
            section,
        });
    }
}

impl Envelope {
    /// Reads one response and checks its shape.
    #[must_use]
    pub fn of(value: &Value) -> Self {
        let mut check = Checker {
            violations: Vec::new(),
        };
        let Value::Object(root) = value else {
            check.fail("5", "The response is not a JSON object.");
            return Self {
                id: Id::Absent,
                body: Body::Unrecognised,
                meta: None,
                violations: check.violations,
            };
        };
        let id = Id::of(root.get("id"));
        if root.get("jsonrpc") != Some(&Value::String("2.0".to_owned())) {
            check.fail("3", "The response has no jsonrpc member equal to \"2.0\".");
        }
        if id == Id::Absent {
            check.fail("3", "The response has no id member.");
        }
        let (body, meta) = match (root.get("result"), root.get("error")) {
            (Some(result), None) => success(result, &mut check),
            (None, Some(error)) => failure(error, &mut check),
            (Some(_), Some(_)) => {
                check.fail("5", "The response has both result and error.");
                (Body::Unrecognised, None)
            }
            (None, None) => {
                check.fail("5", "The response has neither result nor error.");
                (Body::Unrecognised, None)
            }
        };
        if let Some(meta) = &meta
            && meta.request_id != id.to_value()
        {
            check.fail(
                "7",
                format!(
                    "meta.request_id is {}, and the response's id is {}; section 7 says it \
                         is echoed unchanged.",
                    meta.request_id,
                    id.to_value()
                ),
            );
        }
        Self {
            id,
            body,
            meta,
            violations: check.violations,
        }
    }

    /// Whether the call succeeded.
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(self.body, Body::Success { .. })
    }

    /// The status of an error response.
    #[must_use]
    pub fn status(&self) -> Option<&str> {
        match &self.body {
            Body::Failure(failure) => failure.status.as_deref(),
            _ => None,
        }
    }
}

fn success(result: &Value, check: &mut Checker) -> (Body, Option<Meta>) {
    let Value::Object(result) = result else {
        check.fail("5", "result is not an object.");
        return (Body::Unrecognised, None);
    };
    let data = result.get("data").cloned().unwrap_or(Value::Null);
    match result.get("data") {
        Some(Value::Object(_)) => {}
        Some(_) => check.fail("5", "result.data is not an object."),
        None => check.fail("5", "result has no data member."),
    }
    let warnings = match result.get("warnings") {
        Some(Value::Array(entries)) => entries
            .iter()
            .enumerate()
            .map(|(index, entry)| warning(index, entry, check))
            .collect(),
        Some(_) => {
            check.fail("5", "result.warnings is not an array.");
            Vec::new()
        }
        None => {
            check.fail("5", "result has no warnings member; it is never absent.");
            Vec::new()
        }
    };
    let meta = if let Some(value) = result.get("meta") {
        Some(meta(value, "result.meta", check))
    } else {
        check.fail("5", "result has no meta member.");
        None
    };
    let extra: Vec<&str> = result
        .keys()
        .map(String::as_str)
        .filter(|key| !["data", "meta", "warnings"].contains(key))
        .collect();
    if !extra.is_empty() {
        check.fail(
            "5",
            format!(
                "result has {} beyond data, meta and warnings; it has exactly three members.",
                extra.join(", ")
            ),
        );
    }
    (Body::Success { data, warnings }, meta)
}

fn warning(index: usize, entry: &Value, check: &mut Checker) -> Warning {
    let code = entry.get("code").and_then(Value::as_str).map(str::to_owned);
    let message = entry
        .get("message")
        .and_then(Value::as_str)
        .map(str::to_owned);
    if code.is_none() || message.is_none() {
        check.fail(
            "5",
            format!("result.warnings[{index}] is not a code and a message, both strings."),
        );
    }
    Warning { code, message }
}

fn failure(error: &Value, check: &mut Checker) -> (Body, Option<Meta>) {
    let Value::Object(error) = error else {
        check.fail("6", "error is not an object.");
        return (Body::Unrecognised, None);
    };
    let code = error.get("code").and_then(Value::as_i64);
    if code.is_none() {
        check.fail("6", "error.code is not an integer.");
    }
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .map(str::to_owned);
    if message.is_none() {
        check.fail("6", "error.message is not a string.");
    }
    let Some(Value::Object(data)) = error.get("data") else {
        check.fail("6", "error.data is not an object.");
        return (
            Body::Failure(Failure {
                code,
                message,
                ..Failure::default()
            }),
            None,
        );
    };
    let status = data
        .get("status")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let reason = data
        .get("reason")
        .and_then(Value::as_str)
        .map(str::to_owned);
    if status.is_none() {
        check.fail("6", "error.data.status is not a string.");
    }
    if reason.is_none() {
        check.fail("6", "error.data.reason is not a string.");
    }
    check_code(code, status.as_deref(), check);
    let details = if let Some(Value::Array(entries)) = data.get("details") {
        if entries.is_empty() {
            check.fail("6", "error.data.details is empty; it is never empty.");
        }
        entries
            .iter()
            .enumerate()
            .map(|(index, entry)| detail(index, entry, status.as_deref(), check))
            .collect()
    } else {
        check.fail("6", "error.data.details is not an array.");
        Vec::new()
    };
    let meta = if let Some(value) = data.get("meta") {
        Some(meta(value, "error.data.meta", check))
    } else {
        check.fail("6", "error.data has no meta member.");
        None
    };
    let extra = unknown_members(data, &["status", "reason", "details", "meta"]);
    if !extra.is_empty() {
        check.fail(
            "6",
            format!(
                "error.data has {} beyond status, reason, details and meta; it has exactly \
                 four members.",
                extra.join(", ")
            ),
        );
    }
    (
        Body::Failure(Failure {
            code,
            message,
            status,
            reason,
            details,
        }),
        meta,
    )
}

fn check_code(code: Option<i64>, status: Option<&str>, check: &mut Checker) {
    let (Some(code), Some(status)) = (code, status) else {
        return;
    };
    // The two envelope level codes of section 6.1 belong to INVALID_ARGUMENT only.
    if code == -32700 || code == -32600 {
        if status != "INVALID_ARGUMENT" {
            check.fail(
                "6.1",
                format!("error.code {code} is only used with INVALID_ARGUMENT, not {status}."),
            );
        }
        return;
    }
    if let Some((_, expected)) = STATUS_CODES.iter().find(|(name, _)| *name == status)
        && code != *expected
    {
        check.fail(
            "6.1",
            format!("error.code is {code}, and {status} has the code {expected}."),
        );
    }
}

fn detail(index: usize, entry: &Value, status: Option<&str>, check: &mut Checker) -> Detail {
    let Value::Object(map) = entry else {
        check.fail(
            "6",
            format!("error.data.details[{index}] is not an object."),
        );
        return Detail::default();
    };
    let absent: Vec<&str> = DETAIL_MEMBERS
        .iter()
        .copied()
        .filter(|member| !map.contains_key(*member))
        .collect();
    if !absent.is_empty() {
        check.fail(
            "6",
            format!(
                "error.data.details[{index}] has no {}; every member is always present.",
                absent.join(", ")
            ),
        );
    }
    let docs = map.get("docs").and_then(Value::as_str).map(str::to_owned);
    if let (Some(docs), Some(status)) = (&docs, status) {
        let expected = format!("docs/ERRORS.md#{}", status.to_lowercase());
        if docs != &expected {
            check.fail(
                "6",
                format!("error.data.details[{index}].docs is {docs}, and should be {expected}."),
            );
        }
    }
    let text = |key: &str| map.get(key).and_then(Value::as_str).map(str::to_owned);
    Detail {
        field: text("field"),
        expected: text("expected"),
        received: map.get("received").cloned().unwrap_or(Value::Null),
        hint: text("hint"),
        docs,
    }
}

fn meta(value: &Value, place: &str, check: &mut Checker) -> Meta {
    let Value::Object(map) = value else {
        check.fail("7", format!("{place} is not an object."));
        return Meta::default();
    };
    let absent: Vec<&str> = META_MEMBERS
        .iter()
        .copied()
        .filter(|member| !map.contains_key(*member))
        .collect();
    if !absent.is_empty() {
        check.fail("7", format!("{place} has no {}.", absent.join(", ")));
    }
    let duration_us = map.get("duration_us").and_then(Value::as_u64);
    if map.contains_key("duration_us") && duration_us.is_none() {
        check.fail("7", format!("{place}.duration_us is not a whole number."));
    }
    let text = |key: &str| map.get(key).and_then(Value::as_str).map(str::to_owned);
    Meta {
        request_id: map.get("request_id").cloned().unwrap_or(Value::Null),
        method: text("method"),
        api_version: text("api_version"),
        solar_version: text("solar_version"),
        protocol: text("protocol"),
        started_at: text("started_at"),
        duration_us,
        os: text("os"),
        arch: text("arch"),
    }
}

fn unknown_members<'a>(map: &'a Map<String, Value>, known: &[&str]) -> Vec<&'a str> {
    map.keys()
        .map(String::as_str)
        .filter(|key| !known.contains(key))
        .collect()
}

/// The request line ZENITH writes for a call, with no newline.
#[must_use]
pub fn request_line(id: u64, method: &str, params: &Value) -> String {
    let mut request = Map::new();
    request.insert("jsonrpc".to_owned(), Value::String("2.0".to_owned()));
    request.insert("id".to_owned(), Value::from(id));
    request.insert("method".to_owned(), Value::String(method.to_owned()));
    request.insert("params".to_owned(), params.clone());
    Value::Object(request).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const PING: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"data":{"echo":"hi","pong":true,"received_at":"2026-09-27T02:58:14.819113Z"},"meta":{"request_id":1,"method":"solar.ping","api_version":"1.0.0","solar_version":"0.1.0","protocol":"solar/1","started_at":"2026-09-27T02:58:14.818953Z","duration_us":170,"os":"windows","arch":"x86_64"},"warnings":[]}}"#;

    const UNKNOWN_FIELD: &str = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"Invalid params for solar.ping: unknown field `mesage`, expected `message`.","data":{"status":"INVALID_ARGUMENT","reason":"UNKNOWN_FIELD","details":[{"field":"/mesage","expected":"one of: message","received":"hi","hint":"There is no mesage parameter. Did you mean message? A call that works: {\"message\":\"hi\"}.","docs":"docs/ERRORS.md#invalid_argument"}],"meta":{"request_id":1,"method":"solar.ping","api_version":"1.0.0","solar_version":"0.1.0","protocol":"solar/1","started_at":"2026-09-27T02:58:14.846800Z","duration_us":71,"os":"windows","arch":"x86_64"}}}}"#;

    fn single(line: &str) -> Envelope {
        match read(line) {
            Message::Single(envelope) => envelope,
            other => panic!("not a single response: {other:?}"),
        }
    }

    #[test]
    fn a_response_from_solar_s_readme_keeps_the_contract() {
        let envelope = single(PING);
        assert!(envelope.violations.is_empty(), "{:?}", envelope.violations);
        assert!(envelope.id.is(1));
        let Body::Success { data, warnings } = &envelope.body else {
            panic!("not a success");
        };
        assert_eq!(data["echo"], json!("hi"));
        assert!(warnings.is_empty());
        assert_eq!(envelope.meta.as_ref().unwrap().duration_us, Some(170));
    }

    #[test]
    fn an_error_from_solar_s_readme_keeps_the_contract_and_is_read_into_its_parts() {
        let envelope = single(UNKNOWN_FIELD);
        assert!(envelope.violations.is_empty(), "{:?}", envelope.violations);
        assert_eq!(envelope.status(), Some("INVALID_ARGUMENT"));
        let Body::Failure(failure) = &envelope.body else {
            panic!("not a failure");
        };
        assert_eq!(failure.reason.as_deref(), Some("UNKNOWN_FIELD"));
        assert_eq!(failure.details[0].field.as_deref(), Some("/mesage"));
        assert_eq!(failure.details[0].received, json!("hi"));
    }

    #[test]
    fn a_result_without_warnings_is_a_violation_of_section_5() {
        let mut value: Value = serde_json::from_str(PING).unwrap();
        value["result"].as_object_mut().unwrap().remove("warnings");
        let envelope = Envelope::of(&value);
        assert_eq!(envelope.violations.len(), 1);
        assert_eq!(envelope.violations[0].section, "5");
    }

    #[test]
    fn an_error_with_empty_details_or_a_wrong_code_is_a_violation() {
        let mut value: Value = serde_json::from_str(UNKNOWN_FIELD).unwrap();
        value["error"]["data"]["details"] = json!([]);
        value["error"]["code"] = json!(-32601);
        let sections: Vec<&str> = Envelope::of(&value)
            .violations
            .iter()
            .map(|violation| violation.section)
            .collect();
        assert_eq!(sections, vec!["6.1", "6"]);
    }

    #[test]
    fn a_status_the_contract_does_not_list_is_not_a_violation() {
        let mut value: Value = serde_json::from_str(UNKNOWN_FIELD).unwrap();
        value["error"]["code"] = json!(-32008);
        value["error"]["data"]["status"] = json!("CANCELLED");
        value["error"]["data"]["details"][0]["docs"] = json!("docs/ERRORS.md#cancelled");
        assert!(Envelope::of(&value).violations.is_empty());
    }

    #[test]
    fn a_request_id_that_differs_from_the_id_is_a_violation_of_section_7() {
        let mut value: Value = serde_json::from_str(PING).unwrap();
        value["result"]["meta"]["request_id"] = json!(2);
        let envelope = Envelope::of(&value);
        assert_eq!(envelope.violations[0].section, "7");
    }

    #[test]
    fn the_refusal_of_a_batch_by_solar_0_1_0_keeps_the_contract() {
        let line = r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32007,"message":"Batch requests are not implemented in solar/1.","data":{"status":"UNIMPLEMENTED","reason":"BATCH_NOT_SUPPORTED","details":[{"field":null,"expected":"a single JSON object","received":"array","hint":"Send one request per line.","docs":"docs/ERRORS.md#unimplemented"}],"meta":{"request_id":null,"method":null,"api_version":null,"solar_version":"0.1.0","protocol":"solar/1","started_at":"2026-09-27T15:47:00.996062Z","duration_us":11,"os":"windows","arch":"x86_64"}}}}"#;
        let envelope = single(line);
        assert!(envelope.violations.is_empty(), "{:?}", envelope.violations);
        assert!(envelope.id.is_null());
    }

    #[test]
    fn an_array_is_a_batch_and_text_that_is_not_json_is_said_to_be_so() {
        assert!(matches!(read(&format!("[{PING}]")), Message::Batch(items) if items.len() == 1));
        assert!(matches!(read("not json"), Message::NotJson { .. }));
    }

    #[test]
    fn the_request_line_is_the_json_rpc_envelope_in_the_contract_s_order() {
        assert_eq!(
            request_line(3, "solar.ping", &json!({"message": "hi"})),
            r#"{"jsonrpc":"2.0","id":3,"method":"solar.ping","params":{"message":"hi"}}"#
        );
    }
}
