//! `cargo xtask soak [--calls N]`: the memory of the real binary at idle and over a long
//! session, sampled as it runs.
//!
//! The binary runs in a pseudo-terminal against the installed SOLAR, and the session is
//! typed into it: `/ping` and `Enter`, in batches of fifty, a hundred thousand times by
//! default. After every batch the harness waits for the last ping's card, `#<id>`, to be
//! on screen, so that what is measured is calls answered and drawn, not keys queued. The
//! resident memory is sampled at idle and every ten thousand calls; ADR 0006 asks for it
//! not to grow, and the task fails when the last sample exceeds the one at twenty thousand
//! calls by more than a fifth, which is when every ring buffer is already full.

use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use crate::pty::{Session, release_binary, scratch_data_dir, solar};

const PATIENCE: Duration = Duration::from_mins(2);

/// Runs the soak.
///
/// # Errors
///
/// What stopped it, or that the memory grew.
pub(crate) fn run(root: &Path, rest: &[&str]) -> Result<(), String> {
    let calls: u64 = rest
        .iter()
        .position(|argument| *argument == "--calls")
        .and_then(|at| rest.get(at + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(100_000);
    let zenith = release_binary(root)?;
    let solar = solar()?;
    let solar_argument = solar.display().to_string();
    let data = scratch_data_dir("soak")?.display().to_string();
    let mut session = Session::start(
        &zenith,
        &["--solar", solar_argument.as_str()],
        &[("ZENITH_DATA_DIR", data.as_str())],
        120,
        40,
    )?;
    session.wait_for("in orbit", PATIENCE)?;
    thread::sleep(Duration::from_secs(5));
    let (_, idle) = session.usage().ok_or("the process could not be measured")?;
    let started = Instant::now();
    let mut samples: Vec<(u64, u64, u128)> = vec![(0, idle, 0)];
    let batch = 50;
    let mut done = 0;
    let mut keys = Vec::new();
    for _ in 0..batch {
        keys.extend_from_slice(b"/ping\r");
    }
    println!("soak: {calls} calls, the memory every 10 000");
    while done < calls {
        session.write(&keys)?;
        done += batch;
        // The handshake took ids 1 and 2, so the n-th ping is #(n + 2).
        wait_for_card(&session, done + 2)?;
        if done % 10_000 == 0 {
            let (_, resident) = session.usage().ok_or("the process could not be measured")?;
            let elapsed = started.elapsed().as_millis();
            println!(
                "soak: {done:>7} calls  {:>6} KiB resident  {:>6} s",
                resident / 1024,
                elapsed / 1000
            );
            samples.push((done, resident, elapsed));
        }
    }
    thread::sleep(Duration::from_secs(10));
    let (_, after) = session.usage().ok_or("the process could not be measured")?;
    samples.push((done, after, started.elapsed().as_millis()));
    session.quit(PATIENCE)?;
    let out = root.join("target").join("perf");
    std::fs::create_dir_all(&out).map_err(|error| error.to_string())?;
    let json: Vec<serde_json::Value> = samples
        .iter()
        .map(|(calls, resident, ms)| serde_json::json!({"calls": calls, "resident_bytes": resident, "elapsed_ms": ms}))
        .collect();
    std::fs::write(
        out.join("soak.json"),
        serde_json::Value::Array(json).to_string() + "\n",
    )
    .map_err(|error| error.to_string())?;
    println!(
        "soak: idle {} KiB, after {done} calls and ten quiet seconds {} KiB",
        idle / 1024,
        after / 1024
    );
    let at_twenty_thousand = samples
        .iter()
        .find(|(calls, _, _)| *calls >= 20_000)
        .map(|(_, resident, _)| *resident);
    if let Some(reference) = at_twenty_thousand
        && after > reference + reference / 5
    {
        return Err(format!(
            "the memory grew from {} KiB at 20 000 calls to {} KiB at {done}",
            reference / 1024,
            after / 1024
        ));
    }
    Ok(())
}

/// Waits until the card of call `id` is on screen, as `#<id>` with no digit after it.
fn wait_for_card(session: &Session, id: u64) -> Result<(), String> {
    let needle = format!("#{id}");
    let from = Instant::now();
    loop {
        let screen = session.screen();
        let found = screen.match_indices(&needle).any(|(at, _)| {
            !screen[at + needle.len()..]
                .chars()
                .next()
                .is_some_and(|next| next.is_ascii_digit())
        });
        if found {
            return Ok(());
        }
        if from.elapsed() > PATIENCE {
            return Err(format!(
                "the card of call {id} did not appear within {PATIENCE:?}; the screen was:\n{screen}"
            ));
        }
        thread::sleep(Duration::from_millis(5));
    }
}
