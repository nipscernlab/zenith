# Changelog

Every notable change to ZENITH, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the versions follow
[semantic versioning](https://semver.org/spec/v2.0.0.html).

ZENITH speaks SOLAR's protocol, `solar/1`, and reads manifests of layout 2. A change to
either is stated here in its own line.

## [Unreleased]

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

### Changed

- Development builds keep file and line for backtraces and no more, and the dependencies
  keep no debug information: full debug information made `target/debug` 3.6 GB.

### Fixed

- A backslash before a character outside ASCII inside a JSON string, such as `"\é"`, no
  longer brings ZENITH down: the scanner of the command line cut the character in half.
  The property tests found it.
- No member of a response is shown with its name cut short: `/version` showed
  `manifest_schema_versio`. The column of names grows to a third of the line, and a longer
  name has a line of its own.

[Unreleased]: https://github.com/nipscernlab/zenith/commits/main
