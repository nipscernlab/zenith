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

/// `/list` answered by a SOLAR whose manifest declares what the protocol accepts.
fn session_list_capabilities(options: Options) -> Script {
    let mut script = Script::connected(options);
    script.later(Duration::from_secs(2));
    script.run("/list");
    let manifest: Value = serde_json::from_str(include_str!(
        "../../zenith-client/tests/fixtures/solar-0.3.0-manifest.json"
    ))
    .unwrap();
    script.answer("solar.manifest", &manifest, 900);
    script
}

fn session_version(options: Options) -> Script {
    let mut script = Script::connected(options);
    script.later(Duration::from_secs(2));
    script.run("/version");
    script.answer(
        "solar.version",
        &json!({"solar_version": "0.1.0", "protocol": "solar/1",
                "manifest_schema_version": "2.0.0",
                "build": {"git_commit_short": "c9c566b22e2b", "profile": "release",
                          "rustc_version": "rustc 1.97.1", "target": "x86_64-pc-windows-msvc"}}),
        480,
    );
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
    script.feed(Incoming::Connection {
        generation: 1,
        event: ConnectionEvent::StderrClosed,
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

/// The help at its last page, which says what the mouse does.
fn help_overlay_end(options: Options) -> Script {
    let mut script = help_overlay(options);
    script.key(KeyCode::End, KeyModifiers::NONE);
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

const SCENES: [(&str, Scene); 17] = [
    ("opening_connecting", opening_connecting),
    ("opening_not_on_path", opening_not_on_path),
    ("session_just_connected", session_just_connected),
    ("session_after_calls", session_after_calls),
    ("session_version", session_version),
    ("session_list_capabilities", session_list_capabilities),
    ("session_commands_menu", session_commands_menu),
    ("session_parameters_menu", session_parameters_menu),
    ("session_validation_error", session_validation_error),
    ("session_disconnected", session_disconnected),
    ("apis_catalogue", apis_catalogue),
    ("apis_form", apis_form),
    ("log_at_trace", log_at_trace),
    ("history", history),
    ("help_overlay", help_overlay),
    ("help_overlay_end", help_overlay_end),
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

/// The opening carries ZENITH's mark, and the mark is drawn in every theme at every depth
/// a terminal may have, down to none.
#[test]
fn the_opening_in_every_theme_at_every_depth() {
    for depth in [Depth::Indexed, Depth::Sixteen, Depth::None] {
        for theme in ThemeName::ALL {
            let script = opening_connecting(options(theme, depth, Charset::Unicode));
            let snapshot = format!(
                "opening_connecting__80x24__{}__{}",
                theme.name(),
                depth.name()
            );
            insta::assert_snapshot!(snapshot, screen(&draw(&script.app, 80, 24)));
        }
    }
}

/// The Session tab's sky: the rows above a transcript too short to fill the tab show the
/// stars the opening showed in those very cells, with no star on the row just above the
/// first line; a transcript that fills the tab leaves no sky.
#[test]
fn a_short_transcript_sits_under_the_stars_of_the_opening() {
    let night = || options(ThemeName::Night, Depth::TrueColor, Charset::Unicode);
    let stars = zenith::glyphs::Glyphs::of(Charset::Unicode).stars;
    let is_star = |symbol: &str| stars.contains(&symbol);
    let opening = draw(&opening_connecting(night()).app, 80, 24);
    let session = draw(&session_just_connected(night()).app, 80, 24);
    let row_of = |buffer: &ratatui::buffer::Buffer, y: u16| -> Vec<String> {
        (0..80)
            .map(|x| buffer[(x, y)].symbol().to_owned())
            .collect()
    };
    // The first row under the header with something on it other than stars.
    let first = (1..24)
        .find(|y| {
            row_of(&session, *y)
                .iter()
                .any(|symbol| symbol != " " && !is_star(symbol))
        })
        .unwrap();
    let mut seen = 0;
    for y in 1..first {
        for (x, symbol) in row_of(&session, y).iter().enumerate() {
            if is_star(symbol) {
                seen += 1;
                assert_eq!(
                    opening[(u16::try_from(x).unwrap(), y)].symbol(),
                    symbol,
                    "the star at {x}, {y} is not where the opening had one"
                );
            }
        }
    }
    assert!(seen > 3, "only {seen} stars in the sky");
    assert!(
        !row_of(&session, first - 1)
            .iter()
            .any(|symbol| is_star(symbol)),
        "a star touches the first line"
    );
    // The stars are still: a later frame of the same session draws the same sky.
    let mut later = session_just_connected(night());
    later.later(Duration::from_secs(5));
    later.feed(Incoming::Tick);
    let redrawn = draw(&later.app, 80, 24);
    for y in 1..first {
        assert_eq!(row_of(&redrawn, y), row_of(&session, y), "row {y} moved");
    }
    // A transcript that fills the tab leaves no sky: no row of stars and nothing else. The
    // dot between a card's facts is a star's glyph too, which is why rows and not glyphs
    // are counted.
    let full = draw(&session_after_calls(night()).app, 80, 24);
    for y in 1..20 {
        let row = row_of(&full, y);
        let sky = row.iter().any(|symbol| is_star(symbol))
            && row.iter().all(|symbol| symbol == " " || is_star(symbol));
        assert!(!sky, "row {y} of a full transcript is sky");
    }
}

/// ZENITH's mark stands for ZENITH and SOLAR's for SOLAR: the opening draws ZENITH's and
/// not SOLAR's, and where ZENITH shows SOLAR itself, the start of a connection and the
/// card of `/version`, it draws SOLAR's and not ZENITH's.
#[test]
fn each_mark_is_drawn_where_it_stands_for_its_own() {
    use zenith::brand::{SOLAR, ZENITH};
    for charset in [Charset::Unicode, Charset::Ascii] {
        let rows = |mark: &zenith::brand::Mark| -> Vec<String> {
            mark.rows(charset)
                .iter()
                .map(|row| row.trim().to_owned())
                .collect()
        };
        let has = |text: &str, mark: &zenith::brand::Mark| {
            rows(mark).iter().all(|row| text.contains(row.as_str()))
        };
        let opening = opening_connecting(options(ThemeName::Night, Depth::None, charset));
        let text = characters(&draw(&opening.app, 160, 48));
        assert!(has(&text, &ZENITH), "{text}");
        assert!(!has(&text, &SOLAR), "{text}");
        // The first screen after connecting, even at the smallest size, when the opening
        // was too quick to be seen.
        let connected = session_just_connected(options(ThemeName::Night, Depth::None, charset));
        let text = characters(&draw(&connected.app, 80, 24));
        assert!(has(&text, &SOLAR), "{text}");
        assert!(!has(&text, &ZENITH), "{text}");
        let version = session_version(options(ThemeName::Night, Depth::None, charset));
        let text = characters(&draw(&version.app, 160, 48));
        assert!(has(&text, &SOLAR), "{text}");
        assert!(!has(&text, &ZENITH), "{text}");
        assert!(text.contains("The central API of the Constellation"));
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

/// The screens of the README: the session recorded against a real SOLAR, replayed at the
/// moment and with the timings it was recorded with. `cargo xtask screenshots` draws the
/// README's pictures from these snapshots, and CI checks that they are the same.
#[test]
fn the_screens_the_readme_shows() {
    let night = || options(ThemeName::Night, Depth::TrueColor, Charset::Unicode);
    let all = common::README_SCENARIO.len();
    let mut scenes: Vec<(&str, Script)> = Vec::new();
    scenes.push((
        "readme_session__100x30__night",
        Script::replayed(night(), 4),
    ));
    scenes.push((
        "readme_session__100x30__light",
        Script::replayed(
            options(ThemeName::Light, Depth::TrueColor, Charset::Unicode),
            4,
        ),
    ));
    scenes.push(("readme_apis__100x30__night", Script::replayed(night(), all)));
    let mut log = Script::replayed(night(), all);
    log.key(KeyCode::Tab, KeyModifiers::NONE);
    log.key(KeyCode::Char('t'), KeyModifiers::NONE);
    scenes.push(("readme_log__100x30__night", log));
    let mut history = Script::replayed(night(), all);
    history.key(KeyCode::Tab, KeyModifiers::NONE);
    history.key(KeyCode::Tab, KeyModifiers::NONE);
    history.key(KeyCode::Up, KeyModifiers::NONE);
    history.key(KeyCode::Up, KeyModifiers::NONE);
    scenes.push(("readme_history__100x30__night", history));
    let mut menu = Script::replayed(night(), 4);
    menu.type_text("/call solar.describe {");
    scenes.push(("readme_menu__100x30__night", menu));
    let mut opening = Script::with(Options {
        opening: true,
        ..options(ThemeName::HighContrast, Depth::TrueColor, Charset::Unicode)
    });
    for _ in 0..3 {
        opening.later(Duration::from_millis(90));
        opening.feed(Incoming::Tick);
    }
    scenes.push(("readme_opening__100x30__high-contrast", opening));
    for (name, script) in scenes {
        insta::assert_snapshot!(name, screen(&draw(&script.app, 100, 30)));
    }
}
