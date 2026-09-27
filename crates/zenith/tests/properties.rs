//! Anything at all, typed or received, leaves ZENITH standing. The scanner, the parser, the
//! completer and the whole application are fed text, keys and responses that no test would
//! think to write: none of them panics, and nothing they return points outside the line.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "a test reports failure by panicking"
)]

mod common;

use std::ops::Range;
use std::sync::OnceLock;
use std::time::Duration;

use common::{MANIFEST, Script, draw, success};
use proptest::prelude::*;
use proptest::sample::Index;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use serde_json::{Number, Value};
use zenith::app::{Incoming, Options};
use zenith::commands::parse;
use zenith::completion::complete;
use zenith_client::json_text::{position_at, spans};
use zenith_client::manifest::Catalogue;

fn catalogue() -> &'static Catalogue {
    static CATALOGUE: OnceLock<Catalogue> = OnceLock::new();
    CATALOGUE.get_or_init(|| {
        let manifest: Value = serde_json::from_str(MANIFEST).unwrap();
        Catalogue::from_data(&manifest).unwrap()
    })
}

/// Lines like the ones people type, and lines like nothing anyone types.
fn line() -> impl Strategy<Value = String> {
    let pieces = prop::sample::select(vec![
        "/",
        "call ",
        "describe ",
        "ping ",
        "raw ",
        "theme ",
        "help ",
        "export ",
        "solar.",
        "ping",
        "describe",
        "system.info",
        " ",
        "  ",
        "{",
        "}",
        "[",
        "]",
        "\"",
        ":",
        ",",
        "message",
        "api",
        "\\",
        "\\u00",
        "null",
        "true",
        "1e999",
        "-0",
        "0.1",
        "\u{0}",
        "\u{1b}[31m",
        "é",
        "𝄞",
        "\u{200b}",
        "\t",
        "\n",
    ]);
    prop_oneof![
        any::<String>(),
        prop::collection::vec(pieces, 0..24).prop_map(|parts| parts.concat()),
    ]
}

/// A byte offset into `text` that is a character boundary, as the line editor keeps it.
fn boundary(text: &str, choice: Index) -> usize {
    let boundaries: Vec<usize> = text
        .char_indices()
        .map(|(at, _)| at)
        .chain([text.len()])
        .collect();
    boundaries[choice.index(boundaries.len())]
}

fn within(text: &str, range: &Range<usize>) -> bool {
    range.start <= range.end
        && range.end <= text.len()
        && text.is_char_boundary(range.start)
        && text.is_char_boundary(range.end)
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 2048,
        ..ProptestConfig::default()
    })]

    #[test]
    fn the_scanner_takes_any_text(text in line(), choice in any::<Index>()) {
        let _ = spans(&text);
        let _ = position_at(&text, boundary(&text, choice));
    }

    #[test]
    fn a_line_that_is_not_a_command_says_why_about_a_part_of_it(text in line()) {
        let names: Vec<&str> = catalogue().names().collect();
        if let Err(error) = parse(&text, &names) {
            prop_assert!(!error.message.is_empty());
            if let Some(span) = &error.span {
                prop_assert!(within(&text, span), "{:?} in {:?}", span, text);
            }
        }
    }

    #[test]
    fn a_completion_replaces_a_part_of_the_line(text in line(), choice in any::<Index>()) {
        let cursor = boundary(&text, choice);
        if let Some(completion) = complete(&text, cursor, Some(catalogue())) {
            prop_assert!(
                within(&text, &completion.replace),
                "{:?} in {:?}",
                completion.replace,
                text
            );
            prop_assert!(!completion.candidates.is_empty());
        }
    }
}

/// Something that happens to a running ZENITH.
#[derive(Debug, Clone)]
enum Happening {
    Key(KeyCode, KeyModifiers),
    Paste(String),
    Solar(String),
    Answer(Value),
    Stderr(String),
    Later(u64),
    Tick,
    Resize(u16, u16),
    Exited,
    Reconnected,
}

fn key_code() -> impl Strategy<Value = KeyCode> {
    let characters = vec![
        '/', '?', ' ', '{', '}', '"', ':', ',', '1', '2', '3', 'j', 'k', 'g', 'G', 'x', 't', 'i',
        'e', 'w', 'd', 'q', 'c', 'a', 'l', 's', 'p', 'n', 'o', 'r', '.',
    ];
    let keys = vec![
        KeyCode::Enter,
        KeyCode::Tab,
        KeyCode::BackTab,
        KeyCode::Backspace,
        KeyCode::Delete,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::PageUp,
        KeyCode::PageDown,
        KeyCode::Esc,
        KeyCode::Insert,
        KeyCode::F(1),
    ];
    prop_oneof![
        4 => prop::sample::select(characters).prop_map(KeyCode::Char),
        1 => any::<char>().prop_map(KeyCode::Char),
        3 => prop::sample::select(keys),
    ]
}

fn modifiers() -> impl Strategy<Value = KeyModifiers> {
    prop::sample::select(vec![
        KeyModifiers::NONE,
        KeyModifiers::NONE,
        KeyModifiers::NONE,
        KeyModifiers::NONE,
        KeyModifiers::SHIFT,
        KeyModifiers::CONTROL,
        KeyModifiers::ALT,
        KeyModifiers::CONTROL | KeyModifiers::ALT,
    ])
}

fn json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(Value::from),
        any::<f64>().prop_map(|number| Number::from_f64(number).map_or(Value::Null, Value::Number)),
        line().prop_map(Value::String),
    ];
    leaf.prop_recursive(3, 24, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
            prop::collection::btree_map(line(), inner, 0..6)
                .prop_map(|members| Value::Object(members.into_iter().collect())),
        ]
    })
}

fn happening() -> impl Strategy<Value = Happening> {
    prop_oneof![
        24 => (key_code(), modifiers()).prop_map(|(code, held)| Happening::Key(code, held)),
        1 => line().prop_map(Happening::Paste),
        1 => line().prop_map(Happening::Solar),
        2 => json().prop_map(Happening::Answer),
        1 => line().prop_map(Happening::Stderr),
        2 => (0_u64..120_000).prop_map(Happening::Later),
        2 => Just(Happening::Tick),
        2 => (1_u16..=220, 1_u16..=70).prop_map(|(width, height)| Happening::Resize(width, height)),
        1 => Just(Happening::Exited),
        1 => Just(Happening::Reconnected),
    ]
}

// 256 sessions, proptest's default; `PROPTEST_CASES=4096` runs a longer hunt.
proptest! {
    #[test]
    fn nothing_typed_or_received_brings_zenith_down(
        happenings in prop::collection::vec(happening(), 0..96),
    ) {
        let mut script = Script::connected(Options {
            opening: false,
            ..Options::default()
        });
        let (mut width, mut height) = (80, 24);
        for (count, happening) in happenings.into_iter().enumerate() {
            match happening {
                Happening::Key(code, held) => script.key(code, held),
                Happening::Paste(text) => script.feed(Incoming::Paste(text)),
                Happening::Solar(text) => script.line(&text),
                Happening::Answer(data) => {
                    let waiting = script.app.link.tracker.waiting().last().map(|call| call.call);
                    if let Some(id) = waiting {
                        script.line(&success(id, "solar.ping", &data));
                    }
                }
                Happening::Stderr(text) => script.stderr(&text),
                Happening::Later(millis) => script.later(Duration::from_millis(millis)),
                Happening::Tick => script.feed(Incoming::Tick),
                Happening::Resize(columns, rows) => {
                    (width, height) = (columns, rows);
                    script.feed(Incoming::Resize { width, height });
                }
                Happening::Exited => {
                    let generation = script.app.link.generation;
                    let how = "exit code 1".to_owned();
                    script.feed(Incoming::Exited { generation, how });
                }
                Happening::Reconnected => script.handshake(),
            }
            if count % 4 == 3 {
                draw(&script.app, width, height);
            }
        }
        draw(&script.app, width, height);
    }
}
