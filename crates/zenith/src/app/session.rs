//! The Session tab: the transcript, the command line, and the band between them.

use std::ops::Range;

use crate::completion::Completion;
use crate::editor::LineEditor;
use crate::limits;
use crate::ring::{Measured, Ring};

/// How a response is laid out in the transcript.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// Keys and values, for any call.
    Generic,
    /// The table of `/list`.
    List,
    /// The entry of `/describe`.
    Describe,
    /// The one line of `/ping`.
    Ping,
    /// The versions of `/version`.
    Version,
}

/// How much a notice matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// Information.
    Info,
    /// Something went wrong.
    Error,
}

/// One entry of the transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    /// A line that was run, as typed.
    Command {
        /// The line.
        text: String,
    },
    /// A call, drawn from the History.
    Call {
        /// Its number in the History.
        record: u64,
        /// How to lay it out.
        layout: Layout,
    },
    /// Something ZENITH says, not SOLAR.
    Notice {
        /// How much it matters.
        tone: Tone,
        /// Its lines.
        lines: Vec<String>,
    },
    /// The table of commands, or one command.
    Help {
        /// The command, or all of them.
        topic: Option<&'static str>,
    },
    /// A line from SOLAR that no call was waiting for.
    Unexpected {
        /// The line.
        line: String,
    },
    /// SOLAR's mark, at the start of every connection, with the version that answered.
    Mark {
        /// `solar_version`, as the handshake read it.
        version: String,
    },
}

impl Measured for Entry {
    fn bytes(&self) -> usize {
        limits::ENTRY_OVERHEAD
            + match self {
                Self::Command { text } => text.len(),
                Self::Notice { lines, .. } => lines.iter().map(String::len).sum(),
                Self::Unexpected { line } => line.len(),
                Self::Mark { version } => version.len(),
                Self::Call { .. } | Self::Help { .. } => 0,
            }
    }
}

/// What the band above the command line says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Band {
    /// How much it matters.
    pub tone: Tone,
    /// Its lines.
    pub lines: Vec<String>,
    /// The bytes of the command line it is about, to underline.
    pub underline: Option<Range<usize>>,
}

/// The Session tab.
#[derive(Debug, Clone)]
pub struct Session {
    /// The transcript.
    pub transcript: Ring<Entry>,
    /// How many lines the view is scrolled up from the bottom; zero follows the bottom.
    pub scroll: usize,
    /// How many entries arrived below while scrolled up.
    pub unseen: usize,
    /// The command line.
    pub editor: LineEditor,
    /// The completion at the cursor, recomputed on every edit.
    pub completion: Option<Completion>,
    /// The highlighted candidate.
    pub highlighted: usize,
    /// Whether `Esc` closed the menu, which stays closed until the next edit.
    pub menu_dismissed: bool,
    /// The band above the command line.
    pub band: Option<Band>,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            transcript: Ring::new(limits::TRANSCRIPT_ENTRIES, limits::TRANSCRIPT_BYTES),
            scroll: 0,
            unseen: 0,
            editor: LineEditor::new(),
            completion: None,
            highlighted: 0,
            menu_dismissed: false,
            band: None,
        }
    }
}

impl Session {
    /// Adds an entry, and counts it as unseen when the view is scrolled up.
    pub fn push(&mut self, entry: Entry) {
        self.transcript.push(entry);
        if self.scroll > 0 {
            self.unseen += 1;
        }
    }

    /// Whether the completion menu is showing.
    #[must_use]
    pub fn menu_open(&self) -> bool {
        !self.menu_dismissed && self.completion.is_some()
    }
}
