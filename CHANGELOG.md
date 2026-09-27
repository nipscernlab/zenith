# Changelog

Every notable change to ZENITH, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the versions follow
[semantic versioning](https://semver.org/spec/v2.0.0.html).

ZENITH speaks SOLAR's protocol, `solar/1`, and reads manifests of layout 2. A change to
either is stated here in its own line.

## [Unreleased]

### Added

- The command line history survives between sessions, as the architect decided, ADR 0014:
  in `command-history.ndjson` under the per-user data directory of each system, or of
  `ZENITH_DATA_DIR`, bounded like the history in memory, written whole after every line on
  a thread of its own, through a temporary file renamed over it, and in a documented,
  versioned format. `/forget` empties it, and `--no-history` or `ZENITH_NO_HISTORY` keep it
  for the session only. A file ZENITH cannot read is left as it is.
- ZENITH's own mark, in `docs/brand`: the dome of an observatory with its slit open at the
  zenith, on the module and in the five colours of SOLAR's brand. It comes as SVG in the
  palette's colours and in one colour, a separate drawing for 16 px, lockups for dark and
  light backgrounds, and terminal forms in half blocks and in ASCII, with the rules it is
  drawn by in `docs/brand/README.md`. Tests hold every file to those rules.
- What SOLAR offers is read from the manifest's `capabilities`, layout 2.1.0, section 8.2
  of SOLAR's contract: `Ctrl+C` cancels with the method it declares, the connected notice
  says whether SOLAR answers batches and whether the Log tab sets its level, a call longer
  than `limits.max_request_bytes` is refused before it is sent, `/raw` excepted, and the
  card of `/list` shows every capability and limit. A manifest without `capabilities` is
  read as before.
- The Log tab's level keys set SOLAR's own level with `solar.set_log_level`, when SOLAR
  has it, and the header says the level SOLAR logs at; a restart starts SOLAR at that
  level. Without it, the keys filter, as before.
- The guide's table has a row for `/export`, which the walkthrough checks with
  `solar replay`.

### Changed

- `/export`, and `x` on the History tab, write the current connection in SOLAR's
  recording format 1.0.0, `docs/RECORDING.md` in SOLAR's repository, under the name
  `zenith-recording-<time>.ndjson`: a header naming ZENITH as the writer, then every line
  that crossed, in order and exactly as it crossed, so that `solar replay` sends the
  requests again. ZENITH's own `zenith-history` format is gone; the report keeps its own.
- The opening shows ZENITH's mark, with the name, what ZENITH is, the laboratory and the
  version beside it, and the state of the connection on the line below. SOLAR's mark
  moved to where ZENITH shows SOLAR itself, the start of every connection and the card of
  `/version`, with the version of the SOLAR that answered; its copies moved to
  `crates/zenith/assets/solar/`. The opening is snapshotted in every theme at 256
  colours, at 16 and with none, and the card of `/version` in every theme at both sizes.
- The seven decisions the architect confirmed on 27 September 2026 are records in
  `docs/adr/`, 0007 to 0013, and every record is in the MADR 4.0.0 format;
  `docs/OPEN_QUESTIONS.md` keeps only what is still open.

### Fixed

- The property test that holds ZENITH's schema validator to the `jsonschema` crate failed
  on a schema ZENITH judges rightly: `jsonschema` 0.58.1 ignores what `then` and `else`
  evaluate when `if` is `true` or `false`, which the Python `jsonschema` 4.26.0 does not.
  The test now hands the crate each boolean `if` as the object schema it stands for, and
  keeps the case that found it.

- No mark was ever seen on a fast machine: the opening lasts as long as the connection,
  32 ms on the architect's, as the design requires. Every connection now starts the
  Session tab with SOLAR's mark and the version that answered, above the notice that it
  connected, and the opening stays as short as the connection.

## [0.1.0] - 2026-09-27

The first version, for SOLAR's protocol `solar/1` and manifests of layout 2, tested
against SOLAR 0.1.0 and 0.2.0.

### Added

- The design, `docs/DESIGN.md`, written before the code: the screens, the keys, the theme,
  the connection to SOLAR and its failure paths, the tabs, the commands, the files ZENITH
  writes, the limits on memory, the tests and the measurements.
- The six decisions of the brief as records in `docs/adr/`, and the decisions taken
  without asking in `docs/OPEN_QUESTIONS.md`.
- The workspace: the toolchain SOLAR pins, the same lints, formatting, spelling and
  supply chain rules, and the NIPS-CERN Licence 1.1 copied byte for byte.
- `zenith-client`, everything that talks to SOLAR and nothing that draws: finding `solar`
  by `--solar`, `ZENITH_SOLAR` or the `PATH`; on Windows, starting it from a copy under
  `%TEMP%\zenith\solar\<hash>\` so that SOLAR's build can always replace
  `target\release\solar.exe`; the child process with one thread per pipe and a limit in
  bytes on every line; matching responses to calls by `id`; reading every response
  against sections 5, 6 and 7 of SOLAR's contract; the handshake that checks the protocol
  and the manifest layout; the catalogue of the manifest, which detects `solar.cancel`.
- A schema validator of ZENITH's own for JSON Schema 2020-12, held by property tests to
  the verdicts of the `jsonschema` crate on 4 096 generated schemas and on every schema of
  a real manifest, and a reader of schemas as descriptions for completion and forms.
- `zenith-solar-double`, a stand-in for `solar serve --stdio` that the tests use to
  produce the failures a real SOLAR cannot: exiting at once, another protocol, garbage,
  silence, a line too long to keep, and a `solar.cancel` ahead of SOLAR's own.
- The look of ZENITH: the five colours of SOLAR's brand and the roles drawn from them in
  three themes, night, light and high-contrast, at four depths from true colour to none,
  with the contrast of every role computed with the formula of WCAG 2.2; the characters,
  WGL4 by default and 7-bit ASCII with `--ascii`; the SOLAR mark from SOLAR's
  `docs/brand`, byte for byte; ring buffers with a limit in entries and in bytes; times in
  UTC.
- The keys, in one table that both dispatches and draws the help, every action on a plain
  or `Ctrl` key. A character typed with `AltGr` is text, which is how `/` and `?` are typed
  on a Brazilian ABNT2 keyboard. The line editor with a bounded history, the thirteen slash
  commands and their parser, and completion of commands, API names, parameter names and
  values from the manifest.
- The application, a state machine with no terminal and no clock of its own: the
  handshake and its version checks, every failure path with what happened and what to do,
  calls matched by id, parameters checked before they are sent with the part of the line to
  underline, examples judged against what they declare, the parameter form, `Ctrl+C` that
  cancels when SOLAR offers `solar.cancel`, the Log filtered by level, the History, and
  `/export` and `/report` written as they go.
- Every screen drawn: the opening with the SOLAR mark in a starfield, the header, the
  status bar drawn as SOLAR's orbit, the four tabs, the completion menu, the help and the
  envelope viewer. 105 snapshots of every screen at 80 × 24 and 160 × 48 in each theme,
  and without colour in ASCII. Every control character from SOLAR is drawn as an escape.
- The `zenith` binary: the loop that waits on one channel and wakes only for a deadline,
  the terminal given back on a panic or a signal, SOLAR found, copied and started on a
  thread of its own, and `--solar`, `--theme`, `--color`, `--ascii` and `--solar-log`.
- ZENITH's own loop driven by keys against the installed SOLAR: a walk through every API
  of the manifest that runs every example from the APIs tab and opens every form, every
  API described from the command line, the commands of the brief, a batch sent by hand,
  `/report` and `/export`, `Ctrl+R`, and the copy on Windows; and against the double,
  `Ctrl+C` cancelling a call in flight, a SOLAR that exits at once, one that speaks another
  protocol and one that is not there.
- Property tests that give the scanner, the parser and the completer any text at all, and
  the whole application any sequence of keys, pastes, responses, lines of standard error,
  resizes, exits and reconnections: nothing panics, and nothing they return points outside
  the line.
- `CANCELLED`, code `-32008`, among the statuses every response is checked against, now
  that section 6.1 of SOLAR's contract lists it; a cancelled call with any other code is
  a breach of the contract, shown as one.
- The largest response an API may produce, `max_output_bytes`, on its card beside its
  budget, when the API declares it; the facts of that line now break between one another
  rather than inside one.
- The file `ZENITH_TRACE_TIMINGS` names records every wake-up of the loop, and how long
  finding SOLAR, making its copy and starting its process took.
- A test that holds the tables of limits in `docs/DESIGN.md` and `docs/OPEN_QUESTIONS.md`
  to the constants of `limits.rs`, so that the documents cannot say one size and the code
  keep another.

- `docs/TESTING_BY_HAND.md`, the guide for testing SOLAR through ZENITH on a Mac, on
  Linux and on Windows, for someone who has never used Rust: installing, building both
  programs, what to try, and what to send back when something is not right.
- The README, with pictures drawn from snapshots of a session recorded against SOLAR 0.2.0
  and replayed through ZENITH's own drawing code, by `cargo xtask screenshots`, which with
  `--check` fails when a picture is not its snapshot.
- CI on Linux, Windows and macOS, against SOLAR built from its main branch, and
  `cargo xtask ci`, which runs the same steps in the same order with the same flags:
  formatting, TOML formatting, spelling, lints, tests, doctests, documentation, a check
  that every commit changing code adds to this file, the README's pictures, the supply
  chain, and the coverage against a floor of 88 % of lines. Weekly, the next compiler, the
  declared minimum, and mutation testing, which runs only in CI.
- The real binary in a pseudo-terminal. `cargo xtask perf` measures the start, a key to
  its redraw, the processor at idle and the price of starting SOLAR at `trace`;
  `cargo xtask soak` the memory over a hundred thousand calls; `cargo bench` the drawing
  of each tab. `cargo xtask walkthrough` types every row of the table in
  `docs/TESTING_BY_HAND.md` into ZENITH and waits for what the row says, in CI on all
  three systems, and a test holds the rows and the steps together.
- `AGENTS.md`, the one entry point for a person or a coding agent, which `CLAUDE.md`
  imports; `CONTRIBUTING.md`; and `docs/ADDING_A_FEATURE.md`, the one path for adding a
  command, a key, a view or a tab.

- `STATUS.md`: what is ready, what was measured and how, what was decided without asking,
  what ZENITH needed from SOLAR and did not find, and what is left.

### Changed

- The reason for the fifteen seconds the handshake waits is the one measured: the first
  of ten starts on Windows spent 475 ms reading SOLAR's binary and starting it, against
  about 5 ms for the others. It was given as the antivirus scanning `solar.exe`, which is
  what that looks like and was never measured.
- Development builds keep file and line for backtraces and no more, and the dependencies
  keep no debug information: full debug information made `target/debug` 3.6 GB.

### Fixed

- A backslash before a character outside ASCII inside a JSON string, such as `"\é"`, no
  longer brings ZENITH down: the scanner of the command line cut the character in half.
  The property tests found it.
- No member of a response is shown with its name cut short: `/version` showed
  `manifest_schema_version` without its last letter. The column of names grows to a third
  of the line, and a longer name has a line of its own.
- The documentation builds with warnings denied, as CI builds it: a link to the `pointer`
  module of `zenith-client` could also have meant the primitive type of that name.
- The coverage no longer moves with chance: the random cases of the property tests were
  the only tests that reached the Log's paging, expansion and clearing, the History's
  sending again and editing, the form's movement, and sending while SOLAR is down. Each
  now has a test of its own, and a coverage run seeds the random cases.
- When SOLAR exits, its last lines of standard error are in the card that says so: the
  exit could be reported before the thread reading standard error had delivered them. It
  is now reported once that pipe is read to its end, or half a second after the exit.

[Unreleased]: https://github.com/nipscernlab/zenith/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/nipscernlab/zenith/tree/v0.1.0
