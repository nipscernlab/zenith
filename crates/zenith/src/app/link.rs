//! The state of the connection to SOLAR, and every way it can fail, in words.
//!
//! Each failure says what happened in one sentence and what to do in another, which is
//! the table of `docs/DESIGN.md`, section 9.5.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use zenith_client::calls::Tracker;
use zenith_client::handshake::{HandshakeError, ServerInfo};
use zenith_client::locate::{LocateError, Prepared};
use zenith_client::manifest::{Capabilities, Catalogue, ManifestError};

use crate::clock;
use crate::limits;

/// How long the handshake may take. Long, because the first run of a new copy of
/// `solar.exe` is scanned by the antivirus on Windows before it starts, which can take
/// seconds; `docs/OPEN_QUESTIONS.md` records the choice.
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);

/// How often ZENITH asks whether SOLAR has exited, once SOLAR has closed its output.
pub const EXIT_CHECK_INTERVAL: Duration = Duration::from_millis(50);

/// How many times it asks before it kills SOLAR.
pub const EXIT_CHECKS: u32 = 40;

/// How many of SOLAR's last lines of standard error a failure quotes.
pub const LAST_WORDS: usize = 6;

/// Where the connection is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Finding, copying and starting the binary.
    Starting,
    /// Waiting for `solar.version` and `solar.manifest`.
    Handshaking,
    /// Connected.
    Connected,
    /// Not connected, and the failure says why.
    Down,
}

/// Why the connection is down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// `solar` was not found.
    Locate(LocateError),
    /// The copy on Windows could not be made.
    Copy {
        /// What the system said.
        message: String,
    },
    /// The binary could not be started.
    Start {
        /// What the system said.
        message: String,
        /// The program.
        path: PathBuf,
        /// Whether the system refused permission.
        permission: bool,
    },
    /// SOLAR exited.
    Exited {
        /// How: `exited with code 1`.
        how: String,
        /// How long it had been connected, when it had connected at all.
        after: Option<Duration>,
        /// The calls that were still waiting.
        waiting: usize,
        /// Its last lines of standard error.
        last_words: Vec<String>,
    },
    /// SOLAR closed its output and has not exited yet.
    OutputClosed {
        /// What the reading thread said, when it was not a clean end.
        error: Option<String>,
    },
    /// SOLAR stopped reading its input.
    InputClosed {
        /// What the system said.
        error: String,
    },
    /// The protocol or the manifest layout is not one ZENITH knows.
    Handshake(HandshakeError),
    /// The manifest could not be read.
    Manifest(ManifestError),
    /// `solar.version` or `solar.manifest` answered with an error.
    Refused {
        /// The method.
        method: String,
        /// SOLAR's message.
        message: String,
    },
    /// SOLAR answered the handshake with something that is not the contract.
    NotContract {
        /// What, quoted.
        what: String,
    },
    /// SOLAR did not answer the handshake in time.
    Timeout,
}

impl Failure {
    /// What happened, in one sentence.
    #[must_use]
    pub fn what(&self) -> String {
        match self {
            Self::Locate(error) => error.to_string(),
            Self::Copy { message } | Self::Start { message, .. } => message.clone(),
            Self::Exited {
                how,
                after,
                waiting,
                ..
            } => {
                let when = match after {
                    Some(after) => format!(" after {} connected", clock::lasted(*after)),
                    None => " before it answered the handshake".to_owned(),
                };
                let calls = match waiting {
                    0 => String::new(),
                    1 => ", with 1 call still waiting".to_owned(),
                    many => format!(", with {many} calls still waiting"),
                };
                format!("SOLAR {how}{when}{calls}.")
            }
            Self::OutputClosed { error: None } => "SOLAR closed its output.".to_owned(),
            Self::OutputClosed { error: Some(error) } => {
                format!("SOLAR's output could not be read: {error}.")
            }
            Self::InputClosed { error } => format!("SOLAR stopped reading its input: {error}."),
            Self::Handshake(error) => error.to_string(),
            Self::Manifest(error) => error.to_string(),
            Self::Refused { method, message } => {
                format!("SOLAR refused {method}: {message}")
            }
            Self::NotContract { what } => {
                format!("SOLAR answered the handshake with something that is not JSON-RPC: {what}")
            }
            Self::Timeout => format!(
                "SOLAR started and did not answer the handshake within {} seconds.",
                HANDSHAKE_TIMEOUT.as_secs()
            ),
        }
    }

    /// What to do about it, in one sentence or two.
    #[must_use]
    pub fn advice(&self) -> String {
        match self {
            Self::Locate(LocateError::NotOnPath { .. }) => "Put the folder that holds solar on \
                the PATH, SOLAR's target/release when it was built from source, or give the \
                path with --solar <path> or the variable ZENITH_SOLAR."
                .to_owned(),
            Self::Locate(_) => "Give the path of the solar program itself, with --solar or \
                ZENITH_SOLAR."
                .to_owned(),
            Self::Copy { .. } => format!(
                "Check that the temporary directory, {}, can be written to.",
                std::env::temp_dir().display()
            ),
            Self::Start {
                permission: true,
                path,
                ..
            } => format!(
                "Make it executable, for example with chmod +x {}.",
                path.display()
            ),
            Self::Start { .. } => {
                "Check that it is the solar program built for this system.".to_owned()
            }
            Self::Exited { .. } | Self::OutputClosed { .. } | Self::InputClosed { .. } => {
                "Its last words are below and the Log tab has the rest. Ctrl+R starts it again."
                    .to_owned()
            }
            Self::Handshake(_) => {
                "Nothing more can be done from this ZENITH with this SOLAR.".to_owned()
            }
            Self::Manifest(_) | Self::Refused { .. } | Self::NotContract { .. } => {
                "That is a bug in SOLAR or in ZENITH; /report writes the file to send.".to_owned()
            }
            Self::Timeout => "The Log tab has whatever it wrote. Ctrl+R tries again.".to_owned(),
        }
    }

    /// A few words for the status bar.
    #[must_use]
    pub fn short(&self) -> String {
        match self {
            Self::Locate(LocateError::NotOnPath { .. }) => "solar is not on the PATH".to_owned(),
            Self::Locate(_) => "the path given is not solar".to_owned(),
            Self::Copy { .. } => "the copy could not be made".to_owned(),
            Self::Start { .. } => "it could not be started".to_owned(),
            Self::Exited { how, .. } => how.clone(),
            Self::OutputClosed { .. } => "its output closed".to_owned(),
            Self::InputClosed { .. } => "its input closed".to_owned(),
            Self::Handshake(HandshakeError::Protocol { theirs, .. }) => {
                format!("it speaks {theirs}")
            }
            Self::Handshake(HandshakeError::Layout { theirs, .. }) => {
                format!("its manifest is in layout {theirs}")
            }
            Self::Handshake(HandshakeError::Malformed { .. })
            | Self::Manifest(_)
            | Self::Refused { .. }
            | Self::NotContract { .. } => "the handshake failed".to_owned(),
            Self::Timeout => "no answer to the handshake".to_owned(),
        }
    }

    /// The last lines of standard error, when the failure has them.
    #[must_use]
    pub fn last_words(&self) -> &[String] {
        match self {
            Self::Exited { last_words, .. } => last_words,
            _ => &[],
        }
    }
}

/// The connection, in the state the interface needs to draw and decide.
#[derive(Debug)]
pub struct Link {
    /// Which connection this is, from 1; events of an older one are ignored.
    pub generation: u64,
    /// Where it is.
    pub phase: Phase,
    /// The calls waiting for this connection.
    pub tracker: Tracker,
    /// When the start was asked for.
    pub requested: Instant,
    /// When the handshake finished.
    pub connected_at: Option<Instant>,
    /// What `solar.version` said.
    pub info: Option<ServerInfo>,
    /// The catalogue of the manifest. It is kept when the connection goes down, so that
    /// the APIs tab still shows it, and replaced when a new one arrives.
    pub catalogue: Option<Catalogue>,
    /// What the manifest declares beyond calls.
    pub capabilities: Capabilities,
    /// The binary that was found, and the one that runs.
    pub binary: Option<Prepared>,
    /// SOLAR's process id.
    pub pid: Option<u32>,
    /// The round trip of the last call answered.
    pub last_round_trip: Option<Duration>,
    /// Why it is down.
    pub failure: Option<Failure>,
    /// Whether SOLAR has closed its output.
    pub output_closed: bool,
    /// When to ask next whether SOLAR has exited.
    pub next_exit_check: Option<Instant>,
    /// How many times it has been asked.
    pub exit_checks: u32,
    /// When the handshake gives up.
    pub handshake_deadline: Option<Instant>,
    /// The call number of `solar.version` in the handshake.
    pub version_call: Option<u64>,
    /// The call number of `solar.manifest` in the handshake.
    pub manifest_call: Option<u64>,
    /// The last lines of standard error of this connection.
    pub last_words: VecDeque<String>,
}

impl Link {
    /// A connection that is being started.
    #[must_use]
    pub fn starting(generation: u64, now: Instant, catalogue: Option<Catalogue>) -> Self {
        Self {
            generation,
            phase: Phase::Starting,
            tracker: Tracker::new(limits::CALLS_IN_FLIGHT),
            requested: now,
            connected_at: None,
            info: None,
            capabilities: catalogue
                .as_ref()
                .map(Catalogue::capabilities)
                .unwrap_or_default(),
            catalogue,
            binary: None,
            pid: None,
            last_round_trip: None,
            failure: None,
            output_closed: false,
            next_exit_check: None,
            exit_checks: 0,
            handshake_deadline: None,
            version_call: None,
            manifest_call: None,
            last_words: VecDeque::new(),
        }
    }

    /// Whether calls can be made.
    #[must_use]
    pub fn connected(&self) -> bool {
        self.phase == Phase::Connected
    }

    /// Keeps one of SOLAR's lines of standard error among its last words.
    pub fn remember(&mut self, line: &str) {
        if self.last_words.len() == LAST_WORDS {
            self.last_words.pop_front();
        }
        let cut: String = line.chars().take(300).collect();
        self.last_words.push_back(cut);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_exit_says_how_when_and_what_was_waiting() {
        let failure = Failure::Exited {
            how: "exited with code 101".to_owned(),
            after: Some(Duration::from_secs(192)),
            waiting: 2,
            last_words: vec!["thread 'main' panicked".to_owned()],
        };
        assert_eq!(
            failure.what(),
            "SOLAR exited with code 101 after 3m connected, with 2 calls still waiting."
        );
        assert_eq!(failure.last_words().len(), 1);
        let early = Failure::Exited {
            how: "exited with code 3".to_owned(),
            after: None,
            waiting: 0,
            last_words: Vec::new(),
        };
        assert_eq!(
            early.what(),
            "SOLAR exited with code 3 before it answered the handshake."
        );
    }

    #[test]
    fn not_being_on_the_path_says_the_three_ways_to_point_at_solar() {
        let failure = Failure::Locate(LocateError::NotOnPath { directories: 3 });
        let advice = failure.advice();
        assert!(
            advice.contains("PATH")
                && advice.contains("--solar")
                && advice.contains("ZENITH_SOLAR")
        );
    }

    #[test]
    fn the_last_words_keep_only_the_last_few_lines() {
        let mut link = Link::starting(1, Instant::now(), None);
        for number in 0..10 {
            link.remember(&format!("line {number}"));
        }
        assert_eq!(link.last_words.len(), LAST_WORDS);
        assert_eq!(link.last_words.front().map(String::as_str), Some("line 4"));
    }
}
