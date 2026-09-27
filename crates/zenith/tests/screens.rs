//! Every screen and every state, drawn with ratatui's test backend and snapshotted with
//! every colour of every cell: at 80 × 24 and at 160 × 48, in each theme, and without
//! colour and without Unicode.
//!
//! The scenes are scripted with the answers SOLAR 0.1.0 gives, and every time in them is
//! fixed, so a snapshot changes only when the drawing does. `cargo insta review` shows
//! the difference when one does.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test reports failure by panicking"
)]

mod common;

use std::time::Duration;

use common::{Script, characters, draw, screen, success};
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use serde_json::{Value, json};
use zenith::app::link::Failure;
use zenith::app::{Incoming, Options};
use zenith::glyphs::Charset;
use zenith::theme::{Depth, ThemeName};
use zenith_client::connection::Event as ConnectionEvent;
use zenith_client::locate::LocateError;

const SIZES: [(u16, u16); 2] = [(80, 24), (160, 48)];

type Scene = fn(Options) -> Script;

fn options(theme: ThemeName, depth: Depth, charset: Charset) -> Options {
    Options {
        theme,
        depth,
        charset,
        opening: false,
        ..Options::default()
    }
}

fn system_info() -> Value {
    json!({
        "arch": "x86_64", "cpu_count": 16, "current_dir": "/home/lab/zenith",
        "executable": "/home/lab/solar/target/release/solar", "os": "linux", "os_build": null,
        "os_family": "unix", "os_name": "AlmaLinux", "os_release": "9.8",
        "path_list_separator": ":", "path_separator": "/", "pointer_width": 64,
        "temp_dir": "/tmp"
    })
}

fn api_not_found(id: u64) -> String {
    let id_value = json!(id);
    json!({"jsonrpc": "2.0", "id": id, "error": {
        "code": -32601,
        "message": "No API is registered under the name solar.pign.",
        "data": {"status": "NOT_FOUND", "reason": "API_NOT_FOUND",
                 "details": [{"field": "/api", "expected": "solar.ping", "received": "solar.pign",
                              "hint": "Did you mean \"solar.ping\"? The edit distance is 2.",
                              "docs": "docs/ERRORS.md#not_found"}],
                 "meta": common::meta(&id_value, "solar.describe", 58)}}})
    .to_string()
}

fn opening_connecting(options: Options) -> Script {
    Script::with(Options {
        opening: true,
        ..options
    })
}

fn opening_not_on_path(options: Options) -> Script {
    let mut script = opening_connecting(options);
    script.later(Duration::from_millis(9));
    script.feed(Incoming::Started {
        generation: 1,
        result: Err(Failure::Locate(LocateError::NotOnPath { directories: 23 })),
    });
    script
}

fn session_just_connected(options: Options) -> Script {
    let mut script = Script::connected(options);
    script.later(Duration::from_secs(42));
    script.feed(Incoming::Tick);
    script
}

fn session_after_calls(options: Options) -> Script {
    let mut script = Script::connected(options);
    script.later(Duration::from_secs(3));
    script.run("/ping hello");
    script.answer(
        "solar.ping",
        &json!({"echo": "hello", "pong": true, "received_at": "2026-09-27T15:47:03.100521Z"}),
        520,
    );
    script.run("/call system.info");
    script.answer("system.info", &system_info(), 610);
    script.run("/describe solar.pign");
    let id = script.waiting_id();
    script.later(Duration::from_micros(310));
    script.line(&api_not_found(id));
    script.run("/call solar.version");
    script.later(Duration::from_millis(300));
    script.feed(Incoming::Tick);
    script
}

fn session_commands_menu(options: Options) -> Script {
    let mut script = session_just_connected(options);
    script.type_text("/");
    script
}

fn session_parameters_menu(options: Options) -> Script {
    let mut script = session_just_connected(options);
    script.type_text("/call solar.describe {");
    script
}

fn session_validation_error(options: Options) -> Script {
    let mut script = session_just_connected(options);
    script.run(r#"/call solar.ping {"mesage": "hi"}"#);
    script
}

fn session_disconnected(options: Options) -> Script {
    let mut script = Script::connected(options);
    script.later(Duration::from_secs(192));
    script.run("/ping");
    script.stderr("thread 'main' panicked at crates/solar-core/src/dispatch.rs:120:9:");
    script.stderr("the dispatcher was told to fall over");
    script.feed(Incoming::Connection {
        generation: 1,
        event: ConnectionEvent::OutputClosed {
            error: None,
            at: script.now,
        },
    });
    script.feed(Incoming::Exited {
        generation: 1,
        how: "exited with code 101".to_owned(),
    });
    script
}

fn apis_catalogue(options: Options) -> Script {
    let mut script = session_just_connected(options);
    script.key(KeyCode::Tab, KeyModifiers::NONE);
    script.key(KeyCode::Down, KeyModifiers::NONE);
    script.key(KeyCode::Down, KeyModifiers::NONE);
    script.key(KeyCode::Char('1'), KeyModifiers::NONE);
    script.answer(
        "solar.ping",
        &json!({"echo": null, "pong": true, "received_at": "2026-09-27T15:47:42.100000Z"}),
        480,
    );
    script.key(KeyCode::Char('2'), KeyModifiers::NONE);
    script.answer(
        "solar.ping",
        &json!({"echo": null, "pong": true, "received_at": "2026-09-27T15:47:42.200000Z"}),
        470,
    );
    script
}

fn apis_form(options: Options) -> Script {
    let mut script = session_just_connected(options);
    script.key(KeyCode::Tab, KeyModifiers::NONE);
    script.key(KeyCode::Enter, KeyModifiers::NONE);
    script.key(KeyCode::Enter, KeyModifiers::NONE);
    script
}

fn log_at_trace(options: Options) -> Script {
    let mut script = session_just_connected(options);
    for line in [
        r#"{"duration_us":null,"level":"trace","message":"--> {\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"solar.version\",\"params\":{}}","method":null,"request_id":null,"time":"2026-09-27T15:47:00.000412Z"}"#,
        r#"{"duration_us":null,"level":"trace","message":"<-- {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"data\":{\"solar_version\":\"0.1.0\"}}}","method":null,"request_id":null,"time":"2026-09-27T15:47:00.000601Z"}"#,
        r#"{"duration_us":170,"level":"debug","message":"answered","method":"solar.ping","request_id":"3","time":"2026-09-27T15:47:03.100521Z"}"#,
        r#"{"duration_us":null,"level":"warn","message":"the os-release file could not be read","method":"system.info","request_id":"4","time":"2026-09-27T15:47:03.200000Z"}"#,
        r#"{"duration_us":null,"level":"info","message":"the session ended after 4 calls","method":null,"request_id":null,"time":"2026-09-27T15:47:09.000000Z"}"#,
        "thread 'main' panicked at crates/solar-core/src/dispatch.rs:120:9:",
    ] {
        script.stderr(line);
    }
    script.key(KeyCode::Tab, KeyModifiers::NONE);
    script.key(KeyCode::Tab, KeyModifiers::NONE);
    script.key(KeyCode::Char('t'), KeyModifiers::NONE);
    script
}

fn history(options: Options) -> Script {
    let mut script = session_after_calls(options);
    script.key(KeyCode::Char('u'), KeyModifiers::CONTROL);
    script.key(KeyCode::BackTab, KeyModifiers::SHIFT);
    script.key(KeyCode::Up, KeyModifiers::NONE);
    script
}

fn help_overlay(options: Options) -> Script {
    let mut script = session_just_connected(options);
    script.key(KeyCode::Char('?'), KeyModifiers::SHIFT);
    script
}

fn envelope_viewer(options: Options) -> Script {
    let mut script = session_just_connected(options);
    script.run("/ping hello");
    script.answer(
        "solar.ping",
        &json!({"echo": "hello", "pong": true, "received_at": "2026-09-27T15:47:42.100521Z"}),
        520,
    );
    script.key(KeyCode::Char('o'), KeyModifiers::CONTROL);
    script
}

const SCENES: [(&str, Scene); 14] = [
    ("opening_connecting", opening_connecting),
    ("opening_not_on_path", opening_not_on_path),
    ("session_just_connected", session_just_connected),
    ("session_after_calls", session_after_calls),
    ("session_commands_menu", session_commands_menu),
    ("session_parameters_menu", session_parameters_menu),
    ("session_validation_error", session_validation_error),
    ("session_disconnected", session_disconnected),
    ("apis_catalogue", apis_catalogue),
    ("apis_form", apis_form),
    ("log_at_trace", log_at_trace),
    ("history", history),
    ("help_overlay", help_overlay),
    ("envelope_viewer", envelope_viewer),
];

#[test]
fn every_screen_in_every_theme_at_both_sizes() {
    for theme in ThemeName::ALL {
        for (name, scene) in SCENES {
            for (width, height) in SIZES {
                let script = scene(options(theme, Depth::TrueColor, Charset::Unicode));
                let snapshot = format!("{name}__{width}x{height}__{}", theme.name());
                insta::assert_snapshot!(snapshot, screen(&draw(&script.app, width, height)));
            }
        }
    }
}

#[test]
fn every_screen_without_colour_and_in_ascii() {
    for (name, scene) in SCENES {
        let script = scene(options(ThemeName::Night, Depth::None, Charset::Ascii));
        let buffer = draw(&script.app, 80, 24);
        for cell in buffer.content() {
            assert!(
                cell.symbol().is_ascii(),
                "{name} drew {:?} in ASCII",
                cell.symbol()
            );
        }
        let snapshot = format!("{name}__80x24__no_colour_ascii");
        insta::assert_snapshot!(snapshot, screen(&buffer));
    }
}

#[test]
fn the_fallback_depths_draw_in_their_own_colours() {
    for depth in [Depth::Indexed, Depth::Sixteen] {
        for theme in ThemeName::ALL {
            let script = session_after_calls(options(theme, depth, Charset::Unicode));
            let snapshot = format!(
                "session_after_calls__80x24__{}__{}",
                theme.name(),
                depth.name()
            );
            insta::assert_snapshot!(snapshot, screen(&draw(&script.app, 80, 24)));
        }
    }
}

#[test]
fn a_terminal_smaller_than_80_by_24_is_told_so_and_nothing_breaks() {
    for (width, height) in [(79, 24), (80, 23), (40, 10), (1, 1)] {
        let script = session_after_calls(options(
            ThemeName::Night,
            Depth::TrueColor,
            Charset::Unicode,
        ));
        let text = characters(&draw(&script.app, width, height));
        if width > 30 {
            assert!(text.contains("ZENITH needs"), "{width}x{height}:\n{text}");
        }
    }
    let script = session_after_calls(options(
        ThemeName::Night,
        Depth::TrueColor,
        Charset::Unicode,
    ));
    insta::assert_snapshot!("too_small__79x24", characters(&draw(&script.app, 79, 24)));
}

#[test]
fn at_80_by_24_every_tab_keeps_its_header_and_its_status_bar() {
    for (name, scene) in SCENES
        .iter()
        .filter(|(name, _)| !name.starts_with("opening"))
    {
        let script = scene(options(
            ThemeName::Night,
            Depth::TrueColor,
            Charset::Unicode,
        ));
        let text = characters(&draw(&script.app, 80, 24));
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with(" ZENITH"), "{name}: {}", lines[0]);
        let status = lines[23];
        assert!(
            status.contains("in orbit") || status.contains("disconnected"),
            "{name}: {status}"
        );
    }
}

#[test]
fn every_resize_between_the_minimum_and_a_large_screen_draws_without_panicking() {
    let scenes: Vec<Script> = SCENES
        .iter()
        .map(|(_, scene)| {
            scene(options(
                ThemeName::Light,
                Depth::TrueColor,
                Charset::Unicode,
            ))
        })
        .collect();
    for script in &scenes {
        for width in (80..=200).step_by(7) {
            for height in (24..=60).step_by(5) {
                drop(draw(&script.app, width, height));
            }
        }
    }
}

#[test]
fn nothing_solar_sends_can_put_an_escape_sequence_on_the_screen() {
    let mut script = session_just_connected(options(
        ThemeName::Night,
        Depth::TrueColor,
        Charset::Unicode,
    ));
    script.run("/ping x");
    script.answer(
        "solar.ping",
        &json!({"echo": "\u{1b}[2J\u{1b}]0;owned\u{7}", "pong": true}),
        500,
    );
    script.run("/call system.info");
    let id = script.waiting_id();
    script.line(&success(
        id,
        "system.info",
        &json!({"os\u{1b}[31m": "\u{1b}[5mblink"}),
    ));
    for (width, height) in SIZES {
        let buffer = draw(&script.app, width, height);
        for cell in buffer.content() {
            assert!(
                !cell.symbol().chars().any(char::is_control),
                "a control character reached the screen: {:?}",
                cell.symbol()
            );
        }
    }
}
