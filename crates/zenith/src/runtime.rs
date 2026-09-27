//! The loop that drives the application: one channel, the effects, the drawing.
//!
//! The loop waits on one channel that the keyboard, SOLAR's two output pipes and the
//! signal handler all feed, and wakes early only for a deadline the application names.
//! It never polls. After every batch of events it draws once, and ratatui writes to the
//! terminal only the cells that changed.
//!
//! The runtime is generic over ratatui's backend, so the tests run this same loop with
//! the test backend, a real SOLAR, and keys sent through [`Runtime::sender`].

use std::collections::{HashMap, VecDeque};
use std::fmt::Write as _;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use ratatui::Terminal;
use ratatui::backend::Backend;
use zenith_client::connection::{Connection, Sink, describe_exit};
use zenith_client::locate::{Placement, Search, locate, prepare};

use crate::app::history::Recorded;
use crate::app::link::Failure;
use crate::app::{App, Effect, Incoming, Options, StartSteps, Started, Written, report};
use crate::clock::Utc;
use crate::command_history;

/// How many events may wait in the channel. When the interface falls behind, the threads
/// that read SOLAR wait, and then SOLAR waits on its pipe: nothing piles up between.
pub const CHANNEL_CAPACITY: usize = 1024;

/// How long a stopped SOLAR is given to exit on its own before it is killed.
pub const STOP_GRACE: Duration = Duration::from_millis(500);

/// The most events handled before the screen is drawn again, so that a flood of lines
/// from SOLAR cannot keep the screen from updating.
const BATCH: usize = 256;

type Connections = Arc<Mutex<HashMap<u64, Connection>>>;

/// What the loop measured, when `ZENITH_TRACE_TIMINGS` names a file.
#[derive(Debug)]
pub struct Timings {
    out: BufWriter<File>,
    process_start: Instant,
    wakeups: u64,
    frames: u64,
    connected_drawn: bool,
}

impl Timings {
    /// Opens the file named by `ZENITH_TRACE_TIMINGS`, if the variable is set.
    #[must_use]
    pub fn from_environment(process_start: Instant) -> Option<Self> {
        let path = std::env::var_os("ZENITH_TRACE_TIMINGS")?;
        let file = File::create(path).ok()?;
        Some(Self {
            out: BufWriter::new(file),
            process_start,
            wakeups: 0,
            frames: 0,
            connected_drawn: false,
        })
    }

    fn mark(&mut self, event: &str, extra: &[(&str, u128)]) {
        let mut line = format!(
            "{{\"event\":\"{event}\",\"us\":{}",
            self.process_start.elapsed().as_micros()
        );
        for (key, value) in extra {
            let _ = write!(line, ",\"{key}\":{value}");
        }
        line.push('}');
        let _ = writeln!(self.out, "{line}");
    }
}

/// Writes the command line history on a thread of its own, so that the loop never waits
/// for the disk. When several snapshots are waiting, only the newest is written.
#[derive(Debug)]
struct HistoryWriter {
    snapshots: Option<mpsc::Sender<(PathBuf, Vec<String>)>>,
    thread: Option<thread::JoinHandle<()>>,
    report: SyncSender<Incoming>,
}

impl HistoryWriter {
    /// The thread, which says through `report` when a write fails.
    fn start(report: SyncSender<Incoming>) -> Self {
        let (snapshots, waiting) = mpsc::channel::<(PathBuf, Vec<String>)>();
        let failures = report.clone();
        let thread = thread::Builder::new()
            .name("zenith-history".to_owned())
            .spawn(move || {
                while let Ok(mut snapshot) = waiting.recv() {
                    while let Ok(newer) = waiting.try_recv() {
                        snapshot = newer;
                    }
                    let (file, lines) = snapshot;
                    if let Err(why) = write_history(&file, &lines) {
                        let _ = failures.try_send(history_failed(why));
                    }
                }
            })
            .ok();
        Self {
            snapshots: thread.as_ref().map(|_| snapshots),
            thread,
            report,
        }
    }

    /// Writes `lines` to `file`, on the thread, or here when there is no thread.
    fn save(&self, file: PathBuf, lines: Vec<String>) {
        match &self.snapshots {
            Some(snapshots) => {
                let _ = snapshots.send((file, lines));
            }
            None => {
                if let Err(why) = write_history(&file, &lines) {
                    let _ = self.report.try_send(history_failed(why));
                }
            }
        }
    }

    /// Writes what is still waiting, and ends the thread.
    fn finish(&mut self) {
        drop(self.snapshots.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The history written to its file, or why it was not, as a sentence.
fn write_history(file: &Path, lines: &[String]) -> Result<(), String> {
    command_history::write(file, lines).map_err(|error| format!("{}: {error}.", file.display()))
}

/// The event that says the history could not be written, and why.
fn history_failed(why: String) -> Incoming {
    Incoming::Wrote {
        what: Written::CommandHistory,
        result: Err(why),
    }
}

/// The loop, its terminal, and the connections it holds.
#[derive(Debug)]
pub struct Runtime<B: Backend> {
    terminal: Terminal<B>,
    app: App,
    sender: SyncSender<Incoming>,
    receiver: Receiver<Incoming>,
    connections: Connections,
    queue: VecDeque<Incoming>,
    timings: Option<Timings>,
    history: HistoryWriter,
}

impl<B: Backend> Runtime<B> {
    /// The application, started, with its first effects done.
    pub fn new(terminal: Terminal<B>, options: Options, timings: Option<Timings>) -> Self {
        let (sender, receiver) = mpsc::sync_channel(CHANNEL_CAPACITY);
        let history = HistoryWriter::start(sender.clone());
        let (app, effects) = App::new(options, Instant::now(), SystemTime::now());
        let mut runtime = Self {
            terminal,
            app,
            sender,
            receiver,
            connections: Arc::new(Mutex::new(HashMap::new())),
            queue: VecDeque::new(),
            timings,
            history,
        };
        if let Ok(size) = runtime.terminal.size() {
            runtime.queue.push_back(Incoming::Resize {
                width: size.width,
                height: size.height,
            });
        }
        runtime.execute(effects);
        runtime
    }

    /// Where events are sent from other threads, and from the tests.
    #[must_use]
    pub fn sender(&self) -> SyncSender<Incoming> {
        self.sender.clone()
    }

    /// The application, to read.
    #[must_use]
    pub fn app(&self) -> &App {
        &self.app
    }

    /// The terminal, to read what was drawn.
    #[must_use]
    pub fn terminal(&self) -> &Terminal<B> {
        &self.terminal
    }

    /// Runs until the application quits.
    ///
    /// # Errors
    ///
    /// What drawing to the terminal returns.
    pub fn run(&mut self) -> io::Result<()> {
        if let Some(timings) = &mut self.timings {
            timings.mark("loop_started", &[]);
        }
        loop {
            self.draw()?;
            if self.app.quitting {
                break;
            }
            self.wait(None)?;
        }
        self.finish();
        Ok(())
    }

    /// Handles events for at most `limit`, and draws. The tests drive the loop with it.
    ///
    /// # Errors
    ///
    /// What drawing returns.
    pub fn step(&mut self, limit: Duration) -> io::Result<()> {
        self.wait(Some(limit))?;
        self.draw()
    }

    fn wait(&mut self, limit: Option<Duration>) -> io::Result<()> {
        let now = Instant::now();
        let deadline = self.app.next_deadline();
        let mut timeout = deadline.map(|deadline| deadline.saturating_duration_since(now));
        if let Some(limit) = limit {
            timeout = Some(timeout.map_or(limit, |timeout| timeout.min(limit)));
        }
        if self.queue.is_empty() {
            let first = match timeout {
                Some(timeout) => match self.receiver.recv_timeout(timeout) {
                    Ok(incoming) => incoming,
                    Err(RecvTimeoutError::Timeout) => Incoming::Tick,
                    Err(RecvTimeoutError::Disconnected) => Incoming::Terminate,
                },
                None => self.receiver.recv().unwrap_or(Incoming::Terminate),
            };
            self.queue.push_back(first);
        }
        if let Some(timings) = &mut self.timings {
            timings.wakeups += 1;
            timings.mark("wake", &[]);
        }
        let mut handled = 0;
        while handled < BATCH {
            let Some(incoming) = self
                .queue
                .pop_front()
                .or_else(|| self.receiver.try_recv().ok())
            else {
                break;
            };
            let key = matches!(incoming, Incoming::Key(_));
            if let (
                Some(timings),
                Incoming::Started {
                    result: Ok(started),
                    ..
                },
            ) = (&mut self.timings, &incoming)
            {
                let steps = started.steps;
                timings.mark(
                    "solar_started",
                    &[
                        ("locate_us", steps.locate.as_micros()),
                        ("prepare_us", steps.prepare.as_micros()),
                        ("spawn_us", steps.spawn.as_micros()),
                    ],
                );
            }
            let started = Instant::now();
            let effects = self.app.handle(incoming, started);
            self.execute(effects);
            handled += 1;
            if key && let Some(timings) = &mut self.timings {
                let handle = started.elapsed().as_micros();
                // The draw is measured with the key, since the redraw is what a person
                // waits for.
                draw_frame(&mut self.terminal, &self.app)?;
                let total = started.elapsed().as_micros();
                timings.frames += 1;
                timings.mark("key", &[("handle_us", handle), ("total_us", total)]);
            }
        }
        Ok(())
    }

    fn draw(&mut self) -> io::Result<()> {
        draw_frame(&mut self.terminal, &self.app)?;
        if let Some(timings) = &mut self.timings {
            timings.frames += 1;
            if timings.frames == 1 {
                timings.mark("first_frame", &[]);
            }
            if !timings.connected_drawn && self.app.link.connected() {
                timings.connected_drawn = true;
                timings.mark("connected_frame", &[]);
            }
        }
        Ok(())
    }

    fn finish(&mut self) {
        self.history.finish();
        if let Some(timings) = &mut self.timings {
            let (wakeups, frames) = (u128::from(timings.wakeups), u128::from(timings.frames));
            timings.mark("exit", &[("wakeups", wakeups), ("frames", frames)]);
            let _ = timings.out.flush();
        }
    }

    fn execute(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Start {
                    generation,
                    flag,
                    settings,
                } => self.start(generation, flag, settings),
                Effect::Write { generation, line } => {
                    let result =
                        self.connections.lock().ok().and_then(|connections| {
                            connections.get(&generation).map(|c| c.send(line))
                        });
                    if let Some(Err(error)) = result {
                        self.queue.push_back(Incoming::Connection {
                            generation,
                            event: zenith_client::connection::Event::WriteFailed {
                                error: error.to_string(),
                            },
                        });
                    }
                }
                Effect::Stop { generation } => self.stop(generation),
                Effect::CheckExit { generation } => self.check_exit(generation),
                Effect::Export { path } => {
                    let (recorded, result) = self.write_recording(path);
                    self.queue.push_back(Incoming::Wrote {
                        what: Written::Recording(recorded),
                        result,
                    });
                }
                Effect::Report { path, system, why } => {
                    let result = self.write_report(path, system.as_ref(), why.as_deref());
                    self.queue.push_back(Incoming::Wrote {
                        what: Written::Report,
                        result,
                    });
                }
                Effect::SaveHistory { file, lines } => self.history.save(file, lines),
                Effect::Repaint => {
                    let _ = self.terminal.clear();
                }
                Effect::Quit => {}
            }
        }
    }

    /// Finds, copies and starts SOLAR on a thread of its own, because hashing and copying
    /// the binary and starting its process can take a while, hundreds of milliseconds for
    /// the first start of a run on Windows, and the opening must keep drawing meanwhile.
    fn start(
        &mut self,
        generation: u64,
        flag: Option<PathBuf>,
        settings: zenith_client::connection::Settings,
    ) {
        let sender = self.sender.clone();
        let connections = Arc::clone(&self.connections);
        let spawned = thread::Builder::new()
            .name("zenith-start".to_owned())
            .spawn(move || {
                let result = start_solar(generation, flag, &settings, &sender, &connections);
                let _ = sender.send(Incoming::Started { generation, result });
            });
        if let Err(error) = spawned {
            self.queue.push_back(Incoming::Started {
                generation,
                result: Err(Failure::Start {
                    message: format!("ZENITH could not start a thread to start SOLAR: {error}."),
                    path: PathBuf::from("solar"),
                    permission: false,
                }),
            });
        }
    }

    fn stop(&mut self, generation: u64) {
        let connection = self
            .connections
            .lock()
            .ok()
            .and_then(|mut connections| connections.remove(&generation));
        let Some(connection) = connection else {
            return;
        };
        if self.app.quitting {
            let _status = connection.shutdown(STOP_GRACE);
        } else {
            let _ = thread::Builder::new()
                .name("zenith-stop".to_owned())
                .spawn(move || {
                    let _status = connection.shutdown(STOP_GRACE);
                });
        }
    }

    fn check_exit(&mut self, generation: u64) {
        let status = self.connections.lock().ok().and_then(|mut connections| {
            connections
                .get_mut(&generation)
                .map(zenith_client::connection::Connection::try_exit)
        });
        match status {
            Some(Ok(Some(status))) => self.queue.push_back(Incoming::Exited {
                generation,
                how: describe_exit(status),
            }),
            Some(Ok(None)) => self.queue.push_back(Incoming::StillRunning { generation }),
            Some(Err(error)) => self.queue.push_back(Incoming::Exited {
                generation,
                how: format!("could not be asked how it exited: {error}"),
            }),
            None => {}
        }
    }

    /// Writes the recording of the current connection, and says what it holds.
    fn write_recording(
        &self,
        path: Option<PathBuf>,
    ) -> (Recorded, Result<(PathBuf, usize), String>) {
        let (file, path) = match create(path, "zenith-recording", SystemTime::now()) {
            Ok(created) => created,
            Err(error) => return (Recorded::default(), Err(error.to_string())),
        };
        let mut out = BufWriter::new(file);
        match self
            .app
            .history
            .record(&mut out, self.app.link.generation, SystemTime::now())
            .and_then(|recorded| out.flush().map(|()| recorded))
        {
            Ok(recorded) => (recorded, Ok((path, recorded.calls))),
            Err(error) => (
                Recorded::default(),
                Err(format!("{}: {error}", path.display())),
            ),
        }
    }

    fn write_report(
        &self,
        path: Option<PathBuf>,
        system: Option<&serde_json::Value>,
        why: Option<&str>,
    ) -> Result<(PathBuf, usize), String> {
        let (file, path) =
            create(path, "zenith-report", SystemTime::now()).map_err(|error| error.to_string())?;
        let mut out = BufWriter::new(file);
        report::write(
            &self.app,
            &mut out,
            system,
            why,
            SystemTime::now(),
            |name| std::env::var(name).ok(),
        )
        .and_then(|count| out.flush().map(|()| count))
        .map(|count| (path.clone(), count))
        .map_err(|error| format!("{}: {error}", path.display()))
    }
}

/// Draws one frame, whatever the backend's own error type is.
fn draw_frame<B: Backend>(terminal: &mut Terminal<B>, app: &App) -> io::Result<()> {
    terminal
        .draw(|frame| crate::ui::draw(frame, app))
        .map(drop)
        .map_err(|error| io::Error::other(error.to_string()))
}

/// Creates a file that did not exist: the path given, or a name with the time in the
/// current directory, with `-2`, `-3` and so on when that name is taken. ZENITH never
/// overwrites a file.
fn create(path: Option<PathBuf>, stem: &str, now: SystemTime) -> io::Result<(File, PathBuf)> {
    let open = |path: &Path| OpenOptions::new().write(true).create_new(true).open(path);
    if let Some(path) = path {
        return open(&path)
            .map(|file| (file, path.clone()))
            .map_err(|error| io::Error::new(error.kind(), format!("{}: {error}", path.display())));
    }
    let stamp = Utc::of(now).file_stamp();
    for attempt in 1..1000 {
        let name = if attempt == 1 {
            format!("{stem}-{stamp}.ndjson")
        } else {
            format!("{stem}-{stamp}-{attempt}.ndjson")
        };
        let path = PathBuf::from(name);
        match open(&path) {
            Ok(file) => return Ok((file, path)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::other("every name for the file is taken"))
}

fn start_solar(
    generation: u64,
    flag: Option<PathBuf>,
    settings: &zenith_client::connection::Settings,
    sender: &SyncSender<Incoming>,
    connections: &Connections,
) -> Result<Started, Failure> {
    let began = Instant::now();
    let found = locate(&Search::from_environment(flag)).map_err(Failure::Locate)?;
    let located = Instant::now();
    let prepared =
        prepare(found, &Placement::for_this_system()).map_err(|error| Failure::Copy {
            message: error.to_string(),
        })?;
    let ready = Instant::now();
    let sink_sender = sender.clone();
    let sink: Sink = Arc::new(move |generation, event| {
        sink_sender
            .send(Incoming::Connection { generation, event })
            .is_ok()
    });
    let connection =
        Connection::start(prepared.clone(), settings, generation, sink).map_err(|error| {
            Failure::Start {
                permission: error.error.kind() == io::ErrorKind::PermissionDenied,
                path: error.path.clone(),
                message: error.to_string(),
            }
        })?;
    let steps = StartSteps {
        locate: located - began,
        prepare: ready - located,
        spawn: ready.elapsed(),
    };
    let pid = connection.id();
    if let Ok(mut connections) = connections.lock() {
        connections.insert(generation, connection);
    }
    Ok(Started {
        prepared,
        pid,
        steps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_never_overwritten_and_a_taken_name_gets_a_number() {
        let directory = std::env::temp_dir().join(format!("zenith-create-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let given = directory.join("mine.ndjson");
        std::fs::write(&given, b"keep me").unwrap();
        assert!(create(Some(given.clone()), "x", SystemTime::UNIX_EPOCH).is_err());
        assert_eq!(std::fs::read(&given).unwrap(), b"keep me");
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn the_history_is_on_disk_once_the_writer_has_finished() {
        let directory = std::env::temp_dir().join(format!("zenith-writer-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        let file = directory.join(command_history::FILE_NAME);
        let (sender, receiver) = mpsc::sync_channel(8);
        let mut writer = HistoryWriter::start(sender);
        writer.save(file.clone(), vec!["/list".to_owned()]);
        writer.save(file.clone(), vec!["/list".to_owned(), "/ping".to_owned()]);
        writer.finish();
        assert_eq!(
            command_history::read(&file),
            command_history::Loaded::Lines {
                lines: vec!["/list".to_owned(), "/ping".to_owned()],
                skipped: 0
            }
        );
        assert!(receiver.try_recv().is_err(), "no write failed");
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_history_that_cannot_be_written_is_reported_to_the_loop() {
        let directory =
            std::env::temp_dir().join(format!("zenith-writer-fails-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        // A file where the history's directory would have to be.
        let blocker = directory.join("a-file");
        std::fs::write(&blocker, b"").unwrap();
        let (sender, receiver) = mpsc::sync_channel(8);
        let mut writer = HistoryWriter::start(sender);
        writer.save(
            blocker.join(command_history::FILE_NAME),
            vec!["/list".to_owned()],
        );
        writer.finish();
        let Ok(Incoming::Wrote {
            what: Written::CommandHistory,
            result: Err(why),
        }) = receiver.try_recv()
        else {
            panic!("the failure was not reported");
        };
        assert!(why.contains("a-file"), "{why}");
        let _ = std::fs::remove_dir_all(&directory);
    }
}
