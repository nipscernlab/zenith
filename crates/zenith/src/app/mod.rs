//! The application: all of ZENITH's state, and what it does with every event.
//!
//! The application is a state machine with no terminal, no threads and no clock of its
//! own. Everything reaches it as an [`Incoming`] with the moment it happened, and
//! everything it wants done outside itself leaves as an [`Effect`]: start SOLAR, write a
//! line, write a file, quit. The runtime in `runtime.rs` does those; the tests drive the
//! same state machine with a test backend and scripted events, which is what makes every
//! screen and every failure path testable.

pub mod apis;
pub mod history;
pub mod link;
pub mod log;
pub mod report;
pub mod session;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use serde_json::{Map, Value, json};
use zenith_client::calls::{Answer, Refused};
use zenith_client::connection::{Event as ConnectionEvent, Settings};
use zenith_client::envelope::{self, Body, Envelope, Id, Message};
use zenith_client::example;
use zenith_client::handshake;
use zenith_client::json_text;
use zenith_client::locate::Prepared;
use zenith_client::manifest::Catalogue;
use zenith_client::schema::{Failure as SchemaFailure, FailureKind};

use crate::clock;
use crate::commands::{self, Command};
use crate::completion;
use crate::glyphs::{Charset, Glyphs};
use crate::keys::{self, Action, LevelKey, Place};
use crate::theme::{Depth, Theme, ThemeName};

use apis::{ApisTab, ExampleResult, Form};
use history::{CallRecord, History, Origin, Outcome, Summary};
use link::{ExitSeen, Failure, Link, Phase};
use log::{LogEntry, LogTab};
use session::{Band, Entry, Layout, Session, Tone};

/// The four tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    /// The command line and the transcript.
    Session,
    /// The catalogue.
    Apis,
    /// SOLAR's standard error.
    Log,
    /// Every call.
    History,
}

impl Tab {
    /// The tabs, in order.
    pub const ALL: [Self; 4] = [Self::Session, Self::Apis, Self::Log, Self::History];

    /// The name on the tab.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Session => "Session",
            Self::Apis => "APIs",
            Self::Log => "Log",
            Self::History => "History",
        }
    }

    fn position(self) -> usize {
        Self::ALL.iter().position(|tab| *tab == self).unwrap_or(0)
    }

    /// The tab after this one.
    #[must_use]
    pub fn next(self) -> Self {
        Self::ALL[(self.position() + 1) % Self::ALL.len()]
    }

    /// The tab before this one.
    #[must_use]
    pub fn previous(self) -> Self {
        Self::ALL[(self.position() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

/// What is drawn over the tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    /// Every key.
    Help {
        /// How far it is scrolled.
        scroll: usize,
    },
    /// One call's request and response, whole.
    Viewer {
        /// Its number in the History.
        record: u64,
        /// How far it is scrolled.
        scroll: usize,
    },
}

/// The opening: the starfield with the SOLAR mark, while the connection is made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Opening {
    /// Which frame of the twinkle is showing.
    pub frame: u64,
    /// When the next frame is due.
    pub next_frame: Instant,
}

/// The time between two frames of the opening: twelve a second.
pub const OPENING_FRAME: Duration = Duration::from_millis(83);

/// The time between two redraws of the timer of a call in flight.
pub const WAITING_TICK: Duration = Duration::from_millis(250);

/// How ZENITH was started.
#[derive(Debug, Clone)]
pub struct Options {
    /// The theme.
    pub theme: ThemeName,
    /// The colour depth.
    pub depth: Depth,
    /// The characters.
    pub charset: Charset,
    /// `--solar`.
    pub solar: Option<PathBuf>,
    /// The level SOLAR is started at.
    pub solar_log: String,
    /// Whether to draw the opening.
    pub opening: bool,
    /// Variables added to SOLAR's environment; the tests use it for the double.
    pub environment: Vec<(String, String)>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            theme: ThemeName::Night,
            depth: Depth::TrueColor,
            charset: Charset::Unicode,
            solar: None,
            solar_log: "trace".to_owned(),
            opening: true,
            environment: Vec::new(),
        }
    }
}

/// SOLAR has started.
#[derive(Debug, Clone)]
pub struct Started {
    /// The binary that runs.
    pub prepared: Prepared,
    /// Its process id.
    pub pid: u32,
    /// How long each step of the start took, which `ZENITH_TRACE_TIMINGS` records.
    pub steps: StartSteps,
}

/// How long each step of starting SOLAR took.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StartSteps {
    /// Finding the binary.
    pub locate: Duration,
    /// Hashing it and, on Windows, finding or making its copy.
    pub prepare: Duration,
    /// Starting the process and the threads that read and write it.
    pub spawn: Duration,
}

/// Which file was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Written {
    /// The History, by `/export` or `x`.
    History,
    /// A report, by `/report`.
    Report,
}

/// Something that happened.
#[derive(Debug, Clone)]
pub enum Incoming {
    /// A key.
    Key(KeyEvent),
    /// Text pasted into the terminal.
    Paste(String),
    /// The terminal changed size.
    Resize {
        /// Columns.
        width: u16,
        /// Rows.
        height: u16,
    },
    /// The start ZENITH asked for finished, well or not.
    Started {
        /// The connection it was for.
        generation: u64,
        /// What came of it.
        result: Result<Started, Failure>,
    },
    /// Something happened on a connection.
    Connection {
        /// The connection.
        generation: u64,
        /// What.
        event: ConnectionEvent,
    },
    /// SOLAR has exited, or was killed after it would not.
    Exited {
        /// The connection.
        generation: u64,
        /// How it ended, in words.
        how: String,
    },
    /// SOLAR has not exited yet, after closing its output.
    StillRunning {
        /// The connection.
        generation: u64,
    },
    /// A file ZENITH was asked to write is written, or could not be.
    Wrote {
        /// Which.
        what: Written,
        /// Where, and how many calls it holds, or why not.
        result: Result<(PathBuf, usize), String>,
    },
    /// A deadline the application asked for has come.
    Tick,
    /// The process was told to end, by a signal or the console closing.
    Terminate,
}

/// Something the application wants done outside itself.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Find, copy and start SOLAR.
    Start {
        /// The connection it is for.
        generation: u64,
        /// `--solar`, when it was given.
        flag: Option<PathBuf>,
        /// How to start it.
        settings: Settings,
    },
    /// Write one line to SOLAR.
    Write {
        /// The connection.
        generation: u64,
        /// The line, without its newline.
        line: String,
    },
    /// End a connection: close its input, wait a little, kill it.
    Stop {
        /// The connection.
        generation: u64,
    },
    /// Ask whether SOLAR has exited.
    CheckExit {
        /// The connection.
        generation: u64,
    },
    /// Write the History to a file.
    Export {
        /// Where, or the default name in the current directory.
        path: Option<PathBuf>,
    },
    /// Write a report to a file.
    Report {
        /// Where, or the default name in the current directory.
        path: Option<PathBuf>,
        /// The `data` of `system.info`, or `None`.
        system: Option<Value>,
        /// Why there is no `system.info`, when there is none.
        why: Option<String>,
    },
    /// Draw the whole screen again.
    Repaint,
    /// Leave.
    Quit,
}

/// A call that is waiting, and what to do with its answer.
#[derive(Debug, Clone)]
struct Pending {
    record: u64,
    origin: Origin,
    layout: Layout,
}

/// All of ZENITH.
#[derive(Debug)]
pub struct App {
    /// How ZENITH was started.
    pub options: Options,
    /// The theme at its depth.
    pub theme: Theme,
    /// The characters.
    pub glyphs: Glyphs,
    /// The tab showing.
    pub tab: Tab,
    /// What is drawn over it.
    pub overlay: Option<Overlay>,
    /// The opening, while it is showing.
    pub opening: Option<Opening>,
    /// The connection.
    pub link: Link,
    /// The Session tab.
    pub session: Session,
    /// The APIs tab.
    pub apis: ApisTab,
    /// The Log tab.
    pub log: LogTab,
    /// The History tab.
    pub history: History,
    /// A message on the status bar until the next key.
    pub flash: Option<(Tone, String)>,
    /// Whether the next `Ctrl+C` quits.
    pub interrupt_armed: bool,
    /// The moment of the event being handled.
    pub now: Instant,
    /// The size of the terminal.
    pub size: (u16, u16),
    /// Whether ZENITH is leaving.
    pub quitting: bool,
    anchor: (Instant, SystemTime),
    pending: HashMap<u64, Pending>,
    effects: Vec<Effect>,
    next_waiting_tick: Option<Instant>,
}

impl App {
    /// ZENITH as it starts, and the effects that start SOLAR.
    #[must_use]
    pub fn new(options: Options, now: Instant, wall: SystemTime) -> (Self, Vec<Effect>) {
        let theme = Theme::new(options.theme, options.depth);
        let glyphs = Glyphs::of(options.charset);
        let opening = options.opening.then(|| Opening {
            frame: 0,
            next_frame: now + OPENING_FRAME,
        });
        let log = LogTab::new(&options.solar_log);
        let mut app = Self {
            theme,
            glyphs,
            tab: Tab::Session,
            overlay: None,
            opening,
            link: Link::starting(1, now, None),
            session: Session::default(),
            apis: ApisTab::default(),
            log,
            history: History::default(),
            flash: None,
            interrupt_armed: false,
            now,
            size: (80, 24),
            quitting: false,
            anchor: (now, wall),
            pending: HashMap::new(),
            effects: Vec::new(),
            next_waiting_tick: None,
            options,
        };
        app.start_connection();
        let effects = std::mem::take(&mut app.effects);
        (app, effects)
    }

    /// The wall clock time of an instant, from the moment ZENITH started.
    #[must_use]
    pub fn wall(&self, at: Instant) -> SystemTime {
        match at.checked_duration_since(self.anchor.0) {
            Some(after) => self.anchor.1 + after,
            None => self.anchor.1 - self.anchor.0.saturating_duration_since(at),
        }
    }

    /// The catalogue, when there is one.
    #[must_use]
    pub fn catalogue(&self) -> Option<&Catalogue> {
        self.link.catalogue.as_ref()
    }

    /// Handles one event and returns what must be done outside.
    pub fn handle(&mut self, incoming: Incoming, now: Instant) -> Vec<Effect> {
        self.now = now.max(self.now);
        match incoming {
            Incoming::Key(key) => self.on_key(&key),
            Incoming::Paste(text) => self.on_paste(&text),
            Incoming::Resize { width, height } => self.size = (width, height),
            Incoming::Started { generation, result } => self.on_started(generation, result),
            Incoming::Connection { generation, event } => {
                if generation == self.link.generation {
                    self.on_connection(event);
                }
            }
            Incoming::Exited { generation, how } => self.on_exited(generation, how),
            Incoming::StillRunning { generation } => self.on_still_running(generation),
            Incoming::Wrote { what, result } => self.on_wrote(what, result),
            Incoming::Tick => self.on_tick(),
            Incoming::Terminate => self.quit(),
        }
        if self.link.tracker.waiting().next().is_some() {
            self.next_waiting_tick = Some(self.now + WAITING_TICK);
        } else {
            self.next_waiting_tick = None;
        }
        std::mem::take(&mut self.effects)
    }

    /// The next moment something on screen changes by itself, when there is one. An
    /// idle ZENITH connected for more than a minute has one a minute away.
    #[must_use]
    pub fn next_deadline(&self) -> Option<Instant> {
        let mut deadlines: Vec<Instant> = Vec::new();
        if let Some(opening) = &self.opening
            && self.link.phase != Phase::Down
        {
            deadlines.push(opening.next_frame);
        }
        if self.link.phase == Phase::Handshaking {
            deadlines.extend(self.link.handshake_deadline);
        }
        deadlines.extend(self.next_waiting_tick);
        deadlines.extend(self.link.next_exit_check);
        deadlines.extend(self.link.exit_seen.as_ref().map(|seen| seen.until));
        if let Some(connected) = self.link.connected_at
            && self.link.connected()
        {
            let lasted = self.now.saturating_duration_since(connected);
            deadlines.push(self.now + clock::until_lasted_changes(lasted));
        }
        deadlines.into_iter().min()
    }

    // The connection.

    fn start_connection(&mut self) {
        self.effects.push(Effect::Start {
            generation: self.link.generation,
            flag: self.options.solar.clone(),
            settings: Settings {
                log_level: self.options.solar_log.clone(),
                environment: self.options.environment.clone(),
            },
        });
    }

    /// Restarts SOLAR: stops this connection, closes what was waiting on it, and starts a
    /// new one, which finds and copies the binary again.
    pub fn reconnect(&mut self) {
        let old = self.link.generation;
        self.close_waiting("the connection was replaced before SOLAR answered");
        self.effects.push(Effect::Stop { generation: old });
        let catalogue = self.link.catalogue.take();
        self.link = Link::starting(old + 1, self.now, catalogue);
        self.push_notice(Tone::Info, vec!["Restarting SOLAR.".to_owned()]);
        self.start_connection();
    }

    fn quit(&mut self) {
        self.quitting = true;
        self.effects.push(Effect::Stop {
            generation: self.link.generation,
        });
        self.effects.push(Effect::Quit);
    }

    fn fail(&mut self, failure: Failure) {
        if self.link.phase == Phase::Down {
            return;
        }
        let was_connected = self.link.connected();
        self.link.phase = Phase::Down;
        self.link.handshake_deadline = None;
        let mut lines = vec![failure.what(), failure.advice()];
        lines.extend(failure.last_words().iter().map(|line| format!("  {line}")));
        self.link.failure = Some(failure);
        self.close_waiting("the connection ended before SOLAR answered");
        let _ = was_connected;
        self.push_notice(Tone::Error, lines);
    }

    fn close_waiting(&mut self, why: &str) {
        for waiting in self.link.tracker.close_all() {
            if let Some(pending) = self.pending.remove(&waiting.call) {
                if let Some(record) = self.history.calls.get_mut(pending.record) {
                    record.closed = Some(why.to_owned());
                }
                self.finish_example(
                    &pending.origin,
                    ExampleResult::Closed {
                        why: why.to_owned(),
                    },
                );
                if let Origin::Report { path } = pending.origin {
                    self.effects.push(Effect::Report {
                        path: Some(path),
                        system: None,
                        why: Some(why.to_owned()),
                    });
                }
            }
        }
        self.pending.clear();
    }

    fn on_started(&mut self, generation: u64, result: Result<Started, Failure>) {
        if generation != self.link.generation {
            return;
        }
        match result {
            Err(failure) => self.fail(failure),
            Ok(started) => {
                self.link.binary = Some(started.prepared);
                self.link.pid = Some(started.pid);
                self.link.phase = Phase::Handshaking;
                self.link.handshake_deadline = Some(self.now + link::HANDSHAKE_TIMEOUT);
                self.link.version_call = self
                    .send_call(
                        "solar.version",
                        &json!({}),
                        Origin::Handshake,
                        Layout::Version,
                        false,
                    )
                    .ok();
                self.link.manifest_call = self
                    .send_call(
                        "solar.manifest",
                        &json!({}),
                        Origin::Handshake,
                        Layout::List,
                        false,
                    )
                    .ok();
            }
        }
    }

    fn on_connection(&mut self, event: ConnectionEvent) {
        match event {
            ConnectionEvent::Line {
                text,
                invalid_utf8,
                at,
            } => self.on_line(text, invalid_utf8, at),
            ConnectionEvent::LineTooLong { bytes, id, at } => self.on_too_long(bytes, id, at),
            ConnectionEvent::Stderr { text, cut, at } => {
                self.link.remember(&text);
                let wall = self.wall(at);
                self.log
                    .entries
                    .push(LogEntry::read(&text, cut, self.link.generation, wall));
            }
            ConnectionEvent::StderrClosed => {
                self.link.stderr_closed = true;
                if let Some(seen) = self.link.exit_seen.take() {
                    self.report_exit(seen.how, seen.at);
                }
            }
            ConnectionEvent::OutputClosed { error, .. } => {
                self.link.output_closed = true;
                self.link.next_exit_check = Some(self.now);
                self.link.exit_checks = 0;
                self.effects.push(Effect::CheckExit {
                    generation: self.link.generation,
                });
                if error.is_some() {
                    self.fail(Failure::OutputClosed { error });
                }
            }
            ConnectionEvent::WriteFailed { error } => self.fail(Failure::InputClosed { error }),
        }
    }

    fn on_exited(&mut self, generation: u64, how: String) {
        if generation != self.link.generation {
            return;
        }
        self.link.next_exit_check = None;
        // The thread that reads SOLAR's standard error may not have delivered its last
        // lines yet, and they are what says why SOLAR ended. The exit is reported once that
        // pipe is read to its end, or after a moment if something else holds it open.
        if !self.link.stderr_closed {
            self.link.exit_seen = Some(ExitSeen {
                how,
                at: self.now,
                until: self.now + link::LAST_WORDS_GRACE,
            });
            return;
        }
        self.report_exit(how, self.now);
    }

    fn report_exit(&mut self, how: String, at: Instant) {
        let after = match self.link.phase {
            Phase::Connected => self
                .link
                .connected_at
                .map(|connected| at.saturating_duration_since(connected)),
            _ => None,
        };
        let waiting = self.link.tracker.waiting().len();
        let last_words = self.link.last_words.iter().cloned().collect();
        if self.link.phase == Phase::Down {
            // Already down for another reason; the exit is still worth knowing.
            self.link.failure = Some(Failure::Exited {
                how,
                after,
                waiting,
                last_words,
            });
            return;
        }
        self.fail(Failure::Exited {
            how,
            after,
            waiting,
            last_words,
        });
    }

    fn on_still_running(&mut self, generation: u64) {
        if generation != self.link.generation {
            return;
        }
        self.link.exit_checks += 1;
        self.link.next_exit_check = Some(self.now + link::EXIT_CHECK_INTERVAL);
    }

    fn on_tick(&mut self) {
        if let Some(seen) = self.link.exit_seen.take() {
            if self.now >= seen.until {
                self.report_exit(seen.how, seen.at);
            } else {
                self.link.exit_seen = Some(seen);
            }
        }
        if let Some(opening) = &mut self.opening
            && self.now >= opening.next_frame
        {
            opening.frame += 1;
            opening.next_frame = self.now + OPENING_FRAME;
        }
        if self.link.phase == Phase::Handshaking
            && self
                .link
                .handshake_deadline
                .is_some_and(|deadline| self.now >= deadline)
        {
            self.fail(Failure::Timeout);
        }
        if let Some(check) = self.link.next_exit_check
            && self.now >= check
        {
            self.link.next_exit_check = None;
            if self.link.exit_checks >= link::EXIT_CHECKS {
                self.effects.push(Effect::Stop {
                    generation: self.link.generation,
                });
            } else {
                self.effects.push(Effect::CheckExit {
                    generation: self.link.generation,
                });
            }
        }
    }

    // Calls.

    /// Sends a call ZENITH builds, records it, and returns its number in the History.
    fn send_call(
        &mut self,
        method: &str,
        params: &Value,
        origin: Origin,
        layout: Layout,
        show: bool,
    ) -> Result<u64, String> {
        let handshake = origin == Origin::Handshake;
        if !(self.link.connected() || (handshake && self.link.phase == Phase::Handshaking)) {
            return Err(
                "SOLAR is not connected, so nothing was sent. Ctrl+R starts it.".to_owned(),
            );
        }
        let (call, line) = self
            .link
            .tracker
            .call(method, params, self.now)
            .map_err(|refused| refused.to_string())?;
        let record = self.record(call, Some(method.to_owned()), origin.clone(), line.clone());
        self.pending.insert(
            call,
            Pending {
                record,
                origin,
                layout,
            },
        );
        self.effects.push(Effect::Write {
            generation: self.link.generation,
            line,
        });
        if show {
            self.session.push(Entry::Call { record, layout });
        }
        Ok(record)
    }

    fn send_raw(&mut self, line: &str) -> Result<u64, String> {
        if !self.link.connected() {
            return Err(
                "SOLAR is not connected, so nothing was sent. Ctrl+R starts it.".to_owned(),
            );
        }
        let call = self
            .link
            .tracker
            .raw(line, self.now)
            .map_err(|refused: Refused| refused.to_string())?;
        let method = self
            .link
            .tracker
            .find(call)
            .and_then(|waiting| waiting.method.clone());
        let record = self.record(call, method, Origin::Raw, line.to_owned());
        self.pending.insert(
            call,
            Pending {
                record,
                origin: Origin::Raw,
                layout: Layout::Generic,
            },
        );
        self.effects.push(Effect::Write {
            generation: self.link.generation,
            line: line.to_owned(),
        });
        self.session.push(Entry::Call {
            record,
            layout: Layout::Generic,
        });
        Ok(record)
    }

    fn record(
        &mut self,
        call: u64,
        method: Option<String>,
        origin: Origin,
        request: String,
    ) -> u64 {
        self.history.calls.push(CallRecord {
            connection: self.link.generation,
            call,
            method,
            origin,
            request,
            sent: self.now,
            sent_wall: self.wall(self.now),
            outcome: None,
            closed: None,
        })
    }

    fn on_line(&mut self, text: String, invalid_utf8: bool, at: Instant) {
        let message = envelope::read(&text);
        let handshaking = self.link.phase == Phase::Handshaking;
        let answer = self.link.tracker.answer(&message);
        let Answer::Call { waiting, by_order } = answer else {
            if handshaking && matches!(message, Message::NotJson { .. }) {
                let quoted: String = text.chars().take(80).collect();
                self.fail(Failure::NotContract {
                    what: format!("{quoted:?}."),
                });
            }
            self.session.push(Entry::Unexpected { line: text });
            return;
        };
        let Some(pending) = self.pending.remove(&waiting.call) else {
            return;
        };
        let round_trip = at.saturating_duration_since(waiting.sent);
        self.link.last_round_trip = Some(round_trip);
        let (duration_us, mut violations) = match &message {
            Message::Single(envelope) => (
                envelope.meta.as_ref().and_then(|meta| meta.duration_us),
                envelope.violations.len(),
            ),
            Message::Batch(items) => (None, items.iter().map(|item| item.violations.len()).sum()),
            Message::NotJson { .. } => (None, 1),
        };
        if invalid_utf8 {
            violations += 1;
        }
        let outcome = Outcome {
            summary: Summary::of(&message),
            line: Some(text),
            received: at,
            received_wall: self.wall(at),
            duration_us,
            violations,
            by_order,
        };
        if let Some(record) = self.history.calls.get_mut(pending.record) {
            let before = ring_bytes(record);
            record.outcome = Some(outcome);
            self.history.calls.resized(pending.record, before);
        }
        self.after_answer(&pending, &message, round_trip);
    }

    fn on_too_long(&mut self, bytes: usize, id: Option<Id>, at: Instant) {
        let probe = Message::Single(Envelope {
            id: id.unwrap_or(Id::Null),
            body: Body::Unrecognised,
            meta: None,
            violations: Vec::new(),
        });
        let Answer::Call { waiting, by_order } = self.link.tracker.answer(&probe) else {
            self.push_notice(
                Tone::Error,
                vec![format!(
                    "SOLAR wrote a line of {bytes} bytes, longer than the 16 MiB ZENITH keeps, and \
                     no call was waiting for it."
                )],
            );
            return;
        };
        let Some(pending) = self.pending.remove(&waiting.call) else {
            return;
        };
        let round_trip = at.saturating_duration_since(waiting.sent);
        self.link.last_round_trip = Some(round_trip);
        let received_wall = self.wall(at);
        if let Some(record) = self.history.calls.get_mut(pending.record) {
            record.outcome = Some(Outcome {
                line: None,
                received: at,
                received_wall,
                summary: Summary::TooLong { bytes },
                duration_us: None,
                violations: 0,
                by_order,
            });
        }
        self.finish_example(
            &pending.origin,
            ExampleResult::Closed {
                why: format!("the response was {bytes} bytes, longer than ZENITH keeps"),
            },
        );
    }

    fn after_answer(&mut self, pending: &Pending, message: &Message, round_trip: Duration) {
        let data = match message {
            Message::Single(Envelope {
                body: Body::Success { data, .. },
                ..
            }) => Some(data),
            _ => None,
        };
        match &pending.origin {
            Origin::Handshake => self.handshake_answer(pending, message, data),
            Origin::Example { api, index } => {
                let result = self.judge_example(api, *index, message, round_trip);
                self.finish_example(&pending.origin, result);
            }
            Origin::Report { path } => {
                let (system, why) = match data {
                    Some(data) => (Some(data.clone()), None),
                    None => (None, Some("system.info did not succeed".to_owned())),
                };
                self.effects.push(Effect::Report {
                    path: Some(path.clone()),
                    system,
                    why,
                });
            }
            Origin::Reload => {
                if let Some(data) = data {
                    self.adopt_manifest(data, true);
                }
            }
            Origin::Command | Origin::Form | Origin::Raw | Origin::Cancel | Origin::Again => {
                if pending.layout == Layout::List
                    && let Some(data) = data
                {
                    self.adopt_manifest(data, false);
                }
            }
        }
    }

    fn handshake_answer(&mut self, pending: &Pending, message: &Message, data: Option<&Value>) {
        if self.link.phase != Phase::Handshaking {
            return;
        }
        let record = self.history.calls.get(pending.record);
        let method = record
            .and_then(|record| record.method.clone())
            .unwrap_or_default();
        let Some(data) = data else {
            let text = match message {
                Message::Single(envelope) => match &envelope.body {
                    Body::Failure(failure) => failure.message.clone().unwrap_or_default(),
                    _ => "a response that is neither a result nor an error".to_owned(),
                },
                Message::Batch(_) => "a batch".to_owned(),
                Message::NotJson { error } => format!("a line that is not JSON ({error})"),
            };
            self.fail(Failure::Refused {
                method,
                message: text,
            });
            return;
        };
        if method == "solar.version" {
            match handshake::check_version(data) {
                Ok(info) => self.link.info = Some(info),
                Err(error) => {
                    self.fail(Failure::Handshake(error));
                    return;
                }
            }
        } else {
            match Catalogue::from_data(data) {
                Ok(catalogue) => {
                    self.link.capabilities = catalogue.capabilities();
                    self.link.catalogue = Some(catalogue);
                    self.link.manifest_call = None;
                }
                Err(error) => {
                    self.fail(Failure::Manifest(error));
                    return;
                }
            }
        }
        let version_done = self.link.info.is_some();
        let manifest_done = self.link.manifest_call.is_none() && self.link.catalogue.is_some();
        if version_done && manifest_done {
            self.connected();
        }
    }

    fn connected(&mut self) {
        self.link.phase = Phase::Connected;
        self.link.connected_at = Some(self.now);
        self.link.handshake_deadline = None;
        self.opening = None;
        let took = self.now.saturating_duration_since(self.link.requested);
        let count = self.catalogue().map_or(0, |catalogue| catalogue.apis.len());
        let info = self.link.info.clone();
        if let Some(info) = info {
            let cancel = if self.link.capabilities.cancel {
                "Ctrl+C cancels a call in flight."
            } else {
                "This SOLAR has no solar.cancel."
            };
            self.push_notice(
                Tone::Info,
                vec![
                    format!(
                        "Connected to SOLAR {} in {}: protocol {}, manifest {}, {count} {}.",
                        info.solar_version,
                        clock::latency(took),
                        info.protocol,
                        info.manifest_schema_version,
                        if count == 1 { "API" } else { "APIs" },
                    ),
                    format!("{cancel} Type / for the commands, or ? for every key."),
                ],
            );
        }
    }

    fn adopt_manifest(&mut self, data: &Value, say: bool) {
        match Catalogue::from_data(data) {
            Ok(catalogue) => {
                let count = catalogue.apis.len();
                self.link.capabilities = catalogue.capabilities();
                self.link.catalogue = Some(catalogue);
                if say {
                    self.flash = Some((
                        Tone::Info,
                        format!("The manifest was read again: {count} APIs."),
                    ));
                }
            }
            Err(error) => {
                self.flash = Some((Tone::Error, error.to_string()));
            }
        }
    }

    fn judge_example(
        &self,
        api: &str,
        index: usize,
        message: &Message,
        round_trip: Duration,
    ) -> ExampleResult {
        let Some(example) = self
            .catalogue()
            .and_then(|catalogue| catalogue.api(api))
            .and_then(|api| api.examples.get(index))
        else {
            return ExampleResult::Closed {
                why: "the example is no longer in the manifest".to_owned(),
            };
        };
        match message {
            Message::Single(envelope) => match &envelope.body {
                Body::Success { data, .. } => {
                    match example::compare(&example.response, data, &example.matching) {
                        Ok(()) => ExampleResult::Matches { round_trip },
                        Err(difference) => ExampleResult::Differs {
                            sentence: difference.sentence(),
                        },
                    }
                }
                Body::Failure(failure) => ExampleResult::Failed {
                    status: failure.status.clone().unwrap_or_default(),
                    reason: failure.reason.clone().unwrap_or_default(),
                },
                Body::Unrecognised => ExampleResult::Differs {
                    sentence: "the response is neither a result nor an error".to_owned(),
                },
            },
            _ => ExampleResult::Differs {
                sentence: "the response is not a single response".to_owned(),
            },
        }
    }

    fn finish_example(&mut self, origin: &Origin, result: ExampleResult) {
        if let Origin::Example { api, index } = origin {
            self.apis.results.insert((api.clone(), *index), result);
        }
    }

    fn push_notice(&mut self, tone: Tone, lines: Vec<String>) {
        self.session.push(Entry::Notice { tone, lines });
    }

    fn on_wrote(&mut self, what: Written, result: Result<(PathBuf, usize), String>) {
        let noun = match what {
            Written::History => "the History",
            Written::Report => "the report",
        };
        let (tone, text) = match result {
            Ok((path, calls)) => (
                Tone::Info,
                format!(
                    "Wrote {noun}, {calls} {}, to {}.",
                    if calls == 1 { "call" } else { "calls" },
                    path.display()
                ),
            ),
            Err(error) => (Tone::Error, format!("Could not write {noun}: {error}")),
        };
        self.flash = Some((tone, text.clone()));
        self.push_notice(tone, vec![text]);
    }

    // Keys.

    fn places(&self) -> Vec<Place> {
        match self.overlay {
            Some(Overlay::Help { .. }) => return vec![Place::Help, Place::List, Place::Everywhere],
            Some(Overlay::Viewer { .. }) => {
                return vec![Place::Viewer, Place::List, Place::Everywhere];
            }
            None => {}
        }
        if self.opening.is_some() && self.link.phase == Phase::Down {
            return vec![Place::Failure, Place::Everywhere];
        }
        match self.tab {
            Tab::Session => vec![Place::CommandLine, Place::Everywhere],
            Tab::Apis if self.apis.form.is_some() => vec![Place::Form, Place::Everywhere],
            Tab::Apis => vec![Place::Apis, Place::List, Place::Everywhere],
            Tab::Log => vec![Place::Log, Place::List, Place::Everywhere],
            Tab::History => vec![Place::History, Place::List, Place::Everywhere],
        }
    }

    /// Whether printable keys go into a text field rather than to the key table.
    fn typing(&self) -> bool {
        if self.overlay.is_some() {
            return false;
        }
        match self.tab {
            Tab::Session => true,
            Tab::Apis => {
                self.apis.filtering
                    || self
                        .apis
                        .form
                        .as_ref()
                        .and_then(|form| form.fields.get(form.focus))
                        .is_some_and(apis::Field::takes_text)
            }
            Tab::Log | Tab::History => false,
        }
    }

    fn on_key(&mut self, key: &KeyEvent) {
        if key.kind == ratatui::crossterm::event::KeyEventKind::Release {
            return;
        }
        let action = self.action_for(key);
        if action != Some(Action::Interrupt) {
            self.interrupt_armed = false;
        }
        if action.is_some() || is_text(key) {
            self.flash = None;
        }
        // The opening gives way to any key while the connection is still being made.
        if self.opening.is_some() && self.link.phase != Phase::Down {
            self.opening = None;
            if action == Some(Action::Interrupt) {
                self.interrupt();
            }
            return;
        }
        match action {
            Some(action) => self.act(action),
            None if is_text(key) && self.typing() => {
                if let KeyCode::Char(character) = key.code {
                    self.type_text(&character.to_string());
                }
            }
            None => {}
        }
    }

    fn action_for(&self, key: &KeyEvent) -> Option<Action> {
        let places = self.places();
        let line_empty = self.session.editor.is_empty();
        if self.typing() && is_text(key) {
            // A printable key is text, except where the command line gives it a meaning of
            // its own, such as ? on an empty line.
            if self.tab == Tab::Session && self.overlay.is_none() {
                return keys::lookup(&[Place::CommandLine], key, line_empty);
            }
            return None;
        }
        if self.tab == Tab::Apis && self.apis.filtering && self.overlay.is_none() {
            return match key.code {
                KeyCode::Enter | KeyCode::Esc => Some(Action::Close),
                KeyCode::Backspace => Some(Action::Backspace),
                _ => keys::lookup(&[Place::Everywhere], key, line_empty)
                    .filter(|action| matches!(action, Action::Interrupt | Action::Repaint)),
            };
        }
        keys::lookup(&places, key, line_empty)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one match over every action reads as the table it implements"
    )]
    fn act(&mut self, action: Action) {
        if let Some(overlay) = self.overlay
            && self.act_on_overlay(overlay, action)
        {
            return;
        }
        match action {
            Action::Help => self.overlay = Some(Overlay::Help { scroll: 0 }),
            Action::NextTab => self.go_to(self.tab.next()),
            Action::PreviousTab => self.go_to(self.tab.previous()),
            Action::GoToTab(number) => {
                if let Some(tab) = Tab::ALL.get(usize::from(number.saturating_sub(1))) {
                    self.go_to(*tab);
                }
            }
            Action::Slash => {
                self.go_to(Tab::Session);
                if self.session.editor.is_empty() {
                    self.type_text("/");
                }
            }
            Action::Envelope => {
                if let Some(record) = self.history.calls.last().map(|(number, _)| number) {
                    self.overlay = Some(Overlay::Viewer { record, scroll: 0 });
                }
            }
            Action::Reconnect => {
                if let Some(opening) = &mut self.opening {
                    opening.next_frame = self.now + OPENING_FRAME;
                }
                self.reconnect();
            }
            Action::Repaint => self.effects.push(Effect::Repaint),
            Action::Interrupt => self.interrupt(),
            Action::Close => self.close(),
            Action::Quit => self.quit(),
            Action::Continue => self.opening = None,
            _ => match self.tab {
                Tab::Session => self.act_on_command_line(action),
                Tab::Apis => self.act_on_apis(action),
                Tab::Log => self.act_on_log(action),
                Tab::History => self.act_on_history(action),
            },
        }
    }

    fn go_to(&mut self, tab: Tab) {
        self.tab = tab;
        self.overlay = None;
    }

    fn close(&mut self) {
        if self.overlay.take().is_some() {
            return;
        }
        match self.tab {
            Tab::Session => {
                if self.session.menu_open() {
                    self.session.menu_dismissed = true;
                } else {
                    self.session.band = None;
                }
            }
            Tab::Apis => {
                if self.apis.filtering {
                    self.apis.filtering = false;
                } else if self.apis.form.is_some() {
                    self.apis.form = None;
                } else {
                    self.apis.filter.clear();
                }
            }
            Tab::Log => self.log.expanded = false,
            Tab::History => {}
        }
    }

    fn act_on_overlay(&mut self, overlay: Overlay, action: Action) -> bool {
        let page = usize::from(self.size.1.saturating_sub(6)).max(1);
        let scroll_by = |scroll: usize, action: Action| match action {
            Action::Up => Some(scroll.saturating_sub(1)),
            Action::Down => Some(scroll + 1),
            Action::PageUp => Some(scroll.saturating_sub(page)),
            Action::PageDown => Some(scroll + page),
            Action::Top => Some(0),
            Action::Bottom => Some(usize::MAX / 2),
            _ => None,
        };
        match overlay {
            Overlay::Help { scroll } => {
                if let Some(scroll) = scroll_by(scroll, action) {
                    self.overlay = Some(Overlay::Help { scroll });
                    return true;
                }
                if action == Action::Help {
                    self.overlay = None;
                    return true;
                }
            }
            Overlay::Viewer { record, scroll } => {
                if let Some(scroll) = scroll_by(scroll, action) {
                    self.overlay = Some(Overlay::Viewer { record, scroll });
                    return true;
                }
                let first = self.history.calls.first_number();
                let last = self.history.calls.next_number().saturating_sub(1);
                match action {
                    Action::PreviousCall => {
                        let record = record.saturating_sub(1).max(first);
                        self.overlay = Some(Overlay::Viewer { record, scroll: 0 });
                        return true;
                    }
                    Action::NextCall => {
                        let record = (record + 1).min(last);
                        self.overlay = Some(Overlay::Viewer { record, scroll: 0 });
                        return true;
                    }
                    Action::Envelope => {
                        self.overlay = None;
                        return true;
                    }
                    _ => {}
                }
            }
        }
        // Switching tabs from an overlay closes it first, which is what a person expects
        // of a key that leaves; the switch itself happens after.
        if matches!(
            action,
            Action::NextTab | Action::PreviousTab | Action::GoToTab(_) | Action::Slash
        ) {
            self.overlay = None;
        }
        false
    }

    fn interrupt(&mut self) {
        if self.interrupt_armed {
            self.quit();
            return;
        }
        self.interrupt_armed = true;
        let latest = self
            .link
            .tracker
            .waiting()
            .filter(|waiting| {
                waiting.method.as_deref() != Some(zenith_client::manifest::CANCEL_API)
            })
            .last()
            .map(|waiting| (waiting.call, waiting.method.clone()));
        let message = match latest {
            Some((call, method)) if self.link.capabilities.cancel => {
                let sent = self.send_call(
                    zenith_client::manifest::CANCEL_API,
                    &json!({"id": call}),
                    Origin::Cancel,
                    Layout::Generic,
                    true,
                );
                let method = method.unwrap_or_default();
                match sent {
                    Ok(_) => {
                        format!("Cancelling call {call}, {method}. Press Ctrl+C again to quit.")
                    }
                    Err(error) => format!("{error} Press Ctrl+C again to quit."),
                }
            }
            Some((call, method)) => {
                let budget = method
                    .as_deref()
                    .and_then(|name| self.catalogue()?.api(name)?.timeout_ms)
                    .map_or_else(String::new, |budget| {
                        format!(" or reaches its budget of {budget} ms")
                    });
                format!(
                    "This SOLAR offers no solar.cancel, so call {call} runs until it answers{budget}. \
                     Press Ctrl+C again to quit."
                )
            }
            None => {
                if self.tab == Tab::Session && !self.session.editor.is_empty() {
                    self.session.editor.clear();
                    self.refresh_completion();
                }
                "Press Ctrl+C again to quit.".to_owned()
            }
        };
        self.flash = Some((Tone::Info, message));
    }

    fn on_paste(&mut self, text: &str) {
        if self.typing() {
            self.type_text(text);
        } else if self.tab != Tab::Session {
            self.go_to(Tab::Session);
            self.type_text(text);
        }
    }

    fn type_text(&mut self, text: &str) {
        match self.tab {
            Tab::Session => {
                self.session.editor.insert(text);
                self.session.band = None;
                self.refresh_completion();
            }
            Tab::Apis if self.apis.filtering => {
                self.apis.filter.insert(text);
                self.apis.selected = 0;
            }
            Tab::Apis => {
                if let Some(field) = self.apis.form.as_mut().and_then(Form::focused) {
                    field.editor.insert(text);
                }
            }
            Tab::Log | Tab::History => {}
        }
    }

    fn refresh_completion(&mut self) {
        let editor = &self.session.editor;
        self.session.completion =
            completion::complete(editor.text(), editor.cursor(), self.link.catalogue.as_ref());
        self.session.highlighted = 0;
        self.session.menu_dismissed = false;
    }

    fn act_on_command_line(&mut self, action: Action) {
        let page = usize::from(self.size.1.saturating_sub(8)).max(1);
        let menu = self.session.menu_open();
        let editor = &mut self.session.editor;
        let mut edited = true;
        match action {
            Action::Submit => {
                self.submit();
                return;
            }
            Action::Complete => {
                self.accept_candidate();
                return;
            }
            Action::CompleteBack => {
                self.move_highlight(false);
                return;
            }
            Action::Up if menu => {
                self.move_highlight(false);
                return;
            }
            Action::Down if menu => {
                self.move_highlight(true);
                return;
            }
            Action::Up => editor.history_previous(),
            Action::Down => editor.history_next(),
            Action::Left => editor.left(),
            Action::Right => editor.right(),
            Action::Home => editor.home(),
            Action::End => editor.end(),
            Action::WordLeft => editor.word_left(),
            Action::WordRight => editor.word_right(),
            Action::Backspace => editor.backspace(),
            Action::Delete => editor.delete(),
            Action::DeleteWord => editor.delete_word(),
            Action::DeleteToStart => editor.delete_to_start(),
            Action::DeleteToEnd => editor.delete_to_end(),
            Action::PageUp => {
                self.session.scroll += page;
                edited = false;
            }
            Action::PageDown => {
                self.session.scroll = self.session.scroll.saturating_sub(page);
                if self.session.scroll == 0 {
                    self.session.unseen = 0;
                }
                edited = false;
            }
            _ => edited = false,
        }
        if edited {
            self.session.band = None;
            self.refresh_completion();
        }
    }

    fn move_highlight(&mut self, forward: bool) {
        let Some(completion) = &self.session.completion else {
            return;
        };
        let count = completion.candidates.len();
        if count == 0 {
            return;
        }
        self.session.menu_dismissed = false;
        self.session.highlighted = if forward {
            (self.session.highlighted + 1) % count
        } else {
            (self.session.highlighted + count - 1) % count
        };
    }

    fn accept_candidate(&mut self) {
        let Some(completion) = self.session.completion.clone() else {
            return;
        };
        let Some(candidate) = completion.candidates.get(self.session.highlighted) else {
            return;
        };
        self.session.editor.replace(
            completion.replace.start,
            completion.replace.end,
            &candidate.insert,
        );
        self.refresh_completion();
    }

    fn submit(&mut self) {
        let line = self.session.editor.text().to_owned();
        let names: Vec<String> = self
            .catalogue()
            .map(|catalogue| catalogue.names().map(str::to_owned).collect())
            .unwrap_or_default();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        match commands::parse(&line, &names) {
            Ok(None) => {}
            Err(error) => {
                self.session.band = Some(Band {
                    tone: Tone::Error,
                    lines: vec![error.message],
                    underline: error.span,
                });
            }
            Ok(Some(command)) => match self.run(command, &line) {
                Ok(()) => {
                    self.session.editor.take();
                    self.session.band = None;
                    self.session.scroll = 0;
                    self.session.unseen = 0;
                    self.refresh_completion();
                }
                Err(band) => self.session.band = Some(band),
            },
        }
    }

    fn error_band(message: String) -> Band {
        Band {
            tone: Tone::Error,
            lines: vec![message],
            underline: None,
        }
    }

    fn run(&mut self, command: Command, line: &str) -> Result<(), Band> {
        let echo = Entry::Command {
            text: line.trim().to_owned(),
        };
        match command {
            Command::List => self.command_call(echo, "solar.manifest", &json!({}), Layout::List),
            Command::Describe { api } => self.command_call(
                echo,
                "solar.describe",
                &json!({ "api": api }),
                Layout::Describe,
            ),
            Command::Call {
                api,
                json,
                json_start,
            } => {
                let params = self.check_params(&api, &json, json_start)?;
                self.command_call(echo, &api, &params, Layout::Generic)
            }
            Command::Ping { message } => {
                let params =
                    message.map_or_else(|| json!({}), |message| json!({ "message": message }));
                self.command_call(echo, "solar.ping", &params, Layout::Ping)
            }
            Command::Version => {
                self.command_call(echo, "solar.version", &json!({}), Layout::Version)
            }
            Command::Theme { name } => {
                let name = name.unwrap_or_else(|| self.theme.name.next());
                self.theme = Theme::new(name, self.theme.depth);
                self.session.push(echo);
                self.push_notice(
                    Tone::Info,
                    vec![format!("The theme is now {}.", name.name())],
                );
                Ok(())
            }
            Command::Help { command } => {
                self.session.push(echo);
                self.session.push(Entry::Help {
                    topic: command.map(|spec| spec.name),
                });
                Ok(())
            }
            Command::Quit => {
                self.quit();
                Ok(())
            }
            Command::Raw { line: raw } => {
                if !self.link.connected() {
                    return Err(Self::error_band(
                        "SOLAR is not connected, so nothing was sent. Ctrl+R starts it.".to_owned(),
                    ));
                }
                self.session.push(echo);
                self.send_raw(&raw).map(drop).map_err(Self::error_band)
            }
            Command::Reconnect => {
                self.session.push(echo);
                self.reconnect();
                Ok(())
            }
            Command::Clear => {
                self.session.transcript.clear();
                self.session.scroll = 0;
                self.session.unseen = 0;
                Ok(())
            }
            Command::Export { path } => {
                self.session.push(echo);
                self.effects.push(Effect::Export {
                    path: path.map(PathBuf::from),
                });
                Ok(())
            }
            Command::Report { path } => {
                self.session.push(echo);
                self.report(path.map(PathBuf::from));
                Ok(())
            }
        }
    }

    fn report(&mut self, path: Option<PathBuf>) {
        let path = path.unwrap_or_else(|| {
            PathBuf::from(format!(
                "zenith-report-{}.ndjson",
                clock::Utc::of(self.wall(self.now)).file_stamp()
            ))
        });
        if self.link.connected() {
            let sent = self.send_call(
                "system.info",
                &json!({}),
                Origin::Report { path: path.clone() },
                Layout::Generic,
                false,
            );
            if let Err(why) = sent {
                self.effects.push(Effect::Report {
                    path: Some(path),
                    system: None,
                    why: Some(why),
                });
            }
        } else {
            self.effects.push(Effect::Report {
                path: Some(path),
                system: None,
                why: Some("SOLAR was not connected".to_owned()),
            });
        }
    }

    fn command_call(
        &mut self,
        echo: Entry,
        method: &str,
        params: &Value,
        layout: Layout,
    ) -> Result<(), Band> {
        if !self.link.connected() {
            return Err(Self::error_band(
                "SOLAR is not connected, so nothing was sent. Ctrl+R starts it.".to_owned(),
            ));
        }
        self.session.push(echo);
        self.send_call(method, params, Origin::Command, layout, true)
            .map(drop)
            .map_err(Self::error_band)
    }

    /// Checks the JSON of a `/call` before it is sent: that it parses, that it is an
    /// object, and that it fits the API's schema, with the part of the line to underline.
    fn check_params(&self, api: &str, json: &str, json_start: usize) -> Result<Value, Band> {
        if json.trim().is_empty() {
            return Ok(Value::Object(Map::new()));
        }
        let value: Value = serde_json::from_str(json).map_err(|error| {
            let offset = json_text::offset_of(json, error.line(), error.column());
            let at = json_start + offset.min(json.len().saturating_sub(1));
            let message = error.to_string();
            let message = message
                .rsplit_once(" at line ")
                .map_or(message.as_str(), |(before, _)| before)
                .to_owned();
            Band {
                tone: Tone::Error,
                lines: vec![format!("The parameters are not JSON: {message}.")],
                underline: Some(at..at + 1),
            }
        })?;
        if !value.is_object() {
            return Err(Band {
                tone: Tone::Error,
                lines: vec![
                    "The parameters must be a JSON object, as section 3 of SOLAR's contract \
                     requires."
                        .to_owned(),
                ],
                underline: Some(json_start..json_start + json.trim_end().len()),
            });
        }
        let Some(api) = self.catalogue().and_then(|catalogue| catalogue.api(api)) else {
            // Not in the manifest: SOLAR's own answer, with its suggestions, is what the
            // person needs to see.
            return Ok(value);
        };
        let report = api.params().validate(&value);
        if report.is_valid() {
            return Ok(value);
        }
        let tree = json_text::spans(json);
        let underline = tree
            .as_ref()
            .and_then(|tree| failure_span(tree, &report.failures[0]))
            .map(|span| json_start + span.start..json_start + span.end);
        let mut lines: Vec<String> = report
            .failures
            .iter()
            .take(3)
            .map(SchemaFailure::sentence)
            .collect();
        if report.failures.len() > 3 {
            lines.push(format!("And {} more.", report.failures.len() - 3));
        }
        if !report.unchecked.is_empty() {
            lines.push(format!(
                "ZENITH did not check {}; SOLAR will.",
                report
                    .unchecked
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        Err(Band {
            tone: Tone::Error,
            lines,
            underline,
        })
    }

    fn act_on_apis(&mut self, action: Action) {
        if self.apis.form.is_some() {
            self.act_on_form(action);
            return;
        }
        if self.apis.filtering {
            match action {
                Action::Close => self.apis.filtering = false,
                Action::Backspace => {
                    self.apis.filter.backspace();
                    self.apis.selected = 0;
                }
                _ => {}
            }
            return;
        }
        let count = self
            .catalogue()
            .map_or(0, |catalogue| self.apis.visible(catalogue).len());
        let page = usize::from(self.size.1.saturating_sub(6)).max(1);
        match action {
            Action::Up => {
                self.apis.selected = self.apis.selected.saturating_sub(1);
                self.apis.scroll = 0;
            }
            Action::Down => {
                self.apis.selected = (self.apis.selected + 1).min(count.saturating_sub(1));
                self.apis.scroll = 0;
            }
            Action::Top => {
                self.apis.selected = 0;
                self.apis.scroll = 0;
            }
            Action::Bottom => {
                self.apis.selected = count.saturating_sub(1);
                self.apis.scroll = 0;
            }
            Action::PageDown => {
                self.apis.scroll = self
                    .apis
                    .scroll
                    .saturating_add(u16::try_from(page).unwrap_or(u16::MAX));
            }
            Action::PageUp => {
                self.apis.scroll = self
                    .apis
                    .scroll
                    .saturating_sub(u16::try_from(page).unwrap_or(u16::MAX));
            }
            Action::RunExample(number) => self.run_example(usize::from(number).saturating_sub(1)),
            Action::RunAllExamples => {
                let count = self.selected_api().map_or(0, |api| api.examples.len());
                for index in 0..count {
                    self.run_example(index);
                }
            }
            Action::OpenForm => {
                let form = self.selected_api().map(Form::for_api);
                if form.is_some() {
                    self.apis.form = form;
                }
            }
            Action::Edit => {
                let line = self.selected_api().map(|api| {
                    let params = api
                        .examples
                        .first()
                        .map_or_else(|| json!({}), |example| example.params.clone());
                    format!("/call {} {params}", api.name)
                });
                if let Some(line) = line {
                    self.go_to(Tab::Session);
                    self.session.editor.set(&line);
                    self.refresh_completion();
                }
            }
            Action::Filter => self.apis.filtering = true,
            Action::ReloadManifest => {
                let sent = self.send_call(
                    "solar.manifest",
                    &json!({}),
                    Origin::Reload,
                    Layout::List,
                    false,
                );
                if let Err(error) = sent {
                    self.flash = Some((Tone::Error, error));
                }
            }
            _ => {}
        }
    }

    fn selected_api(&self) -> Option<&zenith_client::manifest::Api> {
        self.catalogue()
            .and_then(|catalogue| self.apis.current(catalogue))
    }

    fn run_example(&mut self, index: usize) {
        let Some((name, count, params)) = self.selected_api().map(|api| {
            (
                api.name.clone(),
                api.examples.len(),
                api.examples
                    .get(index)
                    .map(|example| example.params.clone()),
            )
        }) else {
            return;
        };
        let Some(params) = params else {
            self.flash = Some((
                Tone::Error,
                format!(
                    "{name} has {count} {}.",
                    if count == 1 { "example" } else { "examples" }
                ),
            ));
            return;
        };
        let echo = Entry::Command {
            text: format!("/call {name} {params}"),
        };
        if !self.link.connected() {
            self.flash = Some((
                Tone::Error,
                "SOLAR is not connected, so nothing was sent. Ctrl+R starts it.".to_owned(),
            ));
            return;
        }
        self.session.push(echo);
        let origin = Origin::Example {
            api: name.clone(),
            index,
        };
        match self.send_call(&name, &params, origin, Layout::Generic, true) {
            Ok(_) => {
                self.apis
                    .results
                    .insert((name, index), ExampleResult::Running);
            }
            Err(error) => self.flash = Some((Tone::Error, error)),
        }
    }

    fn act_on_form(&mut self, action: Action) {
        let Some(form) = self.apis.form.as_mut() else {
            return;
        };
        let count = form.fields.len();
        match action {
            Action::NextField | Action::Down => {
                if count > 0 {
                    form.focus = (form.focus + 1) % count;
                }
            }
            Action::PreviousField | Action::Up => {
                if count > 0 {
                    form.focus = (form.focus + count - 1) % count;
                }
            }
            Action::Toggle => {
                if let Some(field) = form.focused() {
                    field.cycle(true);
                }
            }
            Action::Left | Action::Right => {
                let forward = action == Action::Right;
                if let Some(field) = form.focused() {
                    if field.takes_text() {
                        if forward {
                            field.editor.right();
                        } else {
                            field.editor.left();
                        }
                    } else {
                        field.cycle(forward);
                    }
                }
            }
            Action::Backspace => {
                if let Some(field) = form.focused() {
                    field.editor.backspace();
                }
            }
            Action::Run => self.run_form(),
            _ => {}
        }
    }

    fn run_form(&mut self) {
        let Some(form) = self.apis.form.as_ref() else {
            return;
        };
        let api_name = form.api.clone();
        let params = match form.params() {
            Ok(params) => params,
            Err(errors) => {
                if let Some(form) = self.apis.form.as_mut() {
                    form.errors = errors;
                }
                return;
            }
        };
        let report = self
            .catalogue()
            .and_then(|catalogue| catalogue.api(&api_name))
            .map(|api| api.params().validate(&params));
        if let Some(report) = report.filter(|report| !report.is_valid()) {
            if let Some(form) = self.apis.form.as_mut() {
                form.errors = report
                    .failures
                    .iter()
                    .map(|failure| {
                        let field =
                            failure
                                .at
                                .segments()
                                .first()
                                .and_then(|segment| match segment {
                                    zenith_client::pointer::Segment::Key(name) => {
                                        form.fields.iter().position(|field| &field.name == name)
                                    }
                                    zenith_client::pointer::Segment::Index(_) => None,
                                });
                        (field, failure.sentence())
                    })
                    .collect();
                form.unchecked = report.unchecked.into_iter().collect();
            }
            return;
        }
        let echo = Entry::Command {
            text: format!("/call {api_name} {params}"),
        };
        if !self.link.connected() {
            if let Some(form) = self.apis.form.as_mut() {
                form.errors = vec![(
                    None,
                    "SOLAR is not connected, so nothing was sent. Ctrl+R starts it.".to_owned(),
                )];
            }
            return;
        }
        self.session.push(echo);
        match self.send_call(&api_name, &params, Origin::Form, Layout::Generic, true) {
            Ok(_) => {
                self.apis.form = None;
                self.flash = Some((
                    Tone::Info,
                    format!("Sent {api_name}; the Session tab has the answer."),
                ));
            }
            Err(error) => {
                if let Some(form) = self.apis.form.as_mut() {
                    form.errors = vec![(None, error)];
                }
            }
        }
    }

    fn act_on_log(&mut self, action: Action) {
        let visible = self.log.visible();
        let current = self.log.current(&visible);
        let position = current.and_then(|number| visible.iter().position(|n| *n == number));
        let page = usize::from(self.size.1.saturating_sub(5)).max(1);
        let select = |position: usize| {
            visible
                .get(position.min(visible.len().saturating_sub(1)))
                .copied()
        };
        match action {
            Action::Level(level) => self.set_log_level(level),
            Action::LowerLevel => self.set_log_level(self.log.minimum.lower()),
            Action::RaiseLevel => self.set_log_level(self.log.minimum.raise()),
            Action::Up => {
                self.log.selected = position.map(|p| p.saturating_sub(1)).and_then(select);
            }
            Action::Down => {
                self.log.selected = position.map(|p| p + 1).and_then(select);
                if self.log.selected == visible.last().copied() {
                    self.log.selected = None;
                }
            }
            Action::PageUp => {
                self.log.selected = position.map(|p| p.saturating_sub(page)).and_then(select);
            }
            Action::PageDown => {
                self.log.selected = position.map(|p| p + page).and_then(select);
                if self.log.selected == visible.last().copied() {
                    self.log.selected = None;
                }
            }
            Action::Top => self.log.selected = visible.first().copied(),
            Action::Bottom => self.log.selected = None,
            Action::Expand => self.log.expanded = !self.log.expanded,
            Action::Clear => {
                self.log.entries.clear();
                self.log.selected = None;
            }
            _ => {}
        }
    }

    fn set_log_level(&mut self, level: LevelKey) {
        self.log.minimum = level;
        self.log.selected = None;
    }

    fn act_on_history(&mut self, action: Action) {
        let first = self.history.calls.first_number();
        let last = self.history.calls.next_number().checked_sub(1);
        let current = self.history.current();
        let page = u64::from(self.size.1.saturating_sub(8)).max(1);
        let Some(last) = last.filter(|last| *last >= first) else {
            return;
        };
        let follow_at = |number: u64| (number < last).then_some(number);
        match action {
            Action::Up => {
                self.history.selected = current.map(|number| number.saturating_sub(1).max(first));
            }
            Action::Down => {
                self.history.selected = current.and_then(|number| follow_at(number + 1));
            }
            Action::PageUp => {
                self.history.selected =
                    current.map(|number| number.saturating_sub(page).max(first));
            }
            Action::PageDown => {
                self.history.selected =
                    current.and_then(|number| follow_at((number + page).min(last)));
            }
            Action::Top => self.history.selected = Some(first),
            Action::Bottom => self.history.selected = None,
            Action::Run => {
                if let Some(record) = current {
                    self.overlay = Some(Overlay::Viewer { record, scroll: 0 });
                }
            }
            Action::RunAgain => self.run_again(current),
            Action::Edit => {
                if let Some(line) = current.and_then(|record| self.command_for(record)) {
                    self.go_to(Tab::Session);
                    self.session.editor.set(&line);
                    self.refresh_completion();
                }
            }
            Action::Export => self.effects.push(Effect::Export { path: None }),
            _ => {}
        }
    }

    /// The command that would make this call again.
    fn command_for(&self, record: u64) -> Option<String> {
        let record = self.history.calls.get(record)?;
        if record.origin == Origin::Raw {
            return Some(format!("/raw {}", record.request));
        }
        let request = record.request_value()?;
        let method = request.get("method")?.as_str()?;
        let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
        Some(format!("/call {method} {params}"))
    }

    fn run_again(&mut self, record: Option<u64>) {
        let Some(record) = record
            .and_then(|number| self.history.calls.get(number))
            .cloned()
        else {
            return;
        };
        let result = if record.origin == Origin::Raw {
            self.session.push(Entry::Command {
                text: format!("/raw {}", record.request),
            });
            self.send_raw(&record.request).map(drop)
        } else if let Some(request) = record.request_value() {
            let method = request
                .get("method")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
            if self.link.connected() {
                self.session.push(Entry::Command {
                    text: format!("/call {method} {params}"),
                });
            }
            self.send_call(&method, &params, Origin::Again, Layout::Generic, true)
                .map(drop)
        } else {
            Err("The request is not JSON, so it cannot be sent again as a call.".to_owned())
        };
        match result {
            Ok(()) => {
                self.flash = Some((
                    Tone::Info,
                    "Sent again; the Session tab has the answer.".to_owned(),
                ));
            }
            Err(error) => self.flash = Some((Tone::Error, error)),
        }
    }
}

fn ring_bytes(record: &CallRecord) -> usize {
    crate::ring::Measured::bytes(record)
}

/// Whether a key is text to type, `AltGr` included; `keys::is_text` says why.
fn is_text(key: &KeyEvent) -> bool {
    keys::is_text(key)
}

/// The part of the JSON text a schema failure is about: the key of a member that should
/// not be there, the braces of an object missing a member, the value otherwise.
fn failure_span(tree: &json_text::Node, failure: &SchemaFailure) -> Option<json_text::Span> {
    match &failure.kind {
        FailureKind::NotAllowed { property, .. } | FailureKind::NotEvaluated { property } => {
            let segments = failure.at.segments();
            let parent = segments[..segments.len().saturating_sub(1)].iter().fold(
                zenith_client::pointer::Pointer::root(),
                |pointer, segment| match segment {
                    zenith_client::pointer::Segment::Key(name) => pointer.key(name),
                    zenith_client::pointer::Segment::Index(index) => pointer.index(*index),
                },
            );
            Some(tree.find(&parent)?.member(property)?.key_span)
        }
        _ => Some(tree.find(&failure.at)?.span()),
    }
}

#[cfg(test)]
mod tests;
