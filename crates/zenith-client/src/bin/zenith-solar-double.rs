//! A stand-in for `solar serve --stdio`, for ZENITH's tests only.
//!
//! The real SOLAR cannot be made to exit early, speak another protocol, write garbage,
//! fall silent or answer a cancellation before its own stage has one. This double can,
//! chosen by the variable `ZENITH_DOUBLE`:
//!
//! | Value | What it does |
//! | ----- | ------------ |
//! | unset, `healthy` | answers `solar.version`, `solar.manifest`, `solar.ping` and `double.slow` |
//! | `cancel` | the same, plus `solar.cancel` with the queue section 9 of SOLAR's draft contract describes |
//! | `exit:<code>` | writes one line to standard error and exits with that code at once |
//! | `protocol:<name>` | reports that protocol in `solar.version` |
//! | `layout:<version>` | reports that manifest layout |
//! | `garbage` | answers every request with a line that is not JSON |
//! | `silent` | reads every request and answers none |
//! | `die-after:<n>` | answers `n` requests, then writes a panic to standard error and exits 101 |
//! | `huge` | answers `solar.ping` with a line of 17 MiB |
//! | `unexpected` | answers every request after the handshake with an extra response nobody asked for |
//! | `no-warnings` | answers `solar.ping` without `result.warnings`, which breaks section 5 |
//!
//! It is built from SOLAR's contract, never from SOLAR's source, and it is not SOLAR: its
//! version is `0.0.0-double`, which no SOLAR has.

#![allow(
    clippy::print_stderr,
    reason = "the double writes its diagnostics to standard error, as SOLAR does"
)]

use std::collections::VecDeque;
use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

fn main() {
    let mode = std::env::var("ZENITH_DOUBLE").unwrap_or_else(|_| "healthy".to_owned());
    if let Some(code) = mode.strip_prefix("exit:") {
        eprintln!("double: exiting with {code} before reading anything");
        std::process::exit(code.parse().unwrap_or(1));
    }
    let output = Arc::new(Mutex::new(io::stdout()));
    if mode == "cancel" {
        serve_with_cancellation(&output);
    } else {
        serve(&mode, &output);
    }
}

fn write_line(output: &Mutex<io::Stdout>, line: &str) {
    if let Ok(stdout) = output.lock() {
        let mut stdout = stdout.lock();
        let _ = stdout.write_all(line.as_bytes());
        let _ = stdout.write_all(b"\n");
        let _ = stdout.flush();
    }
    log("trace", &format!("<-- {line}"));
}

fn log(level: &str, message: &str) {
    let line = json!({
        "time": "2026-09-27T12:00:00.000000Z",
        "level": level,
        "message": message,
        "request_id": null,
        "method": null,
        "duration_us": null,
    });
    eprintln!("{line}");
}

fn serve(mode: &str, output: &Mutex<io::Stdout>) {
    let stdin = io::stdin();
    let mut answered = 0_u64;
    let limit = mode
        .strip_prefix("die-after:")
        .and_then(|count| count.parse::<u64>().ok());
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        log("trace", &format!("--> {line}"));
        if mode == "silent" {
            continue;
        }
        if mode == "garbage" {
            write_line(output, "this is not json");
            continue;
        }
        let request: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
        if mode == "unexpected" && answered >= 2 {
            write_line(
                output,
                &success(&json!(999), "solar.ping", &json!({"pong": true})),
            );
        }
        write_line(output, &answer(mode, &request));
        answered += 1;
        if limit.is_some_and(|limit| answered >= limit) {
            eprintln!("thread 'main' panicked at src/double.rs:1:1:\nthe double was told to die");
            std::process::exit(101);
        }
    }
    log("info", &format!("the session ended after {answered} calls"));
}

fn answer(mode: &str, request: &Value) -> String {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
    match method {
        "solar.version" => {
            let protocol = mode.strip_prefix("protocol:").unwrap_or("solar/1");
            let layout = mode.strip_prefix("layout:").unwrap_or("2.0.0");
            success(
                &id,
                method,
                &json!({
                    "solar_version": "0.0.0-double",
                    "protocol": protocol,
                    "manifest_schema_version": layout,
                    "build": {"profile": "test", "target": "double"}
                }),
            )
        }
        "solar.manifest" => success(&id, method, &manifest(mode, false)),
        "solar.ping" if mode == "huge" => {
            let blob = "x".repeat(17 * 1024 * 1024);
            success(&id, method, &json!({"pong": true, "blob": blob}))
        }
        "solar.ping" if mode == "no-warnings" => json!({
            "jsonrpc": "2.0", "id": id,
            "result": {"data": {"pong": true}, "meta": meta(&id, Some(method))}
        })
        .to_string(),
        "solar.ping" => success(
            &id,
            method,
            &json!({"pong": true, "echo": params.get("message").cloned().unwrap_or(Value::Null),
                    "received_at": "2026-09-27T12:00:00.000000Z"}),
        ),
        "double.slow" => {
            let milliseconds = params.get("ms").and_then(Value::as_u64).unwrap_or(0);
            thread::sleep(Duration::from_millis(milliseconds));
            success(&id, method, &json!({"slept_ms": milliseconds}))
        }
        _ => failure(&id, Some(method), "NOT_FOUND", -32601, "METHOD_NOT_FOUND"),
    }
}

fn manifest(mode: &str, cancel: bool) -> Value {
    let layout = mode.strip_prefix("layout:").unwrap_or("2.0.0");
    let mut apis = vec![
        api(
            "double.slow",
            "Sleeps for as long as it is asked to",
            &json!({"type": "object", "additionalProperties": false,
                   "properties": {"ms": {"type": "integer", "minimum": 0}}}),
            &json!([{"name": "short", "description": "A tenth of a second", "params": {"ms": 100},
                    "response": {"slept_ms": 100}, "match": "exact"}]),
        ),
        api(
            "solar.manifest",
            "Returns the manifest",
            &json!({"type": "object", "additionalProperties": false, "properties": {}}),
            &json!([{"name": "all", "description": "Everything", "params": {},
                    "response": {"apis": "$any"}, "match": "subset"}]),
        ),
        api(
            "solar.ping",
            "Answers immediately",
            &json!({"type": "object", "additionalProperties": false,
                   "properties": {"message": {"type": ["string", "null"], "default": null}}}),
            &json!([{"name": "bare", "description": "No message", "params": {},
                    "response": {"echo": null, "pong": true, "received_at": "$any"},
                    "match": "exact"}]),
        ),
        api(
            "solar.version",
            "Reports the version",
            &json!({"type": "object", "additionalProperties": false, "properties": {}}),
            &json!([{"name": "plain", "description": "The versions", "params": {},
                    "response": {"protocol": "$any"}, "match": "subset"}]),
        ),
    ];
    if cancel {
        apis.push(api(
            "solar.cancel",
            "Cancels a call by its id",
            &json!({"type": "object", "additionalProperties": false, "required": ["id"],
                   "properties": {"id": {"type": ["integer", "string"]}}}),
            &json!([{"name": "unknown", "description": "An id never seen", "params": {"id": 424_242},
                    "response": {"outcome": "unknown"}, "match": "subset"}]),
        ));
    }
    json!({
        "schema_version": layout,
        "schema_dialect": "https://json-schema.org/draft/2020-12/schema",
        "solar_version": "0.0.0-double",
        "protocol": "solar/1",
        "$defs": {},
        "apis": apis,
    })
}

fn api(name: &str, summary: &str, params: &Value, examples: &Value) -> Value {
    json!({
        "name": name, "version": "1.0.0", "summary": summary,
        "description": format!("{summary}, in ZENITH's test double."),
        "errors": [], "side_effects": ["none"], "idempotent": true,
        "stability": "experimental", "since": "0.0.0", "timeout_ms": 2000,
        "params_schema": params, "output_schema": {"type": "object"}, "examples": examples,
    })
}

fn meta(id: &Value, method: Option<&str>) -> Value {
    json!({
        "request_id": id, "method": method, "api_version": method.map(|_| "1.0.0"),
        "solar_version": "0.0.0-double", "protocol": "solar/1",
        "started_at": "2026-09-27T12:00:00.000000Z", "duration_us": 7,
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
    })
}

fn success(id: &Value, method: &str, data: &Value) -> String {
    json!({"jsonrpc": "2.0", "id": id,
           "result": {"data": data, "meta": meta(id, Some(method)), "warnings": []}})
    .to_string()
}

fn failure(id: &Value, method: Option<&str>, status: &str, code: i64, reason: &str) -> String {
    json!({"jsonrpc": "2.0", "id": id, "error": {
        "code": code,
        "message": format!("The double refuses this with {reason}."),
        "data": {
            "status": status, "reason": reason,
            "details": [{"field": null, "expected": null, "received": null, "hint": null,
                         "docs": format!("docs/ERRORS.md#{}", status.to_lowercase())}],
            "meta": meta(id, method),
        }
    }})
    .to_string()
}

/// A call waiting in the queue, or running.
struct Queued {
    id: Value,
    request: Value,
}

#[derive(Default)]
struct Queue {
    waiting: VecDeque<Queued>,
    running: Option<Value>,
    finished: Vec<Value>,
}

/// The session of section 9.1 of SOLAR's draft contract: requests are read on a thread
/// of their own into a queue, calls run one at a time in order, and only `solar.cancel`
/// is answered as soon as it is read.
fn serve_with_cancellation(output: &Arc<Mutex<io::Stdout>>) {
    let queue = Arc::new(Mutex::new(Queue::default()));
    let cancelled = Arc::new(AtomicBool::new(false));
    let (wake, woken) = mpsc::channel::<()>();
    let worker = {
        let queue = Arc::clone(&queue);
        let cancelled = Arc::clone(&cancelled);
        let output = Arc::clone(output);
        thread::spawn(move || {
            while woken.recv().is_ok() {
                loop {
                    let next = queue.lock().ok().and_then(|mut queue| {
                        let next = queue.waiting.pop_front()?;
                        queue.running = Some(next.id.clone());
                        Some(next)
                    });
                    let Some(call) = next else { break };
                    cancelled.store(false, Ordering::SeqCst);
                    let line = run(&call.request, &cancelled);
                    write_line(&output, &line);
                    if let Ok(mut queue) = queue.lock() {
                        queue.running = None;
                        queue.finished.push(call.id);
                    }
                }
            }
        })
    };
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        log("trace", &format!("--> {line}"));
        let request: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        if request.get("method").and_then(Value::as_str) == Some("solar.cancel") {
            let target = request
                .get("params")
                .and_then(|params| params.get("id"))
                .cloned()
                .unwrap_or(Value::Null);
            let outcome = cancel(&queue, &cancelled, &target, output);
            write_line(
                output,
                &success(&id, "solar.cancel", &json!({"outcome": outcome})),
            );
            continue;
        }
        if let Ok(mut queue) = queue.lock() {
            queue.waiting.push_back(Queued { id, request });
        }
        let _ = wake.send(());
    }
    drop(wake);
    let _ = worker.join();
}

fn cancel(
    queue: &Mutex<Queue>,
    cancelled: &AtomicBool,
    target: &Value,
    output: &Mutex<io::Stdout>,
) -> &'static str {
    let Ok(mut queue) = queue.lock() else {
        return "unknown";
    };
    if let Some(position) = queue.waiting.iter().position(|call| &call.id == target) {
        if let Some(call) = queue.waiting.remove(position) {
            let method = call.request.get("method").and_then(Value::as_str);
            write_line(
                output,
                &failure(&call.id, method, "CANCELLED", -32008, "CALL_CANCELLED"),
            );
            queue.finished.push(call.id);
        }
        return "cancelled_while_queued";
    }
    if queue.running.as_ref() == Some(target) {
        cancelled.store(true, Ordering::SeqCst);
        return "cancellation_requested";
    }
    if queue.finished.contains(target) {
        return "already_finished";
    }
    "unknown"
}

fn run(request: &Value, cancelled: &AtomicBool) -> String {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    match method {
        "solar.manifest" => success(&id, method, &manifest("cancel", true)),
        "double.slow" => {
            let milliseconds = request
                .get("params")
                .and_then(|params| params.get("ms"))
                .and_then(Value::as_u64)
                .unwrap_or(0);
            for _ in 0..milliseconds.div_ceil(5) {
                if cancelled.load(Ordering::SeqCst) {
                    return failure(&id, Some(method), "CANCELLED", -32008, "CALL_CANCELLED");
                }
                thread::sleep(Duration::from_millis(5));
            }
            success(&id, method, &json!({"slept_ms": milliseconds}))
        }
        _ => answer("healthy", request),
    }
}
