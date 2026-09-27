# Status

ZENITH 0.1.0, on 27 September 2026: what is ready, what was measured and how, what was
decided without asking, what ZENITH needed from SOLAR and did not find, and what is left.
Every figure below was taken on the machine ZENITH was written on, a Windows 11 laptop
with an Intel Core i7-13620H, against SOLAR 0.2.0 built from commit `76d80c0` of its main
branch with uncommitted changes, as `/version` reported it.

## Ready

Everything the brief asks for, as `docs/DESIGN.md` describes it.

| Part | What there is |
| ---- | ------------- |
| The connection | `solar` found by `--solar`, `ZENITH_SOLAR` or the `PATH`, and on Windows run from a copy under `%TEMP%\zenith\solar\<hash>\` so that SOLAR's build can always replace its binary. The handshake checks the protocol, `solar/1`, and the manifest layout, 2, and says both versions when they differ. Every failure, from a missing `solar` to one that exits or breaks the contract, is one sentence of what happened and one of what to do, with SOLAR's last lines of standard error, and `Ctrl+R` or `/reconnect` starts it again. |
| The tabs | Session, with the command line, its history and completion, and every response laid out; APIs, the catalogue of the manifest, with each example run by a key and judged against what it declares, and a form for the parameters; Log, SOLAR's standard error by level; History, every call with its timing. |
| The commands | The eight of the brief, `/list`, `/describe`, `/call`, `/ping`, `/version`, `/theme`, `/help` and `/quit`, and five more: `/raw`, `/reconnect`, `/clear`, `/export` and `/report`. |
| Checking | Parameters checked against the API's schema before they are sent, with the wrong part of the line underlined, by a validator of ZENITH's own that property tests hold to the verdicts of the `jsonschema` crate. Every response checked against sections 5, 6 and 7 of SOLAR's contract, and a breach shown with its section. |
| Cancelling | `Ctrl+C` sends `solar.cancel` for the call in flight when the manifest offers it, and pressed again quits. |
| The keys | One table that dispatches the keys and draws the help overlay. Every action has a plain key or a `Ctrl` key, for macOS Terminal, where Option is not Alt; a character typed with `AltGr` is text. |
| The look | The five colours of SOLAR's brand in three themes, night, light and high-contrast, each at four depths down to none, with `NO_COLOR` obeyed; the contrast of every colour computed against WCAG 2.2; WGL4 characters, or 7-bit ASCII with `--ascii`; the SOLAR mark from SOLAR's brand, byte for byte. Works at 80 × 24 and above, follows resizing, and gives the terminal back on exit, on a signal and after a panic. |
| Memory | Every collection that grows with use is a ring buffer with a limit in entries and in bytes, and says how much it dropped. `/export` and `/report` are written as they go. |
| Bug reports | `/report` writes one file with the versions of ZENITH and SOLAR, the system as `system.info` reports it, the recent log and the calls of the session. |

## Tested

| What | How many | Where |
| ---- | -------- | ----- |
| Tests | 293, all passing; 27 of them start a real program, the installed SOLAR or a double of it for the failures SOLAR cannot produce | `cargo nextest run --workspace` |
| Snapshots | 112: every screen at 80 × 24 and 160 × 48 in each theme, without colour and in ASCII, and the README's seven | `crates/zenith/tests/snapshots/` |
| Line coverage | 88.37 % of the lines of `zenith-client` and `zenith` on Windows, 88.15 % on Linux in CI; the floor is 88 % | `cargo xtask coverage` |
| The guide's table | 20 of 20 rows, typed into the real binary in a pseudo-terminal, on Windows 11 and in AlmaLinux 9 | `cargo xtask walkthrough` |

**CI**, run 36343099884, on the push of 27 September 2026 at 16:06: all four jobs pass,
every step on Linux, Windows and macOS, the tests against SOLAR and the walkthrough
included. CI builds SOLAR from the head of its main branch, which at that moment was
commit `2497b3f`, SOLAR 0.1.0, from before batches and `solar.cancel`: CI walked the
table with a SOLAR that refuses batches, and the row for `solar.cancel` did not apply.
SOLAR pushed its batches and `solar.cancel` minutes later.

One test was flaky on Linux in that run: a SOLAR that dies at once could have its exit
reported before its last words had been read, so the card lost the line that says why it
died. It is fixed; in AlmaLinux under WSL the test failed 2 runs in 40 before the fix and
none in 40 after.

**AlmaLinux 9 under WSL**, on the same machine. ZENITH is cloned into `~/zenith`, inside
the WSL file system, and built there; `zenith` and `solar` are links in `~/.local/bin`,
which AlmaLinux's own `~/.bashrc` puts on the `PATH`, so both run from any new shell.
`solar` is SOLAR's own build in `~/solar`, which ZENITH's setup only reads, so it is
always SOLAR's latest there. At commit `77f25ff` the 291 tests pass against it, none
skipped, and the walkthrough does the 20 rows. Nothing needed `sudo`.

`cargo` is not on the `PATH` of a new shell there, because rustup was installed without
touching `~/.bashrc`. To rebuild ZENITH after a push:

```bash
cd ~/zenith && git pull --ff-only && ~/.cargo/bin/cargo build --release --locked -p zenith
```

or, once, `echo '. "$HOME/.cargo/env"' >> ~/.bashrc` to have `cargo` everywhere.

The laboratory Mac is not tested yet: that is Arthur's run of `docs/TESTING_BY_HAND.md`.

## Measured

`cargo xtask perf` and `cargo xtask soak` measure the release binary in a pseudo-terminal,
ConPTY on this machine; `cargo bench -p zenith --bench draw` measures drawing in process.
The raw figures are in `target/perf/` after a run. The table has the last of four runs of
`perf` that day, at commit `77f25ff`; over the four, the median start to the first
connected frame was 20.5 to 24.7 ms, and the median key to its frame 0.31 to 0.49 ms.

| What | Figure |
| ---- | ------ |
| Start to the first frame drawn connected, by ZENITH's own clock | 24.7 ms, the median of ten starts; 19.5 ms the fastest |
| Start to `in orbit` on the terminal's screen | 50.7 ms, the median of the same ten |
| A key read to its frame flushed | 0.49 ms median, 1.07 ms at the 99th percentile, 6.22 ms the slowest, over 208 keys |
| A key written into the terminal to seeing it there | 15.7 ms median, 18.5 ms at the 99th percentile, over 50 keys |
| Idle, over a minute after the first | 0 ms of processor, one wake-up, 8.6 MiB resident |
| A `solar.ping` through ZENITH's connection code | 0.082 ms median with SOLAR at `trace`, 0.076 ms at `off`, over 2 000 calls each |
| What SOLAR writes to standard error at `trace` | 693 bytes a call |
| Memory over 100 000 `/ping` | 8.7 MiB at the start, 24.6 MiB at 10 000, 27.9 MiB at 20 000, and 28.6 MiB at 100 000; the highest sample was 29.6 MiB, at 60 000 |
| Drawing a frame, at 80 × 24 | Session 0.23 ms, APIs 0.13 ms, Log 0.14 ms, History 0.11 ms, the help overlay 0.64 ms |
| Drawing a frame, at 200 × 60 | Session 1.03 ms, APIs 0.91 ms, Log 0.38 ms, History 0.54 ms, the help overlay 2.48 ms |
| The release binary, `zenith.exe` | 2 285 568 bytes, with fat LTO and one codegen unit |

What the figures say:

- **Idle is idle.** The loop waits on one channel and wakes only for a deadline on screen;
  after the first minute, the only one is the minute of `in orbit`.
- **The memory is bounded.** It rises while the History, the Log and the transcript fill,
  and from 20 000 calls on it stays level, within 2.4 % at 100 000. The soak fails when
  the last figure is more than a fifth above the one at 20 000.
- **`trace` costs SOLAR's standard error and not time.** The median round trip differed
  by at most 6 µs between `trace` and `off` in the two runs that measured it, so ZENITH
  can start SOLAR at `trace` and filter in the Log tab.
- **The first start of a run is slower.** In three of the four runs the first of the ten
  starts was the slowest, 329, 582 and 671 ms to show `in orbit`. The last run recorded
  where the time went: finding SOLAR took 0.09 ms, hashing `solar.exe` to name its copy
  238 ms, and starting the copy's process 237 ms, against about 1.2 ms and 3 to 8 ms in
  the quick starts; the copy already existed. Both slow steps are a first read and a first
  run of a file, which is what the antivirus of Windows scanning it would look like: that
  is deduced from the pattern, not measured. In three other starts of that run, 39 to
  86 ms went before ZENITH's own loop began, in parsing the command line, entering raw
  mode and the alternate screen, and opening the timings file; which of those is not
  recorded.
- **Most of a key's way to the screen is outside ZENITH.** Between a key flushed, 0.49 ms,
  and a key seen, 15.7 ms, are ConPTY and the harness that reads it: deduced from the two
  figures, not measured on its own.

## Decided without asking

`docs/OPEN_QUESTIONS.md` has each decision with what it rules out and what would change
it. The ones with the most consequence:

- **A schema validator of ZENITH's own.** `jsonschema` would have taken the release binary
  from 775 168 bytes to 4 598 272. Property tests hold ZENITH's validator to its verdicts.
- **`Tab` switches tabs on an empty command line**, and completes otherwise; no action
  depends on Alt or a function key.
- **Five commands beyond the eight of the brief**, `/raw`, `/reconnect`, `/clear`,
  `/export` and `/report`, because testing SOLAR by hand needs them.
- **Nothing is written to disk unless asked**: no configuration file, no persistent
  history.
- **The handshake waits fifteen seconds**, because on Windows the first run of a
  `solar.exe` can take far longer than the next, as the slow first start above shows.
- **The limits on memory**, and the soak's rule for growth.
- **The coverage floor at 88 %**, mutation testing that reports and does not gate yet, and
  development builds with line tables only, for the disk.

## What ZENITH needed from SOLAR and did not find

Each is in the first section of `docs/OPEN_QUESTIONS.md`, with what would close it. ZENITH
works around none of them.

1. **The manifest does not say whether batches are accepted.** SOLAR 0.1.0 refuses them
   and SOLAR 0.2.0 answers them, both as `solar/1`. ZENITH sends no batches of its own;
   `/raw` sends one by hand.
2. **The log level cannot be changed while SOLAR runs.** ZENITH starts SOLAR at `trace`
   and filters in the Log tab, which the measurement above says costs no time.
3. **The recording format of `solar serve --record` is not documented.** `/export`
   writes a format of ZENITH's own, specified in `docs/DESIGN.md`, section 12.

## Left

- **The Mac.** Arthur's run of `docs/TESTING_BY_HAND.md` on the laboratory Mac, in macOS
  Terminal and in iTerm2 if it is there.
- **Branch protection**, a setting of the repository for whoever administers
  `nipscernlab/zenith`: require the checks `ubuntu-latest`, `windows-latest`,
  `macos-latest` and `coverage` to pass before merging into `main`, and a review from the
  code owners.
- **The first mutation run**, on the first Monday: each survivor killed by a test or
  recorded as a mutation that changes nothing, after which the job can fail on new ones.
- **A mark for ZENITH**, when the brand has one. ZENITH draws only SOLAR's.
- **The slow first start**, with the steps it now records.

## Versions

ZENITH 0.1.0, tagged `v0.1.0`. Rust 1.97.1, the toolchain SOLAR pins. Tested against
SOLAR 0.1.0, whose manifest is the fixture of the unit tests and which CI built at
`2497b3f`, and SOLAR 0.2.0, which the tests against SOLAR, the measurements and the
walkthrough ran against on this machine.
