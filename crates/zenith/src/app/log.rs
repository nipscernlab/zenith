//! The Log tab: SOLAR's standard error, kept in a ring buffer and filtered by level.
//!
//! ZENITH starts SOLAR at `trace`, unless told otherwise, with `SOLAR_LOG_FORMAT=json`.
//! When SOLAR's manifest has `solar.set_log_level`, the level chosen here is also SOLAR's
//! own from then on; a SOLAR without it reads its level once, when it starts, and the
//! level chosen here only filters what it wrote.

use std::time::SystemTime;

use serde_json::Value;

use crate::keys::LevelKey;
use crate::limits;
use crate::ring::{Measured, Ring};

/// The level of a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// A level SOLAR wrote.
    Known(LevelKey),
    /// A line that is not one of SOLAR's JSON log lines, such as a panic.
    Raw,
}

impl Level {
    /// The word the tab writes, always, so that the level is never shown by colour alone.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Known(LevelKey::Error) => "ERROR",
            Self::Known(LevelKey::Warn) => "WARN",
            Self::Known(LevelKey::Info) => "INFO",
            Self::Known(LevelKey::Debug) => "DEBUG",
            Self::Known(LevelKey::Trace) => "TRACE",
            Self::Raw => "RAW",
        }
    }
}

impl LevelKey {
    /// Every level, from the fewest lines to the most.
    pub const ALL: [Self; 5] = [
        Self::Error,
        Self::Warn,
        Self::Info,
        Self::Debug,
        Self::Trace,
    ];

    /// The name SOLAR uses for it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }

    /// The level a name stands for.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|level| level.name().eq_ignore_ascii_case(name.trim()))
    }

    /// One level more.
    #[must_use]
    pub fn raise(self) -> Self {
        match self {
            Self::Error => Self::Warn,
            Self::Warn => Self::Info,
            Self::Info => Self::Debug,
            Self::Debug | Self::Trace => Self::Trace,
        }
    }

    /// One level fewer.
    #[must_use]
    pub fn lower(self) -> Self {
        match self {
            Self::Error | Self::Warn => Self::Error,
            Self::Info => Self::Warn,
            Self::Debug => Self::Info,
            Self::Trace => Self::Debug,
        }
    }
}

/// One line of SOLAR's standard error.
#[derive(Debug, Clone, PartialEq)]
pub struct LogEntry {
    /// When ZENITH read it.
    pub received_wall: SystemTime,
    /// The connection it came from.
    pub connection: u64,
    /// `time`, as SOLAR wrote it.
    pub time: Option<String>,
    /// `level`.
    pub level: Level,
    /// `message`, or the whole line when it is not JSON.
    pub message: String,
    /// `request_id`, when SOLAR named one.
    pub request_id: Option<String>,
    /// `method`.
    pub method: Option<String>,
    /// `duration_us`.
    pub duration_us: Option<u64>,
    /// How many bytes were cut from the end of a line too long to keep.
    pub cut: usize,
}

impl LogEntry {
    /// Reads one line, as SOLAR writes it with `SOLAR_LOG_FORMAT=json`, or as it arrived.
    #[must_use]
    pub fn read(text: &str, cut: usize, connection: u64, received_wall: SystemTime) -> Self {
        let parsed = serde_json::from_str::<Value>(text)
            .ok()
            .filter(Value::is_object);
        let Some(entry) = parsed else {
            return Self {
                received_wall,
                connection,
                time: None,
                level: Level::Raw,
                message: text.to_owned(),
                request_id: None,
                method: None,
                duration_us: None,
                cut,
            };
        };
        let text_of = |key: &str| entry.get(key).and_then(Value::as_str).map(str::to_owned);
        let level = text_of("level")
            .and_then(|name| LevelKey::parse(&name))
            .map_or(Level::Raw, Level::Known);
        Self {
            received_wall,
            connection,
            time: text_of("time"),
            level,
            message: text_of("message").unwrap_or_else(|| text.to_owned()),
            request_id: entry
                .get("request_id")
                .filter(|value| !value.is_null())
                .map(|value| {
                    value
                        .as_str()
                        .map_or_else(|| value.to_string(), str::to_owned)
                }),
            method: text_of("method"),
            duration_us: entry.get("duration_us").and_then(Value::as_u64),
            cut,
        }
    }

    /// Whether the chosen level shows this line. A raw line is always shown, because it
    /// is usually the one that matters, such as a panic.
    #[must_use]
    pub fn shown_at(&self, minimum: LevelKey) -> bool {
        match self.level {
            Level::Known(level) => level <= minimum,
            Level::Raw => true,
        }
    }
}

impl Measured for LogEntry {
    fn bytes(&self) -> usize {
        self.message.len()
            + self.time.as_ref().map_or(0, String::len)
            + self.request_id.as_ref().map_or(0, String::len)
            + self.method.as_ref().map_or(0, String::len)
            + limits::ENTRY_OVERHEAD
    }
}

/// The Log tab.
#[derive(Debug, Clone)]
pub struct LogTab {
    /// Every line kept.
    pub entries: Ring<LogEntry>,
    /// The chosen level.
    pub minimum: LevelKey,
    /// The selected line, or `None` to follow the newest.
    pub selected: Option<u64>,
    /// Whether the selected line is shown whole.
    pub expanded: bool,
    /// The level SOLAR logs at, as far as ZENITH knows: the one it was started with, then
    /// the one `solar.set_log_level` last answered with.
    pub solar_level: String,
    /// Whether SOLAR can change its level while it runs, because its manifest has
    /// `solar.set_log_level`.
    pub can_set: bool,
    /// The level asked of SOLAR by a call that has not been answered yet.
    pub setting: Option<LevelKey>,
    /// Why the last change of SOLAR's level did not happen, when it did not.
    pub note: Option<String>,
}

impl LogTab {
    /// An empty tab, for a SOLAR started at `solar_level`.
    #[must_use]
    pub fn new(solar_level: &str) -> Self {
        Self {
            entries: Ring::new(limits::LOG_ENTRIES, limits::LOG_BYTES),
            minimum: LevelKey::Info,
            selected: None,
            expanded: false,
            solar_level: solar_level.to_owned(),
            can_set: false,
            setting: None,
            note: None,
        }
    }

    /// The numbers of the lines the chosen level shows, oldest first.
    #[must_use]
    pub fn visible(&self) -> Vec<u64> {
        self.entries
            .iter()
            .filter(|(_, entry)| entry.shown_at(self.minimum))
            .map(|(number, _)| number)
            .collect()
    }

    /// The selected line, or the newest shown.
    #[must_use]
    pub fn current(&self, visible: &[u64]) -> Option<u64> {
        self.selected
            .filter(|number| visible.contains(number))
            .or_else(|| visible.last().copied())
    }

    /// Whether SOLAR logs at a level lower than the one chosen, so that some lines the
    /// level asks for do not arrive.
    #[must_use]
    pub fn solar_is_quieter(&self) -> bool {
        LevelKey::parse(&self.solar_level).is_some_and(|started| started < self.minimum)
            || self.solar_level == "off"
    }

    /// What the header says of SOLAR's own level, when there is something to say: what it
    /// logs at, when SOLAR can change it; that it was started quieter than the level
    /// chosen, when it cannot.
    #[must_use]
    pub fn solar_said(&self) -> Option<String> {
        if let Some(note) = &self.note {
            return Some(note.clone());
        }
        if let Some(level) = self.setting {
            return Some(format!("asking SOLAR for {}", level.name()));
        }
        if self.can_set {
            return Some(format!("SOLAR logs at {}", self.solar_level));
        }
        self.solar_is_quieter()
            .then(|| format!("SOLAR was started at {}", self.solar_level))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRACE: &str = r#"{"duration_us":null,"level":"trace","message":"--> {\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"solar.ping\"}","method":null,"request_id":null,"time":"2026-09-27T15:47:00.906643Z"}"#;

    #[test]
    fn a_json_line_of_solar_is_read_into_its_members() {
        let entry = LogEntry::read(TRACE, 0, 1, SystemTime::UNIX_EPOCH);
        assert_eq!(entry.level, Level::Known(LevelKey::Trace));
        assert!(entry.message.starts_with("--> "));
        assert_eq!(entry.time.as_deref(), Some("2026-09-27T15:47:00.906643Z"));
        assert_eq!(entry.request_id, None);
    }

    #[test]
    fn a_line_that_is_not_json_is_raw_and_always_shown() {
        let entry = LogEntry::read(
            "thread 'main' panicked at src/x.rs:1:1",
            0,
            1,
            SystemTime::UNIX_EPOCH,
        );
        assert_eq!(entry.level, Level::Raw);
        assert!(entry.shown_at(LevelKey::Error));
    }

    #[test]
    fn the_level_filters_what_is_shown() {
        let mut tab = LogTab::new("trace");
        tab.entries
            .push(LogEntry::read(TRACE, 0, 1, SystemTime::UNIX_EPOCH));
        tab.entries.push(LogEntry::read(
            r#"{"level":"info","message":"the session ended after 1 calls"}"#,
            0,
            1,
            SystemTime::UNIX_EPOCH,
        ));
        assert_eq!(tab.visible().len(), 1);
        tab.minimum = LevelKey::Trace;
        assert_eq!(tab.visible().len(), 2);
    }

    #[test]
    fn levels_are_raised_and_lowered_within_their_range() {
        assert_eq!(LevelKey::Trace.raise(), LevelKey::Trace);
        assert_eq!(LevelKey::Error.lower(), LevelKey::Error);
        assert_eq!(LevelKey::Info.raise().lower(), LevelKey::Info);
    }

    #[test]
    fn a_quieter_solar_is_noticed() {
        let mut tab = LogTab::new("info");
        tab.minimum = LevelKey::Trace;
        assert!(tab.solar_is_quieter());
        assert_eq!(
            tab.solar_said().as_deref(),
            Some("SOLAR was started at info")
        );
        tab.minimum = LevelKey::Warn;
        assert!(!tab.solar_is_quieter());
        assert_eq!(tab.solar_said(), None);
        let mut silent = LogTab::new("off");
        silent.minimum = LevelKey::Error;
        assert!(silent.solar_is_quieter());
    }

    #[test]
    fn a_solar_that_can_set_its_level_is_said_to_log_at_it() {
        let mut tab = LogTab::new("trace");
        tab.can_set = true;
        assert_eq!(tab.solar_said().as_deref(), Some("SOLAR logs at trace"));
        tab.setting = Some(LevelKey::Warn);
        assert_eq!(tab.solar_said().as_deref(), Some("asking SOLAR for warn"));
        tab.setting = None;
        tab.note = Some("SOLAR kept its level: no".to_owned());
        assert_eq!(
            tab.solar_said().as_deref(),
            Some("SOLAR kept its level: no")
        );
    }

    #[test]
    fn a_line_of_the_log_is_counted_by_its_members_and_an_overhead() {
        let entry = LogEntry::read(TRACE, 0, 1, SystemTime::UNIX_EPOCH);
        let message = entry.message.len();
        let time = "2026-09-27T15:47:00.906643Z".len();
        assert_eq!(entry.request_id, None);
        assert_eq!(entry.bytes(), message + time + limits::ENTRY_OVERHEAD);
        let full = LogEntry::read(
            r#"{"level":"debug","message":"answered","method":"solar.ping","request_id":"3","time":"t"}"#,
            0,
            1,
            SystemTime::UNIX_EPOCH,
        );
        assert_eq!(
            full.bytes(),
            "answered".len() + 1 + 1 + "solar.ping".len() + limits::ENTRY_OVERHEAD
        );
        let raw = LogEntry::read("panicked", 0, 1, SystemTime::UNIX_EPOCH);
        assert_eq!(raw.bytes(), "panicked".len() + limits::ENTRY_OVERHEAD);
    }
}
