//! `cargo xtask perf`: startup, a key to its redraw, the processor at idle, and the price
//! of starting SOLAR at `trace`, measured on the real binary against the installed SOLAR.
//!
//! | What | How |
//! | ---- | --- |
//! | Startup | From starting the process in a pseudo-terminal to the screen showing `in orbit`, over ten runs; and the moment the process itself drew its first connected frame, from `ZENITH_TRACE_TIMINGS` |
//! | A key to its redraw | For 200 keys typed one at a time, the time from the loop reading the key to the frame being flushed, from `ZENITH_TRACE_TIMINGS`; and for 50 of them, from writing the key into the terminal to seeing it on the screen |
//! | Idle | The processor time over a window after the first minute connected, when the status bar changes once a minute, and how many times the loop woke in it |
//! | The price of `trace` | The round trip of 2 000 `solar.ping` through ZENITH's own connection code, with SOLAR at `trace`, as ZENITH starts it, and at `off`, and the bytes of standard error SOLAR writes a call |
//!
//! The results are printed as a table and written to `target/perf/results.json`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use zenith_client::connection::{Connection, Event as Happened, Settings, Sink};
use zenith_client::locate::{Found, Origin, Placement, prepare};

use crate::pty::{Session, percentile, release_binary, solar};

/// The calls each level of the price of `trace` is measured over, after [`WARM_UP`].
const CALLS: usize = 2_000;

/// The calls sent first and not counted, while caches fill.
const WARM_UP: usize = 100;

/// How long any one thing may take before the measurement gives up.
const PATIENCE: Duration = Duration::from_secs(30);

/// What every measurement needs: the two programs and where the timings go.
struct Setup {
    zenith: PathBuf,
    solar: PathBuf,
    out: PathBuf,
}

impl Setup {
    /// ZENITH started in a pseudo-terminal of 100 × 30, recording its timings in `name`,
    /// and connected.
    fn connected(&self, name: &str) -> Result<(Session, PathBuf), String> {
        let timings = self.out.join(name);
        let timings_text = timings.display().to_string();
        let solar = self.solar.display().to_string();
        let session = Session::start(
            &self.zenith,
            &["--solar", solar.as_str()],
            &[("ZENITH_TRACE_TIMINGS", timings_text.as_str())],
            100,
            30,
        )?;
        session.wait_for("in orbit", PATIENCE)?;
        Ok((session, timings))
    }
}

/// Runs the measurements. `--idle-seconds N` sets the idle window, sixty by default, and
/// `--runs N` the number of starts, ten by default.
///
/// # Errors
///
/// What stopped a measurement.
pub(crate) fn run(root: &Path, rest: &[&str]) -> Result<(), String> {
    let option = |name: &str, default: u64| -> u64 {
        rest.iter()
            .position(|argument| *argument == name)
            .and_then(|at| rest.get(at + 1))
            .and_then(|value| value.parse().ok())
            .unwrap_or(default)
    };
    let setup = Setup {
        zenith: release_binary(root)?,
        solar: solar()?,
        out: root.join("target").join("perf"),
    };
    std::fs::create_dir_all(&setup.out).map_err(|error| error.to_string())?;

    let starts = startup(&setup, option("--runs", 10))?;
    let (handled, seen) = keys(&setup)?;
    let idle = idle(&setup, option("--idle-seconds", 60))?;
    println!("perf: {CALLS} solar.ping at trace, then at off");
    let (at_trace, stderr_per_call) = round_trips(&setup.solar, "trace")?;
    let (at_off, _) = round_trips(&setup.solar, "off")?;

    println!();
    println!("  what                                        median       p99       max");
    row("start to connected, seen on screen", &starts.observed);
    row("start to connected frame, in process", &starts.drawn);
    if let Some(slowest) = starts
        .steps
        .iter()
        .max_by_key(|step| step["started_us"].as_u64().unwrap_or(0))
    {
        let at = |key: &str| millis(u128::from(slowest[key].as_u64().unwrap_or(0)));
        println!(
            "  slowest start: SOLAR running at {}, after finding it in {}, its copy in {} \
             and its process in {}",
            at("started_us"),
            at("locate_us"),
            at("prepare_us"),
            at("spawn_us")
        );
    }
    row(
        &format!("key read to frame flushed, {} keys", handled.len()),
        &handled,
    );
    row(
        &format!("key written to key on screen, {} keys", seen.len()),
        &seen,
    );
    row("solar.ping round trip, SOLAR at trace", &at_trace);
    row("solar.ping round trip, SOLAR at off", &at_off);
    println!("  at trace, SOLAR writes {stderr_per_call} bytes of standard error a call");
    println!(
        "  idle: {} ms of processor in {} ms, {} wake-ups, {} MiB resident",
        idle["cpu_ms"],
        idle["window_ms"],
        idle["wakeups"],
        idle["resident_bytes"].as_u64().unwrap_or(0) / (1024 * 1024)
    );
    let results = json!({
        "startup_observed_us": starts.observed,
        "startup_connected_frame_us": starts.drawn,
        "startup_steps_us": starts.steps,
        "key_to_frame_us": summary(&handled),
        "key_to_screen_us": summary(&seen),
        "idle": idle,
        "ping_round_trip_us": {
            "calls": CALLS,
            "trace": summary(&at_trace),
            "off": summary(&at_off),
            "stderr_bytes_per_call_at_trace": stderr_per_call,
        },
    });
    let path = setup.out.join("results.json");
    std::fs::write(&path, results.to_string() + "\n").map_err(|error| error.to_string())?;
    println!("\nperf: results in {}", path.display());
    Ok(())
}

/// Microseconds as milliseconds, to a hundredth.
fn millis(us: u128) -> String {
    format!("{}.{:02} ms", us / 1000, us % 1000 / 10)
}

/// One line of the table: the median, the 99th percentile and the largest.
fn row(what: &str, sorted: &[u128]) {
    println!(
        "  {what:<40} {:>10} {:>9} {:>9}",
        millis(percentile(sorted, 0.5)),
        millis(percentile(sorted, 0.99)),
        millis(sorted.last().copied().unwrap_or(0))
    );
}

/// The median, the 99th percentile, the largest and the count, for the results file.
fn summary(sorted: &[u128]) -> Value {
    json!({
        "p50": percentile(sorted, 0.5),
        "p99": percentile(sorted, 0.99),
        "max": sorted.last(),
        "count": sorted.len(),
    })
}

/// What the starts measured.
struct Starts {
    /// From starting ZENITH to `in orbit` on its screen, sorted.
    observed: Vec<u128>,
    /// From starting ZENITH to its first connected frame, by its own clock, sorted.
    drawn: Vec<u128>,
    /// The steps of starting SOLAR in each run, in the order of the runs.
    steps: Vec<Value>,
}

/// Starts ZENITH `runs` times and measures each start.
fn startup(setup: &Setup, runs: u64) -> Result<Starts, String> {
    let mut observed = Vec::new();
    let mut drawn = Vec::new();
    let mut steps = Vec::new();
    for run in 0..runs {
        let (mut session, timings) = setup.connected(&format!("startup-{run}.ndjson"))?;
        observed.push(session.started().elapsed().as_micros());
        session.quit(PATIENCE)?;
        if let Some(us) = events(&timings, "connected_frame")
            .first()
            .and_then(|event| event.us)
        {
            drawn.push(us);
        }
        if let Some(started) = events(&timings, "solar_started").first() {
            steps.push(json!({
                "started_us": started.us,
                "locate_us": started.value["locate_us"],
                "prepare_us": started.value["prepare_us"],
                "spawn_us": started.value["spawn_us"],
            }));
        }
        println!("perf: start {} of {runs} connected", run + 1);
    }
    observed.sort_unstable();
    drawn.sort_unstable();
    Ok(Starts {
        observed,
        drawn,
        steps,
    })
}

/// For two hundred keys, the time from the loop reading each to its frame being flushed;
/// and for fifty of them, from writing the key to seeing it on the screen; each sorted.
fn keys(setup: &Setup) -> Result<(Vec<u128>, Vec<u128>), String> {
    let (mut session, timings) = setup.connected("keys.ndjson")?;
    thread::sleep(Duration::from_millis(500));
    let mut seen = Vec::new();
    for index in 0..200 {
        if index % 40 == 0 {
            // A fresh line, so the typed text is always on screen whole.
            session.write(b"\x15")?;
            thread::sleep(Duration::from_millis(30));
        }
        let before = Instant::now();
        session.write(b"z")?;
        if index % 4 == 0 {
            let typed = "z".repeat(index % 40 + 1);
            session.wait_for(&format!("{typed} "), PATIENCE)?;
            seen.push(before.elapsed().as_micros());
        } else {
            thread::sleep(Duration::from_millis(15));
        }
    }
    session.write(b"\x15")?;
    thread::sleep(Duration::from_millis(200));
    session.quit(PATIENCE)?;
    let mut handled: Vec<u128> = events(&timings, "key")
        .into_iter()
        .filter_map(|event| event.total_us)
        .collect();
    handled.sort_unstable();
    seen.sort_unstable();
    Ok((handled, seen))
}

/// The processor time and the wake-ups over `seconds` after the first minute connected,
/// and the resident memory at the end, for the results file.
fn idle(setup: &Setup, seconds: u64) -> Result<Value, String> {
    let (mut session, timings) = setup.connected("idle.ndjson")?;
    println!("perf: idle for {seconds} s after the first minute");
    thread::sleep(Duration::from_secs(65));
    let (cpu_before, _) = session.usage().ok_or("the process could not be measured")?;
    let window_start = session.started().elapsed();
    thread::sleep(Duration::from_secs(seconds));
    let (cpu_after, memory) = session.usage().ok_or("the process could not be measured")?;
    let window_end = session.started().elapsed();
    // ZENITH's clock starts a few milliseconds after the harness's, so the key that quits
    // it waits a second to keep its wake-up out of the window.
    thread::sleep(Duration::from_secs(1));
    session.quit(PATIENCE)?;
    let wakes = events(&timings, "wake")
        .into_iter()
        .filter_map(|event| event.us)
        .filter(|us| *us >= window_start.as_micros() && *us <= window_end.as_micros())
        .count();
    Ok(json!({
        "window_ms": window_end.saturating_sub(window_start).as_millis(),
        "cpu_ms": cpu_after.saturating_sub(cpu_before),
        "wakeups": wakes,
        "resident_bytes": memory,
    }))
}

/// The round trips of [`CALLS`] `solar.ping`, one at a time, through ZENITH's own
/// connection code with SOLAR at `level`, sorted; and the bytes of standard error SOLAR
/// wrote a call.
fn round_trips(solar: &Path, level: &str) -> Result<(Vec<u128>, usize), String> {
    let found = Found {
        path: solar.to_path_buf(),
        origin: Origin::Environment,
    };
    let prepared =
        prepare(found, &Placement::for_this_system()).map_err(|error| error.to_string())?;
    let (sender, events) = mpsc::channel();
    let sink: Sink = Arc::new(move |_, happened| sender.send(happened).is_ok());
    let settings = Settings {
        log_level: level.to_owned(),
        environment: Vec::new(),
    };
    let connection =
        Connection::start(prepared, &settings, 1, sink).map_err(|error| error.to_string())?;
    let mut trips = Vec::with_capacity(CALLS);
    let mut stderr_bytes = 0;
    let mut count = |happened: Happened, counting: bool| {
        if let Happened::Stderr { text, .. } = happened
            && counting
        {
            stderr_bytes += text.len() + 1;
        }
    };
    for id in 0..WARM_UP + CALLS {
        let counting = id >= WARM_UP;
        let line = format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"solar.ping"}}"#);
        let before = Instant::now();
        connection.send(line).map_err(|error| error.to_string())?;
        loop {
            match events.recv_timeout(PATIENCE) {
                Ok(Happened::Line { .. }) => break,
                Ok(happened) => count(happened, counting),
                Err(_) => return Err(format!("solar did not answer call {id} at {level}")),
            }
        }
        if counting {
            trips.push(before.elapsed().as_micros());
        }
    }
    // The standard error of the last calls may still be on its way.
    thread::sleep(Duration::from_millis(300));
    while let Ok(happened) = events.try_recv() {
        count(happened, true);
    }
    let _ = connection.shutdown(Duration::from_secs(2));
    trips.sort_unstable();
    Ok((trips, stderr_bytes / CALLS))
}

/// One line of a timing file.
struct Event {
    us: Option<u128>,
    total_us: Option<u128>,
    value: Value,
}

fn events(path: &Path, name: &str) -> Vec<Event> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|value| value["event"] == name)
        .map(|value| Event {
            us: value["us"].as_u64().map(u128::from),
            total_us: value["total_us"].as_u64().map(u128::from),
            value,
        })
        .collect()
}
