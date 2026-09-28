//! The real `zenith` binary in a pseudo-terminal: keys written into it, the screen read
//! back through a terminal emulator, and its processor time and memory read from the
//! system.
//!
//! On Windows the pseudo-terminal is `ConPTY`, on macOS and Linux a Unix one, both through
//! `portable-pty`. This is the harness of `cargo xtask perf` and `cargo xtask soak`, which
//! measure the program people run rather than a library inside a test, and of
//! `cargo xtask walkthrough`, which does what `docs/TESTING_BY_HAND.md` tells a person to.
//!
//! A harness may poll, and this one does, every two milliseconds, while it waits for
//! something to appear on the screen. ZENITH itself never polls.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// A running `zenith` in a pseudo-terminal.
pub(crate) struct Session {
    child: Box<dyn Child + Send + Sync>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    parser: Arc<Mutex<vt100::Parser>>,
    started: Instant,
    system: System,
    // Kept for resizing, and alive: dropping the master closes the pseudo-terminal.
    master: Box<dyn MasterPty + Send>,
}

impl Session {
    /// Starts `program` with `arguments` and `environment` in a terminal of this size.
    ///
    /// # Errors
    ///
    /// Why the pseudo-terminal or the program could not be started.
    pub(crate) fn start(
        program: &Path,
        arguments: &[&str],
        environment: &[(&str, &str)],
        columns: u16,
        rows: u16,
    ) -> Result<Self, String> {
        let system = native_pty_system();
        let pair = system
            .openpty(PtySize {
                rows,
                cols: columns,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("the pseudo-terminal could not be opened: {error}"))?;
        let mut command = CommandBuilder::new(program);
        command.args(arguments);
        for (key, value) in environment {
            command.env(key, value);
        }
        if let Ok(directory) = std::env::current_dir() {
            command.cwd(directory);
        }
        let started = Instant::now();
        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| format!("{} could not be started: {error}", program.display()))?;
        drop(pair.slave);
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| format!("the terminal could not be read: {error}"))?;
        let writer =
            Arc::new(Mutex::new(pair.master.take_writer().map_err(|error| {
                format!("the terminal could not be written: {error}")
            })?));
        let answering = Arc::clone(&writer);
        let parser = Arc::new(Mutex::new(vt100::Parser::new(rows, columns, 0)));
        let feeding = Arc::clone(&parser);
        let raw_log = std::env::var_os("XTASK_PTY_LOG");
        // The output must be read all the time: ConPTY stops the program when nobody reads.
        thread::spawn(move || {
            let mut log = raw_log.and_then(|path| std::fs::File::create(path).ok());
            let mut buffer = vec![0_u8; 64 * 1024];
            while let Ok(read) = reader.read(&mut buffer) {
                if read == 0 {
                    break;
                }
                if let Some(log) = &mut log {
                    let _ = log.write_all(&buffer[..read]);
                }
                // ConPTY asks where the cursor is before it starts the program, and waits
                // for the answer a terminal would give. This harness is that terminal.
                if buffer[..read].windows(4).any(|window| window == b"\x1b[6n")
                    && let Ok(mut writer) = answering.lock()
                {
                    let _ = writer.write_all(b"\x1b[1;1R");
                    let _ = writer.flush();
                }
                if let Ok(mut parser) = feeding.lock() {
                    parser.process(&buffer[..read]);
                }
            }
        });
        Ok(Self {
            child,
            writer,
            parser,
            started,
            system: System::new(),
            master: pair.master,
        })
    }

    /// When the program was started.
    pub(crate) fn started(&self) -> Instant {
        self.started
    }

    /// Writes bytes, as a person typing them.
    ///
    /// # Errors
    ///
    /// What the pseudo-terminal says.
    pub(crate) fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| "the terminal's input was poisoned".to_owned())?;
        writer
            .write_all(bytes)
            .and_then(|()| writer.flush())
            .map_err(|error| format!("the terminal refused input: {error}"))
    }

    /// The characters on the screen.
    pub(crate) fn screen(&self) -> String {
        self.parser
            .lock()
            .map(|parser| parser.screen().contents())
            .unwrap_or_default()
    }

    /// The screen as a person would read it, row by row, for a message that failed.
    pub(crate) fn dump(&self) -> String {
        self.rows()
            .iter()
            .map(|row| row.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Waits until the screen shows `needle`, and says how long that took.
    ///
    /// # Errors
    ///
    /// When it does not appear within `patience`, with the screen as it was.
    pub(crate) fn wait_for(&self, needle: &str, patience: Duration) -> Result<Duration, String> {
        let from = Instant::now();
        loop {
            if self.screen().contains(needle) {
                return Ok(from.elapsed());
            }
            if from.elapsed() > patience {
                return Err(format!(
                    "{needle:?} did not appear within {patience:?}; the screen was:\n{}",
                    self.dump()
                ));
            }
            thread::sleep(Duration::from_millis(2));
        }
    }

    /// Waits until the screen no longer shows `needle`.
    ///
    /// # Errors
    ///
    /// When it is still there after `patience`, with the screen as it was.
    pub(crate) fn wait_until_gone(&self, needle: &str, patience: Duration) -> Result<(), String> {
        let from = Instant::now();
        while self.screen().contains(needle) {
            if from.elapsed() > patience {
                return Err(format!(
                    "{needle:?} was still on the screen after {patience:?}; the screen was:\n{}",
                    self.dump()
                ));
            }
            thread::sleep(Duration::from_millis(2));
        }
        Ok(())
    }

    /// Makes the terminal `columns` by `rows`, as a person resizing the window does.
    ///
    /// # Errors
    ///
    /// What the pseudo-terminal says.
    pub(crate) fn resize(&mut self, columns: u16, rows: u16) -> Result<(), String> {
        self.master
            .resize(PtySize {
                rows,
                cols: columns,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("the terminal could not be resized: {error}"))?;
        if let Ok(mut parser) = self.parser.lock() {
            parser.screen_mut().set_size(rows, columns);
        }
        Ok(())
    }

    /// The background colour of a cell, and whether it is underlined.
    pub(crate) fn cell(&self, row: u16, column: u16) -> Option<(vt100::Color, bool)> {
        let parser = self.parser.lock().ok()?;
        let cell = parser.screen().cell(row, column)?;
        Some((cell.bgcolor(), cell.underline()))
    }

    /// The rows of the screen, as text.
    pub(crate) fn rows(&self) -> Vec<String> {
        self.parser
            .lock()
            .map(|parser| {
                let (_, columns) = parser.screen().size();
                parser.screen().rows(0, columns).collect()
            })
            .unwrap_or_default()
    }

    /// Whether the terminal is as a shell would have it: the main screen, with its cursor
    /// shown and its mouse its own.
    pub(crate) fn restored(&self) -> bool {
        self.parser.lock().is_ok_and(|parser| {
            !parser.screen().alternate_screen()
                && !parser.screen().hide_cursor()
                && parser.screen().mouse_protocol_mode() == vt100::MouseProtocolMode::None
        })
    }

    /// Whether the program has asked the terminal for the mouse and not given it back.
    pub(crate) fn mouse_taken(&self) -> bool {
        self.parser.lock().is_ok_and(|parser| {
            parser.screen().mouse_protocol_mode() != vt100::MouseProtocolMode::None
        })
    }

    /// Waits for the program to exit by itself.
    ///
    /// # Errors
    ///
    /// When it has not exited after `patience`; it is then killed.
    pub(crate) fn exited(&mut self, patience: Duration) -> Result<(), String> {
        let from = Instant::now();
        loop {
            if let Ok(Some(_)) = self.child.try_wait() {
                return Ok(());
            }
            if from.elapsed() > patience {
                let _ = self.child.kill();
                return Err(format!(
                    "zenith did not exit within {patience:?}; it was killed. The screen was:\n{}",
                    self.dump()
                ));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    /// The process id.
    pub(crate) fn pid(&self) -> Option<u32> {
        self.child.process_id()
    }

    /// The processor time the process has used, and its resident memory, in milliseconds
    /// and bytes.
    pub(crate) fn usage(&mut self) -> Option<(u64, u64)> {
        let pid = Pid::from_u32(self.pid()?);
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
        let process = self.system.process(pid)?;
        Some((process.accumulated_cpu_time(), process.memory()))
    }

    /// Asks ZENITH to quit with `Ctrl+C` twice, and waits for it to exit.
    ///
    /// # Errors
    ///
    /// When it does not exit within the patience given.
    pub(crate) fn quit(&mut self, patience: Duration) -> Result<(), String> {
        self.write(b"\x03")?;
        thread::sleep(Duration::from_millis(50));
        self.write(b"\x03")?;
        self.exited(patience)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.kill();
        }
    }
}

/// The `zenith` release binary, built now so that what is measured is the current code.
///
/// # Errors
///
/// When the build fails, which on Windows includes a `zenith.exe` that is running from
/// `target\release` and cannot be replaced.
pub(crate) fn release_binary(root: &Path) -> Result<std::path::PathBuf, String> {
    let status = std::process::Command::new("cargo")
        .args([
            "build",
            "--release",
            "--locked",
            "-p",
            "zenith",
            "--bin",
            "zenith",
        ])
        .current_dir(root)
        .status()
        .map_err(|error| format!("cargo could not be started: {error}"))?;
    if !status.success() {
        return Err(
            "the release build failed; on Windows, close any zenith running from target\\release"
                .to_owned(),
        );
    }
    Ok(root
        .join("target")
        .join("release")
        .join(format!("zenith{}", std::env::consts::EXE_SUFFIX)))
}

/// The `solar` to measure against: `ZENITH_SOLAR`, then the `PATH`, as ZENITH finds it.
///
/// # Errors
///
/// When there is none.
pub(crate) fn solar() -> Result<std::path::PathBuf, String> {
    zenith_client::locate::locate(&zenith_client::locate::Search::from_environment(None))
        .map(|found| found.path)
        .map_err(|error| format!("{error} Set ZENITH_SOLAR to the solar binary."))
}

/// A new, empty directory for `ZENITH_DATA_DIR`, so that what the harness types never
/// reaches the command line history of the person running it.
///
/// # Errors
///
/// When the directory cannot be made.
pub(crate) fn scratch_data_dir(task: &str) -> Result<std::path::PathBuf, String> {
    let directory = std::env::temp_dir().join(format!("zenith-{task}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("{} could not be made: {error}", directory.display()))?;
    Ok(directory)
}

/// A percentile of some measurements, by the nearest rank.
pub(crate) fn percentile(sorted: &[u128], fraction: f64) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "a rank among at most a few thousand measurements"
    )]
    let rank = ((fraction * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len());
    sorted[rank - 1]
}

#[cfg(test)]
mod tests {
    #[test]
    fn percentiles_are_taken_by_the_nearest_rank() {
        let values = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        assert_eq!(super::percentile(&values, 0.5), 5);
        assert_eq!(super::percentile(&values, 0.99), 10);
        assert_eq!(super::percentile(&[], 0.5), 0);
    }
}
