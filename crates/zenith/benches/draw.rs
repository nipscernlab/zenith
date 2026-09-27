//! What drawing a screen costs, in process: each tab of the session the README shows, and
//! the help overlay, at 80 × 24 and at 200 × 60. Each figure is the building of a frame
//! and its comparison with the last one; what then reaches a real terminal, and how long a
//! key takes to show, is measured on the real binary by `cargo xtask perf`.
//!
//! `cargo bench -p zenith --bench draw` runs it.

#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "criterion's macros write undocumented items, and a benchmark fails by panicking"
)]

#[path = "../tests/common/mod.rs"]
mod common;

use std::hint::black_box;

use common::{Act, README_SCENARIO, Script};
use criterion::{Criterion, criterion_group, criterion_main};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use zenith::app::Options;

/// The session the README shows, after its first `acts` acts and then these keys.
fn scene(acts: usize, keys: &[KeyCode]) -> Script {
    let options = Options {
        opening: false,
        ..Options::default()
    };
    let mut script = Script::replayed(options, acts);
    for key in keys {
        script.key(*key, KeyModifiers::NONE);
    }
    script
}

fn drawing(criterion: &mut Criterion) {
    // The lines typed on the Session tab, then everything, which ends on the APIs tab.
    let lines = README_SCENARIO
        .iter()
        .position(|act| matches!(act, Act::NextTab))
        .expect("the README's session leaves the Session tab");
    let all = README_SCENARIO.len();
    let scenes = [
        ("session", scene(lines, &[])),
        ("apis", scene(all, &[])),
        ("log", scene(all, &[KeyCode::Tab])),
        ("history", scene(all, &[KeyCode::Tab, KeyCode::Tab])),
        ("help", scene(lines, &[KeyCode::Char('?')])),
    ];
    for (name, script) in &scenes {
        for (width, height) in [(80, 24), (200, 60)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            criterion.bench_function(&format!("{name} at {width} x {height}"), |bencher| {
                bencher.iter(|| {
                    terminal
                        .draw(|frame| zenith::ui::draw(frame, black_box(&script.app)))
                        .unwrap();
                });
            });
        }
    }
}

criterion_group!(benches, drawing);
criterion_main!(benches);
