# Status

ZENITH 0.2.0, on 27 September 2026: what is ready, what was measured and how, what was
decided without asking, what ZENITH needed from SOLAR and did not find, and what is left.
Every figure below was taken on the machine ZENITH was written on, a Windows 11 laptop
with an Intel Core i7-13620H, against SOLAR 0.3.1 built from commit `fcad2a395f07` of its
main branch with nothing uncommitted, as `solar version` reported it, unless it says
otherwise.

## Ready

Everything the brief asks for, as `docs/DESIGN.md` describes it.

| Part | What there is |
| ---- | ------------- |
| The connection | `solar` found by `--solar`, `ZENITH_SOLAR` or the `PATH`, and on Windows run from a copy under `%TEMP%\zenith\solar\<hash>\` so that SOLAR's build can always replace its binary. The handshake checks the protocol, `solar/1`, and the manifest layout, 2, and says both versions when they differ. Every failure, from a missing `solar` to one that exits or breaks the contract, is one sentence of what happened and one of what to do, with SOLAR's last lines of standard error, and `Ctrl+R` or `/reconnect` starts it again. |
| The tabs | Session, with the command line, its history and completion, and every response laid out; APIs, the catalogue of the manifest, with each example run by a key and judged against what it declares, and a form for the parameters; Log, SOLAR's standard error by level, whose keys set SOLAR's own level when it has `solar.set_log_level`; History, every call with its timing. |
| The commands | The eight of the brief, `/list`, `/describe`, `/call`, `/ping`, `/version`, `/theme`, `/help` and `/quit`, five more of the first stage, `/raw`, `/reconnect`, `/clear`, `/export` and `/report`, and `/forget` and `/mouse`. |
| Checking | Parameters checked against the API's schema before they are sent, with the wrong part of the line underlined, by a validator of ZENITH's own that property tests hold to the verdicts of the `jsonschema` crate. Every response checked against sections 5, 6 and 7 of SOLAR's contract, and a breach shown with its section. |
| What SOLAR offers | Read from the manifest's `capabilities` when SOLAR declares them, and from its APIs when it does not: `Ctrl+C` cancels the call in flight with the method the manifest declares, and pressed again quits; the connected notice says whether SOLAR answers batches and whether the Log tab sets its level; a call longer than SOLAR reads is refused before it is sent; `/list` shows every capability and limit. |
| The keys | One table that dispatches the keys and draws the help overlay. Every action has a plain key or a `Ctrl` key, for macOS Terminal, where Option is not Alt; a character typed with `AltGr` is text. |
| The mouse | The wheel scrolls what is under the pointer, text three lines a notch and a list one row, and never walks the command history; selecting is the terminal's, with its own key held; `/mouse`, `--no-mouse` and `ZENITH_NO_MOUSE` leave the mouse to the terminal (ADR 0015). |
| The look | The five colours of SOLAR's brand in three themes, night, light and high-contrast, each at four depths down to none, with `NO_COLOR` obeyed; the contrast of every colour computed against WCAG 2.2; WGL4 characters, or 7-bit ASCII with `--ascii`; ZENITH's own mark in the opening, from `docs/brand`, and SOLAR's at the start of every connection and on the card of `/version`, from SOLAR's brand, byte for byte. Works at 80 × 24 and above, follows resizing, and gives the terminal back on exit, on a signal and after a panic. |
| Memory | Every collection that grows with use is a ring buffer with a limit in entries and in bytes, and says how much it dropped. `/export` and `/report` are written as they go. |
| Bug reports | `/report` writes one file with the versions of ZENITH and SOLAR, the system as `system.info` reports it, the recent log and the calls of the session. `/export` writes the current connection in SOLAR's recording format 1.0.0, which `solar replay` sends again. |
| The command line history | Kept between sessions in the per-user data directory of each system, written atomically after every line, in a documented format; `/forget` empties it and `--no-history` turns it off. |

## Tested

| What | How many | Where |
| ---- | -------- | ----- |
| Tests | 452 on Windows against SOLAR 0.3.1 and 457 in AlmaLinux 9 under WSL against SOLAR 0.2.0, all passing and none skipped; 30 of them start the installed SOLAR or ZENITH's double of it, one on Linux only, and 2 start ZENITH itself in a pseudo-terminal, on Unix | `cargo nextest run --workspace` |
| Snapshots | 142: every screen at 80 × 24 and 160 × 48 in each theme, without colour and in ASCII, the help at its last page among them; the opening in each theme at 256 colours, 16 and none, and the Session tab at 256 and 16; the screen of a terminal too small; and the README's seven | `crates/zenith/tests/snapshots/` |
| Line coverage | 92.09 % of the lines of `zenith-client` and `zenith` on Windows, at the release, 92.83 % in AlmaLinux under WSL and 92.89 % in CI on Linux, 12 978 of 13 971 lines, at the mouse wheel, counted from the job's lcov; the floor is 88 % | `cargo xtask coverage` |
| The guide's table | 26 of 26 rows, typed into the real binary in a pseudo-terminal: on Windows 11 on this machine against SOLAR 0.3.1, in AlmaLinux under WSL against SOLAR 0.2.0, and in CI on Linux, Windows and macOS against SOLAR 0.3.1, the three rows of the mouse among them | `cargo xtask walkthrough` |
| Mutations | Before: on 0.1.0, 17 of the 20 shards finished, and of 2 237 mutants 1 608 were caught, 463 survived, 6 timed out and 160 could not be built, 77.70 % of the viable ones caught. After: not measured, since no run has finished all twenty shards; the ceiling is open for stage three | `.github/workflows/scheduled.yml` |

**CI** builds SOLAR from the head of its main branch on every run: SOLAR 0.3.0 for the
tests written for the mutation run's survivors, and 0.3.1 for the mouse wheel, the last
two pull requests of the stage. Every check passed on both, on Linux, Windows and macOS, the tests and
the walkthrough included, and on Windows the walkthrough's rows for the wheel went through
ConPTY, as they do on this machine.

**AlmaLinux 9 under WSL**, on the same machine. ZENITH is cloned into `~/zenith`, inside
the WSL file system, and built there; `zenith` and `solar` are links in `~/.local/bin`,
which AlmaLinux's own `~/.bashrc` puts on the `PATH`, so both run from any new shell.
`solar` is SOLAR's own build in `~/solar`, which ZENITH's setup only reads; it was SOLAR
0.2.0. At commit `25ff6bb`, the mouse wheel, every step of `cargo xtask ci` passed against
it: the 457 tests, none skipped, the test of the binary's wheel in a pseudo-terminal among
them, and the walkthrough's 26 rows. Nothing needed `sudo`.

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
The raw figures are in `target/perf/` after a run. The table has one run of each on the
code of 0.2.0, one after the other with nothing else running.

| What | Figure |
| ---- | ------ |
| Start to the first frame drawn connected, by ZENITH's own clock | 20.8 ms, the median of ten starts; 19.9 ms the fastest |
| Start to `in orbit` on the terminal's screen | 44.8 ms, the median of the same ten; 34.0 ms the fastest |
| A key read to its frame flushed | 0.45 ms median, 0.73 ms at the 99th percentile, 5.61 ms the slowest, over 208 keys |
| A key written into the terminal to seeing it there | 15.6 ms median, 18.5 ms at the 99th percentile, over 50 keys |
| Idle, over a minute after the first | 0 ms of processor, one wake-up, 9.4 MiB resident |
| A `solar.ping` through ZENITH's connection code | 0.078 ms median with SOLAR at `trace`, 0.072 ms at `off`, over 2 000 calls each |
| What SOLAR writes to standard error at `trace` | 693 bytes a call |
| Memory over 100 000 `/ping` | 9.5 MiB idle at the start, 25.3 MiB at 10 000, 29.1 MiB at 20 000, and 30.5 MiB from 70 000 to 100 000, the highest |
| Drawing a frame, at 80 × 24 | Session 0.11 ms, APIs 0.26 ms, Log 0.04 ms, History 0.15 ms, the help overlay 0.30 ms |
| Drawing a frame, at 200 × 60 | Session 0.72 ms, APIs 0.55 ms, Log 0.39 ms, History 0.21 ms, the help overlay 0.76 ms |
| The release binary, `zenith.exe` | 2 411 520 bytes, with fat LTO and one codegen unit |
| The build footprint | 12.8 GB after the release: the main checkout's build tree, 4.6 GB, the release's worktree, 5.0 GB, and the one in AlmaLinux under WSL, 3.2 GB; at the most, with a third tree for the mouse wheel, 16.0 GB. The architect's budget is 20 GB |

What the figures say:

- **Idle is idle.** The loop waits on one channel and wakes only for a deadline on screen;
  after the first minute, the only one is the minute of `in orbit`. Taking the mouse
  changed nothing here: ZENITH asks for no movement, and on Windows, whose console
  reports it anyway, movement stops in the thread that reads the terminal.
- **The memory is bounded.** It rises while the History, the Log and the transcript fill,
  and from 20 000 calls on it stays within 4.6 %, level from 70 000. The soak fails when
  the last figure is more than a fifth above the one at 20 000.
- **`trace` costs SOLAR's standard error and not time.** The median round trip differed
  by 6 µs between `trace` and `off`, so ZENITH can start SOLAR at `trace` and filter in the
  Log tab.
- **The first start of a run is slower.** Of the ten starts the first took 421 ms to show
  `in orbit`, and its steps say where: finding SOLAR took 0.07 ms, naming and checking its
  copy 3.1 ms, and starting the copy's process 286 ms, against 3 to 8 ms in the other
  nine. A first run of a file is what the antivirus of Windows scanning it would look like:
  that is deduced from the pattern, as in the first stage, not measured. One other start
  took 143 ms to show `in orbit`, where ZENITH's own steps account for at most 14 ms;
  where the rest went is not recorded.
- **Most of a key's way to the screen is outside ZENITH.** Between a key flushed, 0.45 ms,
  and a key seen, 15.6 ms, are ConPTY and the harness that reads it: deduced from the two
  figures, not measured on its own.
- **Drawing is well inside a frame.** The slowest, the help overlay at 200 × 60, takes
  0.76 ms. Against the first stage's figures some tabs drew faster and some slower; the
  scenes of the bench changed with the stage, the mark at the start of every connection
  and the sky among them, so the two are not compared figure by figure.

## Decided without asking

On 27 September 2026 the architect confirmed seven of those decisions, which are now
records in `docs/adr/`, 0007 to 0013: ZENITH's own schema validator, `Tab` switching tabs
on an empty command line, the five commands beyond the brief, the fifteen-second
handshake, the coverage floor at 88 %, the name `zenith`, and no mouse, for now. The same
day the architect decided the wheel must scroll, which is record 0015 and supersedes 0013,
and that the mutation measurement does not hold 0.2.0 back.

`docs/OPEN_QUESTIONS.md` has each decision that is still open, with what it rules out and
what would change it. The ones with the most consequence:

- **What a notch of the wheel does**: text three lines and a list one row; over the band
  and the command line the transcript scrolls, over the header, the status bar, the
  opening and the form nothing does, and an open help or viewer scrolls from anywhere.
- **The wheel is not a key**: it neither clears the status bar's message nor disarms the
  second `Ctrl+C`.
- **ZENITH asks for the mouse's buttons and not its movement**, so that moving the mouse
  wakes nothing.
- **Each terminal's key to select text is its documentation's**, not measured, since
  selecting takes a hand on the mouse; macOS Terminal documents none, only `Cmd+R`.
- **Every scroll stops at its end**, from the lines the drawing lays out, so that the
  first notch or key back moves the view.
- **The status bar keeps the state when it is too narrow**, as `docs/DESIGN.md` says, by
  dropping the stretches before `in orbit`.
- **A `multipleOf` of zero holds no number to anything**, since JSON Schema does not allow
  one and a tool that tests SOLAR must not refuse what SOLAR would accept.
- **The survivors of the mutation run in the drawing code wait for the complete run**,
  and where a mutant changed nothing, the redundant code went rather than the mutant being
  recorded as equivalent.
- **Where the command line history is kept**, `nipscern-zenith/command-history.ndjson`
  in the system's per-user data directory, the command `/forget` and the flag
  `--no-history`, and that two ZENITHs at once each write their own, the last one winning.
- **ZENITH's mark**, drawn without a designer: the dome of an observatory with its slit
  open at the zenith, in the opening, with SOLAR's mark moved to the start of every
  connection and the card of `/version`.
- **The Log tab's keys set SOLAR's own level**, one level and not a filter beside it, and
  a restart keeps it.
- **`/export` writes the current connection** in SOLAR's recording format, unanswered
  requests included and answers too long to keep left out whole.
- **The theme is not remembered**: no configuration file.
- **The limits on memory**, and the soak's rule for growth.
- **Mutation testing that reports and does not gate yet**, and development builds with
  line tables only, for the disk.

## What ZENITH needed from SOLAR and did not find

Each is in the first section of `docs/OPEN_QUESTIONS.md`, with what would close it. ZENITH
works around none of them.

1. **The exit codes of `solar replay` are not in the contract.** ZENITH's test of the
   replay takes exit 0 as every answer being the same, which is what SOLAR 0.3.0 did and
   0.3.1 does.
2. **`solar replay` reports a batch's answer as different when only its times differ**,
   seen with SOLAR 0.3.0 built with uncommitted changes. ZENITH's test of the replay
   records no batch.

What the first stage needed and did not find, SOLAR now has: the manifest declares its
capabilities, batches included, `solar.set_log_level` changes the level while SOLAR runs,
and the recording format is documented and versioned. ZENITH uses all three, and keeps
what it did before for a SOLAR that has none of them.

## Left

- **The Mac.** Arthur's run of `docs/TESTING_BY_HAND.md` on the laboratory Mac, in macOS
  Terminal and in iTerm2 if it is there, with what each does when text is selected while
  ZENITH has the mouse.
- **The mutation ceiling**, for stage three, set from the first run whose twenty shards
  all finish, as the architect decided on 27 September 2026; `docs/OPEN_QUESTIONS.md` has
  why. The one measurement, of 17 of the 20 shards on version 0.1.0, found 463 survivors
  among 2 237 mutants, 77.70 % of the viable ones caught. The survivors outside the
  drawing code have tests now; those in it wait for that run.
- **Clicks**, which do nothing in this version.
- **The slow first start**, with the steps it now records.

## How changes reach `main`

A ruleset protects `main`, as the repository's settings showed on 27 September 2026: a
change arrives by pull request, with the checks `ubuntu-latest`, `windows-latest`,
`macos-latest` and `coverage` passing; force pushes and deletion are refused; no review is
required. Every change of the stage arrived that way, the last two as pull requests #10
and #11, and 0.2.0 as its own.

## Versions

ZENITH 0.2.0, tagged `v0.2.0` on the merge commit of its release, with no release
artefacts. Rust 1.97.1, the toolchain SOLAR pins. Tested against SOLAR 0.1.0's manifest,
the fixture of the unit tests; SOLAR 0.2.0 in AlmaLinux under WSL; and SOLAR 0.3.1, which
the tests against SOLAR, the measurements and the walkthrough ran against on this machine,
and CI built from the head of SOLAR's main for the last pull request, after 0.3.0 for the
one before it.
