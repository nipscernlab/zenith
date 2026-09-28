//! The `zenith` binary itself, in a pseudo-terminal, against ZENITH's test double: what only
//! the program shows, which the other tests cannot, since they drive the loop without a
//! terminal. The loop runs until it is told to quit, the terminal is given back, and
//! `ZENITH_TRACE_TIMINGS` gets the timings `cargo xtask perf` reads.
//!
//! On Unix only: there the pseudo-terminal passes the program's escape sequences through as
//! it wrote them, which is what the test reads. Windows' `ConPTY` redraws them its own way,
//! and `cargo xtask walkthrough` runs the binary there.

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test reports failure by panicking"
)]

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use serde_json::Value;

const PATIENCE: Duration = Duration::from_secs(30);

/// ZENITH's test double, which `zenith-client` builds beside this test's executable.
fn double() -> PathBuf {
    let test = std::env::current_exe().unwrap();
    let profile = test.parent().and_then(std::path::Path::parent).unwrap();
    let path = profile.join("zenith-solar-double");
    assert!(
        path.is_file(),
        "{} is not built; run the tests of the whole workspace",
        path.display()
    );
    path
}

fn position(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn last_position(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .rposition(|window| window == needle)
}

fn screen(bytes: &[u8]) -> String {
    let mut parser = vt100::Parser::new(24, 80, 0);
    parser.process(bytes);
    parser.screen().contents()
}

#[test]
fn the_program_runs_until_it_quits_gives_the_terminal_back_and_writes_its_timings() {
    let scratch = std::env::temp_dir().join(format!("zenith-binary-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();
    let timings = scratch.join("timings.ndjson");

    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_zenith"));
    command.args(["--solar", double().to_str().unwrap()]);
    command.env("ZENITH_DATA_DIR", &scratch);
    command.env("ZENITH_TRACE_TIMINGS", &timings);
    command.env("TERM", "xterm-256color");
    let mut child = pair.slave.spawn_command(command).unwrap();
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().unwrap();
    let mut writer = pair.master.take_writer().unwrap();
    let output = Arc::new(Mutex::new(Vec::new()));
    let collecting = Arc::clone(&output);
    thread::spawn(move || {
        let mut buffer = vec![0_u8; 64 * 1024];
        while let Ok(read) = reader.read(&mut buffer) {
            if read == 0 {
                break;
            }
            collecting
                .lock()
                .unwrap()
                .extend_from_slice(&buffer[..read]);
        }
    });
    let wait_for = |what: &str, condition: &dyn Fn(&[u8]) -> bool| {
        let deadline = Instant::now() + PATIENCE;
        loop {
            if condition(&output.lock().unwrap()) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "gave up waiting for {what}; the screen was:\n{}",
                screen(&output.lock().unwrap())
            );
            thread::sleep(Duration::from_millis(20));
        }
    };

    // Connected: the loop runs, draws, and reads what the double answers.
    wait_for("the connection", &|bytes| {
        screen(bytes).contains("in orbit")
    });
    // Ctrl+C twice quits.
    writer.write_all(b"\x03").unwrap();
    writer.flush().unwrap();
    wait_for("the offer to quit", &|bytes| {
        screen(bytes).contains("Press Ctrl+C again to quit")
    });
    writer.write_all(b"\x03").unwrap();
    writer.flush().unwrap();
    let deadline = Instant::now() + PATIENCE;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "ZENITH did not quit");
        thread::sleep(Duration::from_millis(20));
    };
    assert!(status.success(), "{status:?}");

    // The terminal is given back: the alternate screen left after it was entered, and the
    // cursor shown again after that.
    thread::sleep(Duration::from_millis(100));
    let bytes = output.lock().unwrap().clone();
    let entered = position(&bytes, b"\x1b[?1049h").expect("the alternate screen was entered");
    let left = last_position(&bytes, b"\x1b[?1049l").expect("the alternate screen was left");
    assert!(left > entered);
    assert!(
        last_position(&bytes, b"\x1b[?25h").is_some_and(|shown| shown > entered),
        "the cursor was not shown again"
    );

    // The timings: one line an event, from the start of the loop to its end.
    let text = std::fs::read_to_string(&timings).unwrap();
    let lines: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let events: Vec<&str> = lines
        .iter()
        .map(|line| line["event"].as_str().unwrap())
        .collect();
    let count = |event: &str| events.iter().filter(|name| **name == event).count();
    assert_eq!(events.first(), Some(&"loop_started"), "{events:?}");
    assert_eq!(count("first_frame"), 1, "{events:?}");
    assert_eq!(count("connected_frame"), 1, "{events:?}");
    assert_eq!(count("solar_started"), 1, "{events:?}");
    assert_eq!(count("key"), 2, "{events:?}");
    let exit = lines.last().unwrap();
    assert_eq!(exit["event"], "exit", "{events:?}");
    let wakeups = exit["wakeups"].as_u64().unwrap();
    let frames = exit["frames"].as_u64().unwrap();
    assert_eq!(usize::try_from(wakeups).unwrap(), count("wake"));
    // A frame before every wait and one after the last, and one more for each key.
    assert_eq!(frames, wakeups + 1 + 2, "{events:?}");
    let _ = std::fs::remove_dir_all(&scratch);
}

/// The binary in a pseudo-terminal of 80 × 24, against the double, and what it wrote.
struct Running {
    child: Box<dyn portable_pty::Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
    parser: Arc<Mutex<vt100::Parser>>,
    scratch: PathBuf,
}

impl Running {
    fn start(name: &str) -> Self {
        let scratch = std::env::temp_dir().join(format!("zenith-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(&scratch).unwrap();
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_zenith"));
        command.args(["--solar", double().to_str().unwrap()]);
        command.env("ZENITH_DATA_DIR", &scratch);
        command.env("TERM", "xterm-256color");
        let child = pair.slave.spawn_command(command).unwrap();
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().unwrap();
        let writer = pair.master.take_writer().unwrap();
        let parser = Arc::new(Mutex::new(vt100::Parser::new(24, 80, 0)));
        let feeding = Arc::clone(&parser);
        thread::spawn(move || {
            let _master = pair.master;
            let mut buffer = vec![0_u8; 64 * 1024];
            while let Ok(read) = reader.read(&mut buffer) {
                if read == 0 {
                    break;
                }
                feeding.lock().unwrap().process(&buffer[..read]);
            }
        });
        Self {
            child,
            writer,
            parser,
            scratch,
        }
    }

    fn screen(&self) -> String {
        self.parser.lock().unwrap().screen().contents()
    }

    fn mouse(&self) -> (vt100::MouseProtocolMode, vt100::MouseProtocolEncoding) {
        let parser = self.parser.lock().unwrap();
        let screen = parser.screen();
        (
            screen.mouse_protocol_mode(),
            screen.mouse_protocol_encoding(),
        )
    }

    fn wait_until(&self, what: &str, condition: impl Fn(&Self) -> bool) {
        let deadline = Instant::now() + PATIENCE;
        while !condition(self) {
            assert!(
                Instant::now() < deadline,
                "gave up waiting for {what}; the screen was:\n{}",
                self.screen()
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn write(&mut self, bytes: &[u8]) {
        self.writer.write_all(bytes).unwrap();
        self.writer.flush().unwrap();
    }

    /// A notch of the wheel at a cell, counted from 1, as a terminal reports it in SGR.
    fn wheel(&mut self, down: bool, column: u16, row: u16) {
        let button = if down { 65 } else { 64 };
        self.write(format!("\x1b[<{button};{column};{row}M").as_bytes());
        thread::sleep(Duration::from_millis(30));
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = std::fs::remove_dir_all(&self.scratch);
    }
}

#[test]
fn the_wheel_scrolls_the_real_program_and_the_mouse_is_given_back_when_asked_and_at_the_end() {
    use vt100::{MouseProtocolEncoding, MouseProtocolMode};
    let mut zenith = Running::start("wheel");
    zenith.wait_until("the connection", |zenith| {
        zenith.screen().contains("in orbit")
    });
    // ZENITH asked for the buttons, the wheel's among them, in SGR, and no movement.
    zenith.wait_until("the mouse", |zenith| {
        zenith.mouse() == (MouseProtocolMode::PressRelease, MouseProtocolEncoding::Sgr)
    });
    // The help, scrolled down past its first heading by the wheel and back up.
    zenith.write(b"?");
    zenith.wait_until("the help", |zenith| zenith.screen().contains("Everywhere"));
    for _ in 0..4 {
        zenith.wheel(true, 40, 10);
    }
    zenith.wait_until("the help to scroll", |zenith| {
        !zenith.screen().contains("Everywhere")
    });
    for _ in 0..4 {
        zenith.wheel(false, 40, 10);
    }
    zenith.wait_until("the help to scroll back", |zenith| {
        zenith.screen().contains("Everywhere")
    });
    zenith.write(b"\x1b");
    zenith.wait_until("the help to close", |zenith| {
        !zenith.screen().contains("Everywhere")
    });
    // /mouse gives it to the terminal and takes it back.
    zenith.write(b"/mouse off\r");
    zenith.wait_until("the mouse to be given back", |zenith| {
        zenith.mouse().0 == MouseProtocolMode::None
    });
    zenith.write(b"/mouse on\r");
    zenith.wait_until("the mouse to be taken again", |zenith| {
        zenith.mouse().0 == MouseProtocolMode::PressRelease
    });
    // Quitting gives it back with the rest of the terminal.
    zenith.write(b"\x03");
    zenith.wait_until("the offer to quit", |zenith| {
        zenith.screen().contains("Press Ctrl+C again to quit")
    });
    zenith.write(b"\x03");
    let deadline = Instant::now() + PATIENCE;
    let status = loop {
        if let Some(status) = zenith.child.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "ZENITH did not quit");
        thread::sleep(Duration::from_millis(20));
    };
    assert!(status.success(), "{status:?}");
    zenith.wait_until("the mouse to be given back at the end", |zenith| {
        zenith.mouse().0 == MouseProtocolMode::None
    });
}
