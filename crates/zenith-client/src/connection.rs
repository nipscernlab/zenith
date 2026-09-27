//! The child process `solar serve --stdio`, and the three threads that talk to it.
//!
//! One thread writes request lines to SOLAR's standard input, one reads its standard
//! output a line at a time, and one reads its standard error. The two readers hand what
//! they read to a sink, which the application points at its own event channel, so that
//! the interface waits on one channel for everything and never polls.
//!
//! Every line has a limit in bytes, so that nothing SOLAR writes can make ZENITH hold
//! more than that: a response line may be up to 16 MiB, the limit SOLAR itself puts on a
//! request line, and a line of standard error up to 64 KiB. A longer line is read to its
//! end and dropped, and the event says how long it was.

use std::fmt;
use std::io::{self, BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::thread;
use std::time::{Duration, Instant};

use crate::envelope::Id;
use crate::locate::Prepared;

/// The longest response line ZENITH keeps, in bytes: 16 MiB, the same limit SOLAR puts on
/// a request line, section 2 of its contract.
pub const RESPONSE_LINE_LIMIT: usize = 16 * 1024 * 1024;

/// The longest line of standard error ZENITH keeps, in bytes.
pub const STDERR_LINE_LIMIT: usize = 64 * 1024;

/// How many request lines may wait to be written before ZENITH refuses more.
pub const OUTGOING_LIMIT: usize = 256;

/// How a connection is started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// The value of `SOLAR_LOG` SOLAR is started with.
    pub log_level: String,
    /// Variables added to SOLAR's environment, on top of the two above. The tests use it
    /// to tell ZENITH's test double how to misbehave; ZENITH itself adds nothing.
    pub environment: Vec<(String, String)>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            log_level: "trace".to_owned(),
            environment: Vec::new(),
        }
    }
}

/// Something that happened on the connection.
#[derive(Debug, Clone)]
pub enum Event {
    /// A whole line of standard output.
    Line {
        /// The line, without its newline or a carriage return before it.
        text: String,
        /// Whether the bytes were not valid UTF-8 and had to be replaced, which section 2
        /// of the contract does not allow.
        invalid_utf8: bool,
        /// When it was read.
        at: Instant,
    },
    /// A line of standard output longer than [`RESPONSE_LINE_LIMIT`], dropped.
    LineTooLong {
        /// How many bytes it had.
        bytes: usize,
        /// The `id` found near its start, when there was one to find.
        id: Option<Id>,
        /// When its end was read.
        at: Instant,
    },
    /// Standard output ended: SOLAR is exiting, or has exited.
    OutputClosed {
        /// The error that ended it, when it was not a clean end.
        error: Option<String>,
        /// When.
        at: Instant,
    },
    /// A line of standard error.
    Stderr {
        /// The line, cut at [`STDERR_LINE_LIMIT`].
        text: String,
        /// How many bytes were cut from its end.
        cut: usize,
        /// When it was read.
        at: Instant,
    },
    /// Standard error ended.
    StderrClosed,
    /// A request could not be written, because SOLAR's input is closed.
    WriteFailed {
        /// What the system said.
        error: String,
    },
}

/// Where the threads send their events: the connection's generation, which tells a new
/// connection's events from an old one's, and the event. It returns `false` when nobody
/// is listening any more, and the thread then stops.
pub type Sink = Arc<dyn Fn(u64, Event) -> bool + Send + Sync>;

/// A running `solar serve --stdio`.
#[derive(Debug)]
pub struct Connection {
    generation: u64,
    child: Child,
    outgoing: Option<SyncSender<String>>,
    prepared: Prepared,
    started: Instant,
}

/// Why SOLAR could not be started.
#[derive(Debug)]
pub struct StartError {
    /// The program that was to run.
    pub path: std::path::PathBuf,
    /// What the system said.
    pub error: io::Error,
}

impl fmt::Display for StartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "ZENITH could not start {}: {}.",
            self.path.display(),
            self.error
        )
    }
}

impl std::error::Error for StartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// Why a line could not be sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendError {
    /// [`OUTGOING_LIMIT`] lines are already waiting to be written: SOLAR is not reading
    /// its input.
    Full,
    /// The connection's input is closed.
    Closed,
}

impl fmt::Display for SendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full => write!(
                formatter,
                "SOLAR is not reading its input: {OUTGOING_LIMIT} lines are already waiting."
            ),
            Self::Closed => formatter.write_str("SOLAR's input is closed."),
        }
    }
}

impl std::error::Error for SendError {}

impl Connection {
    /// Starts `solar serve --stdio` and the three threads.
    ///
    /// # Errors
    ///
    /// [`StartError`] when the system cannot start the program or its threads.
    pub fn start(
        prepared: Prepared,
        settings: &Settings,
        generation: u64,
        sink: Sink,
    ) -> Result<Self, StartError> {
        let failed = |error| StartError {
            path: prepared.runs.clone(),
            error,
        };
        let mut command = Command::new(&prepared.runs);
        command
            .args(["serve", "--stdio"])
            .env("SOLAR_LOG", &settings.log_level)
            .env("SOLAR_LOG_FORMAT", "json")
            .envs(settings.environment.iter().map(|(key, value)| (key, value)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        detach(&mut command);
        let mut child = command.spawn().map_err(failed)?;
        let started = Instant::now();
        let (Some(stdin), Some(stdout), Some(stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            let _ = child.kill();
            return Err(failed(io::Error::other(
                "the pipes to solar were not created",
            )));
        };
        let (outgoing, lines) = mpsc::sync_channel::<String>(OUTGOING_LIMIT);
        let spawned = spawn_writer(stdin, lines, generation, Arc::clone(&sink))
            .and_then(|()| spawn_output_reader(stdout, generation, Arc::clone(&sink)))
            .and_then(|()| spawn_stderr_reader(stderr, generation, sink));
        if let Err(error) = spawned {
            let _ = child.kill();
            return Err(failed(error));
        }
        Ok(Self {
            generation,
            child,
            outgoing: Some(outgoing),
            prepared,
            started,
        })
    }

    /// The generation this connection's events carry.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The binary that runs, and where it came from.
    #[must_use]
    pub fn prepared(&self) -> &Prepared {
        &self.prepared
    }

    /// The process id of SOLAR.
    #[must_use]
    pub fn id(&self) -> u32 {
        self.child.id()
    }

    /// When SOLAR was started.
    #[must_use]
    pub fn started(&self) -> Instant {
        self.started
    }

    /// Queues one line to be written, without its newline, and returns at once.
    ///
    /// # Errors
    ///
    /// [`SendError::Full`] when SOLAR has stopped reading, and [`SendError::Closed`] when
    /// the input is closed.
    pub fn send(&self, line: String) -> Result<(), SendError> {
        let outgoing = self.outgoing.as_ref().ok_or(SendError::Closed)?;
        outgoing.try_send(line).map_err(|error| match error {
            TrySendError::Full(_) => SendError::Full,
            TrySendError::Disconnected(_) => SendError::Closed,
        })
    }

    /// Closes SOLAR's input, which ends a session the way the contract says one ends.
    pub fn close_input(&mut self) {
        self.outgoing = None;
    }

    /// The exit status, if SOLAR has exited.
    ///
    /// # Errors
    ///
    /// What the system says when it cannot be asked.
    pub fn try_exit(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    /// Ends the session: closes the input, waits up to `grace` for SOLAR to exit on its
    /// own, and kills it after that. This is the only place ZENITH waits in a loop, and it
    /// only runs when ZENITH is quitting or reconnecting.
    #[must_use]
    pub fn shutdown(mut self, grace: Duration) -> Option<ExitStatus> {
        self.close_input();
        let deadline = Instant::now() + grace;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => return Some(status),
                Ok(None) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(5));
                }
                _ => {
                    let _ = self.child.kill();
                    return self.child.wait().ok();
                }
            }
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        // A connection dropped without a shutdown is one ZENITH gave up on, for example in
        // a panic. SOLAR is killed rather than left running on its own.
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// Keeps SOLAR out of the reach of the terminal's signals, so that only ZENITH decides
/// when it ends: its own process group on Unix, no console on Windows, where a
/// `Ctrl+Break` would otherwise reach every process attached to the console.
fn detach(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
}

fn spawn_writer(
    stdin: ChildStdin,
    lines: mpsc::Receiver<String>,
    generation: u64,
    sink: Sink,
) -> io::Result<()> {
    thread::Builder::new()
        .name("zenith-solar-input".to_owned())
        .spawn(move || {
            let mut stdin = stdin;
            for line in lines {
                let written = stdin
                    .write_all(line.as_bytes())
                    .and_then(|()| stdin.write_all(b"\n"))
                    .and_then(|()| stdin.flush());
                if let Err(error) = written {
                    sink(
                        generation,
                        Event::WriteFailed {
                            error: error.to_string(),
                        },
                    );
                    return;
                }
            }
            // The sender is gone: dropping stdin here is what closes SOLAR's input.
        })
        .map(drop)
}

fn spawn_output_reader(
    stdout: impl io::Read + Send + 'static,
    generation: u64,
    sink: Sink,
) -> io::Result<()> {
    thread::Builder::new()
        .name("zenith-solar-output".to_owned())
        .spawn(move || {
            let mut reader = BufReader::with_capacity(64 * 1024, stdout);
            let mut buffer = Vec::new();
            loop {
                let event =
                    match read_capped_line(&mut reader, RESPONSE_LINE_LIMIT, 256, &mut buffer) {
                        Ok(Read::Line(bytes)) => {
                            let (text, invalid_utf8) = decode(bytes);
                            Event::Line {
                                text,
                                invalid_utf8,
                                at: Instant::now(),
                            }
                        }
                        Ok(Read::TooLong { bytes, head }) => Event::LineTooLong {
                            bytes,
                            id: id_near_start(&head),
                            at: Instant::now(),
                        },
                        Ok(Read::End) => {
                            sink(
                                generation,
                                Event::OutputClosed {
                                    error: None,
                                    at: Instant::now(),
                                },
                            );
                            return;
                        }
                        Err(error) => {
                            sink(
                                generation,
                                Event::OutputClosed {
                                    error: Some(error.to_string()),
                                    at: Instant::now(),
                                },
                            );
                            return;
                        }
                    };
                if !sink(generation, event) {
                    return;
                }
            }
        })
        .map(drop)
}

fn spawn_stderr_reader(
    stderr: impl io::Read + Send + 'static,
    generation: u64,
    sink: Sink,
) -> io::Result<()> {
    thread::Builder::new()
        .name("zenith-solar-stderr".to_owned())
        .spawn(move || {
            let mut reader = BufReader::with_capacity(16 * 1024, stderr);
            let mut buffer = Vec::new();
            loop {
                let (bytes, cut) = match read_capped_line(
                    &mut reader,
                    STDERR_LINE_LIMIT,
                    STDERR_LINE_LIMIT,
                    &mut buffer,
                ) {
                    Ok(Read::Line(bytes)) => (bytes, 0),
                    Ok(Read::TooLong { bytes, head }) => {
                        let cut = bytes - head.len();
                        (head, cut)
                    }
                    Ok(Read::End) | Err(_) => {
                        sink(generation, Event::StderrClosed);
                        return;
                    }
                };
                let (text, _) = decode(bytes);
                let event = Event::Stderr {
                    text,
                    cut,
                    at: Instant::now(),
                };
                if !sink(generation, event) {
                    return;
                }
            }
        })
        .map(drop)
}

/// What one read of a line produced.
#[derive(Debug, PartialEq, Eq)]
pub enum Read {
    /// A whole line, without its newline.
    Line(Vec<u8>),
    /// A line longer than the limit, read to its end and dropped.
    TooLong {
        /// How many bytes it had, without its newline.
        bytes: usize,
        /// Its first bytes, as many as were asked to be kept.
        head: Vec<u8>,
    },
    /// The end of the stream, with nothing read.
    End,
}

/// Reads one line, keeping at most `limit` bytes of it. A longer line is consumed to its
/// newline without being kept, except for its first `keep` bytes, so that memory never
/// holds more than the limit whatever arrives.
///
/// # Errors
///
/// What the reader returns, except an interruption, which is retried.
pub fn read_capped_line<R: BufRead>(
    reader: &mut R,
    limit: usize,
    keep: usize,
    buffer: &mut Vec<u8>,
) -> io::Result<Read> {
    buffer.clear();
    let mut total = 0_usize;
    let mut head: Option<Vec<u8>> = None;
    loop {
        let available = match reader.fill_buf() {
            Ok(available) => available,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if available.is_empty() {
            return Ok(match head {
                Some(head) => Read::TooLong { bytes: total, head },
                None if total == 0 => Read::End,
                None => Read::Line(strip_carriage_return(std::mem::take(buffer))),
            });
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let chunk = &available[..newline.unwrap_or(available.len())];
        match &mut head {
            Some(head) => {
                let room = keep.saturating_sub(head.len());
                head.extend_from_slice(&chunk[..room.min(chunk.len())]);
            }
            None if buffer.len() + chunk.len() > limit => {
                let mut kept = std::mem::take(buffer);
                kept.truncate(keep);
                let room = keep.saturating_sub(kept.len());
                kept.extend_from_slice(&chunk[..room.min(chunk.len())]);
                head = Some(kept);
            }
            None => buffer.extend_from_slice(chunk),
        }
        total += chunk.len();
        let consumed = chunk.len() + usize::from(newline.is_some());
        reader.consume(consumed);
        if newline.is_some() {
            return Ok(match head {
                Some(head) => Read::TooLong { bytes: total, head },
                None => Read::Line(strip_carriage_return(std::mem::take(buffer))),
            });
        }
    }
}

fn strip_carriage_return(mut line: Vec<u8>) -> Vec<u8> {
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    line
}

fn decode(bytes: Vec<u8>) -> (String, bool) {
    match String::from_utf8(bytes) {
        Ok(text) => (text, false),
        Err(error) => (String::from_utf8_lossy(error.as_bytes()).into_owned(), true),
    }
}

/// The `id` of a response, read from the first bytes of a line too long to parse. SOLAR
/// writes `jsonrpc` and then `id` first, so the id is always near the start.
#[must_use]
pub fn id_near_start(head: &[u8]) -> Option<Id> {
    let text = String::from_utf8_lossy(head);
    let after = &text[text.find("\"id\":")? + 5..];
    let after = after.trim_start();
    if let Some(rest) = after.strip_prefix('"') {
        let end = rest.find('"')?;
        return Some(Id::String(rest[..end].to_owned()));
    }
    if after.starts_with("null") {
        return Some(Id::Null);
    }
    let digits: String = after
        .chars()
        .take_while(|character| character.is_ascii_digit() || *character == '-')
        .collect();
    serde_json::from_str::<serde_json::Number>(&digits)
        .ok()
        .map(Id::Number)
}

/// The exit status in words: `exited with code 1`, `was killed by signal 9 (SIGKILL)`.
#[must_use]
pub fn describe_exit(status: ExitStatus) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            let name = match signal {
                1 => " (SIGHUP)",
                2 => " (SIGINT)",
                3 => " (SIGQUIT)",
                6 => " (SIGABRT)",
                9 => " (SIGKILL)",
                11 => " (SIGSEGV)",
                13 => " (SIGPIPE)",
                15 => " (SIGTERM)",
                _ => "",
            };
            return format!("was killed by signal {signal}{name}");
        }
    }
    match status.code() {
        // A Windows status such as 0xC0000005 reads as a large negative number; the hex
        // form is the one its documentation uses.
        Some(code) if code < 0 => {
            format!("exited with code {code} (0x{:08X})", code.cast_unsigned())
        }
        Some(code) => format!("exited with code {code}"),
        None => "exited".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn lines(input: &[u8], limit: usize, keep: usize) -> Vec<Read> {
        let mut reader = BufReader::with_capacity(4, Cursor::new(input.to_vec()));
        let mut buffer = Vec::new();
        let mut out = Vec::new();
        loop {
            let read = read_capped_line(&mut reader, limit, keep, &mut buffer).unwrap();
            if read == Read::End {
                return out;
            }
            out.push(read);
        }
    }

    #[test]
    fn lines_are_read_whole_across_small_buffers_and_lose_their_carriage_return() {
        assert_eq!(
            lines(b"first line\r\nsecond\nlast", 100, 10),
            vec![
                Read::Line(b"first line".to_vec()),
                Read::Line(b"second".to_vec()),
                Read::Line(b"last".to_vec()),
            ]
        );
    }

    #[test]
    fn a_line_over_the_limit_is_dropped_with_its_length_and_the_next_one_is_read() {
        assert_eq!(
            lines(b"0123456789abcdef\nok\n", 8, 3),
            vec![
                Read::TooLong {
                    bytes: 16,
                    head: b"012".to_vec()
                },
                Read::Line(b"ok".to_vec()),
            ]
        );
    }

    #[test]
    fn a_line_exactly_at_the_limit_is_kept() {
        assert_eq!(
            lines(b"12345678\n", 8, 3),
            vec![Read::Line(b"12345678".to_vec())]
        );
    }

    #[test]
    fn the_id_of_a_line_too_long_to_parse_is_found_near_its_start() {
        assert_eq!(
            id_near_start(br#"{"jsonrpc":"2.0","id":42,"result":{"data":"#),
            Some(Id::Number(42.into()))
        );
        assert_eq!(
            id_near_start(br#"{"jsonrpc":"2.0","id":"a","#),
            Some(Id::String("a".to_owned()))
        );
        assert_eq!(
            id_near_start(br#"{"jsonrpc":"2.0","id":null,"#),
            Some(Id::Null)
        );
        assert_eq!(id_near_start(b"garbage"), None);
    }

    #[test]
    fn bytes_that_are_not_utf8_are_replaced_and_flagged() {
        let (text, invalid) = decode(vec![b'a', 0xff, b'b']);
        assert_eq!(text, "a\u{fffd}b");
        assert!(invalid);
    }
}
