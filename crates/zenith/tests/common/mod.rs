//! What the integration tests share: a scripted ZENITH, and a screen as text.

#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "each test file uses part of this, and a test reports failure by panicking"
)]

use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use ratatui::style::{Color, Modifier, Style};
use serde_json::{Value, json};
use zenith::app::{App, Effect, Incoming, Options, StartSteps, Started};
use zenith_client::connection::Event as ConnectionEvent;
use zenith_client::locate::{Found, Origin, Prepared};

/// The manifest SOLAR 0.1.0 wrote on the machine the fixtures were taken on.
pub(crate) const MANIFEST: &str =
    include_str!("../../../zenith-client/tests/fixtures/solar-0.1.0-manifest.json");

/// 27 September 2026, 15:47:00 UTC, in seconds since the epoch.
const SECONDS_SINCE_THE_EPOCH: u64 = 1_790_524_020;

/// The wall clock the scripted sessions start at: 27 September 2026, 15:47:00 UTC.
pub(crate) fn wall() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(SECONDS_SINCE_THE_EPOCH)
}

/// The `meta` of a response, as section 7 of SOLAR's contract lays it out.
pub(crate) fn meta(id: &Value, method: &str, duration_us: u64) -> Value {
    json!({
        "request_id": id, "method": method, "api_version": "1.0.0", "solar_version": "0.1.0",
        "protocol": "solar/1", "started_at": "2026-09-27T15:47:00.000000Z",
        "duration_us": duration_us, "os": "windows", "arch": "x86_64"
    })
}

/// A success response.
pub(crate) fn success(id: u64, method: &str, data: &Value) -> String {
    let id = json!(id);
    json!({"jsonrpc": "2.0", "id": id,
           "result": {"data": data, "meta": meta(&id, method, 170), "warnings": []}})
    .to_string()
}

/// A scripted ZENITH: events go in at chosen moments, and what it asks for is kept.
pub(crate) struct Script {
    /// The application.
    pub(crate) app: App,
    /// The moment of the next event.
    pub(crate) now: Instant,
    /// What it asked for.
    pub(crate) effects: Vec<Effect>,
}

impl Script {
    /// A ZENITH that has just started, with the options given.
    pub(crate) fn with(options: Options) -> Self {
        let now = Instant::now();
        let (app, effects) = App::new(options, now, wall());
        Self { app, now, effects }
    }

    /// A ZENITH that has just started, without the opening.
    pub(crate) fn new() -> Self {
        Self::with(Options {
            opening: false,
            ..Options::default()
        })
    }

    /// A ZENITH connected to a SOLAR 0.1.0 whose handshake took two milliseconds.
    pub(crate) fn connected(options: Options) -> Self {
        let mut script = Self::with(options);
        script.handshake();
        script
    }

    /// Answers the handshake as SOLAR 0.1.0 does.
    pub(crate) fn handshake(&mut self) {
        self.started();
        self.later(Duration::from_micros(1_210));
        self.line(&success(
            1,
            "solar.version",
            &json!({"solar_version": "0.1.0", "protocol": "solar/1",
                    "manifest_schema_version": "2.0.0",
                    "build": {"git_commit_short": "c9c566b22e2b", "profile": "release",
                              "rustc_version": "rustc 1.97.1", "target": "x86_64-pc-windows-msvc"}}),
        ));
        self.later(Duration::from_micros(630));
        let manifest: Value = serde_json::from_str(MANIFEST).unwrap();
        self.line(&success(2, "solar.manifest", &manifest));
        self.effects.clear();
    }

    /// SOLAR has started.
    pub(crate) fn started(&mut self) {
        let generation = self.app.link.generation;
        let prepared = Prepared {
            found: Found {
                path: PathBuf::from("solar"),
                origin: Origin::Path,
            },
            runs: PathBuf::from("solar"),
            sha256: None,
        };
        self.feed(Incoming::Started {
            generation,
            result: Ok(Started {
                prepared,
                pid: 4242,
                steps: StartSteps::default(),
            }),
        });
    }

    /// An event, now.
    pub(crate) fn feed(&mut self, incoming: Incoming) {
        let effects = self.app.handle(incoming, self.now);
        self.effects.extend(effects);
    }

    /// Time passes.
    pub(crate) fn later(&mut self, by: Duration) {
        self.now += by;
    }

    /// A line of SOLAR's standard output.
    pub(crate) fn line(&mut self, text: &str) {
        let generation = self.app.link.generation;
        let at = self.now;
        self.feed(Incoming::Connection {
            generation,
            event: ConnectionEvent::Line {
                text: text.to_owned(),
                invalid_utf8: false,
                at,
            },
        });
    }

    /// A line of SOLAR's standard error.
    pub(crate) fn stderr(&mut self, text: &str) {
        let generation = self.app.link.generation;
        let at = self.now;
        self.feed(Incoming::Connection {
            generation,
            event: ConnectionEvent::Stderr {
                text: text.to_owned(),
                cut: 0,
                at,
            },
        });
    }

    /// A key.
    pub(crate) fn key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        self.feed(Incoming::Key(KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }));
    }

    /// Text typed a character at a time.
    pub(crate) fn type_text(&mut self, text: &str) {
        for character in text.chars() {
            self.key(KeyCode::Char(character), KeyModifiers::NONE);
        }
    }

    /// A line typed and run.
    pub(crate) fn run(&mut self, line: &str) {
        self.type_text(line);
        self.key(KeyCode::Enter, KeyModifiers::NONE);
    }

    /// The id of the newest call waiting.
    pub(crate) fn waiting_id(&self) -> u64 {
        self.app
            .link
            .tracker
            .waiting()
            .last()
            .map(|waiting| waiting.call)
            .expect("no call is waiting")
    }

    /// Answers the newest call waiting with `data`, after `micros`.
    pub(crate) fn answer(&mut self, method: &str, data: &Value, micros: u64) {
        let id = self.waiting_id();
        self.later(Duration::from_micros(micros));
        self.line(&success(id, method, data));
    }
}

/// Draws the application at a size and returns the buffer.
pub(crate) fn draw(app: &App, width: u16, height: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| zenith::ui::draw(frame, app)).unwrap();
    terminal.backend().buffer().clone()
}

fn colour(color: Option<Color>) -> String {
    match color {
        None | Some(Color::Reset) => "-".to_owned(),
        Some(Color::Rgb(r, g, b)) => format!("#{r:02X}{g:02X}{b:02X}"),
        Some(Color::Indexed(index)) => format!("@{index}"),
        Some(other) => format!("{other:?}"),
    }
}

fn describe(style: Style) -> String {
    let mut text = format!("{} on {}", colour(style.fg), colour(style.bg));
    for (modifier, name) in [
        (Modifier::BOLD, "bold"),
        (Modifier::DIM, "dim"),
        (Modifier::ITALIC, "italic"),
        (Modifier::UNDERLINED, "underlined"),
        (Modifier::REVERSED, "reversed"),
    ] {
        if style.add_modifier.contains(modifier) {
            text.push(' ');
            text.push_str(name);
        }
    }
    text
}

/// A screen as text: the characters, then every run of cells whose style is not the most
/// common one, with the style written out, so that a snapshot shows the colours too.
pub(crate) fn screen(buffer: &Buffer) -> String {
    let area = buffer.area;
    let mut text = String::new();
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for y in 0..area.height {
        let mut row = String::new();
        let mut skip = 0;
        for x in 0..area.width {
            let cell = &buffer[(x, y)];
            *counts.entry(describe(cell.style())).or_default() += 1;
            if skip > 0 {
                skip -= 1;
                continue;
            }
            let symbol = cell.symbol();
            row.push_str(symbol);
            skip = unicode_width::UnicodeWidthStr::width(symbol).saturating_sub(1);
        }
        let _ = writeln!(text, "{}", row.trim_end());
    }
    let common = counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
        .map(|(style, _)| style)
        .unwrap_or_default();
    let _ = writeln!(text, "--- every cell not listed is {common}");
    for y in 0..area.height {
        let mut x = 0;
        while x < area.width {
            let style = describe(buffer[(x, y)].style());
            let start = x;
            while x < area.width && describe(buffer[(x, y)].style()) == style {
                x += 1;
            }
            if style != common {
                let _ = writeln!(text, "{y:>2}:{start:>3}-{:<3} {style}", x - 1);
            }
        }
    }
    text
}

/// Every character of a screen, without the styles.
pub(crate) fn characters(buffer: &Buffer) -> String {
    screen(buffer)
        .split("--- every cell")
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// One thing a person does in the session the README shows.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Act {
    /// A line typed and run.
    Line(&'static str),
    /// `Tab`, to the next tab.
    NextTab,
    /// The APIs tab's selection moved to this API, from the top.
    Select(&'static str),
    /// A plain key.
    Key(char),
}

/// The session the README shows, recorded against a real SOLAR by
/// `against_solar::record_the_session_the_readme_shows` and replayed by the screens.
pub(crate) const README_SCENARIO: &[Act] = &[
    Act::Line("/list"),
    Act::Line("/ping hello"),
    Act::Line("/call system.info"),
    Act::Line("/describe solar.pign"),
    Act::NextTab,
    Act::Select("solar.ping"),
    Act::Key('1'),
    Act::Key('2'),
];

/// Where the recording lives.
pub(crate) const README_RECORDING: &str = "tests/fixtures/readme-session.json";

/// What a real SOLAR answered to the README's session, in the order of the calls.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub(crate) struct Recording {
    /// When the session was recorded, in seconds since the epoch.
    pub(crate) recorded_at: u64,
    /// The SOLAR that answered.
    pub(crate) solar_version: String,
    /// Each call's response line and its round trip, by call number, the handshake first.
    pub(crate) calls: Vec<RecordedCall>,
    /// SOLAR's standard error over the whole session, as it wrote it.
    pub(crate) stderr: Vec<String>,
}

/// One call of the recording.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub(crate) struct RecordedCall {
    /// The response line, exactly as SOLAR wrote it.
    pub(crate) response: String,
    /// The round trip ZENITH measured, in microseconds.
    pub(crate) round_trip_us: u64,
}

impl Script {
    /// Answers every call that is waiting, in order, from the recording.
    pub(crate) fn answer_from(&mut self, recording: &Recording, next: &mut usize) {
        while self.app.link.tracker.waiting().next().is_some() {
            let Some(call) = recording.calls.get(*next) else {
                panic!("the recording has no answer for call {}", *next + 1);
            };
            *next += 1;
            self.later(Duration::from_micros(call.round_trip_us));
            self.line(&call.response);
        }
    }

    /// Does one act of the README's session.
    pub(crate) fn act(&mut self, act: Act) {
        match act {
            Act::Line(line) => self.run(line),
            Act::NextTab => self.key(KeyCode::Tab, KeyModifiers::NONE),
            Act::Select(name) => {
                self.key(KeyCode::Char('g'), KeyModifiers::NONE);
                let position = self
                    .app
                    .catalogue()
                    .and_then(|catalogue| catalogue.apis.iter().position(|api| api.name == name))
                    .expect("the API is in the manifest");
                for _ in 0..position {
                    self.key(KeyCode::Down, KeyModifiers::NONE);
                }
            }
            Act::Key(character) => self.key(KeyCode::Char(character), KeyModifiers::NONE),
        }
    }

    /// The README's session, replayed from the recording up to `acts` acts, with SOLAR's
    /// standard error, at the moment and with the timings of the recording.
    pub(crate) fn replayed(options: Options, acts: usize) -> Self {
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(README_RECORDING),
        )
        .expect("the README's session is recorded; see README_SCENARIO");
        let recording: Recording = serde_json::from_str(&text).unwrap();
        let now = Instant::now();
        let wall = SystemTime::UNIX_EPOCH + Duration::from_secs(recording.recorded_at);
        let (app, effects) = App::new(options, now, wall);
        let mut script = Self { app, now, effects };
        script.started();
        let mut next = 0;
        script.answer_from(&recording, &mut next);
        for act in README_SCENARIO.iter().take(acts) {
            script.later(Duration::from_millis(900));
            script.act(*act);
            script.answer_from(&recording, &mut next);
        }
        for line in &recording.stderr {
            script.stderr(line);
        }
        script.effects.clear();
        script
    }
}
