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

[Unreleased]: https://github.com/nipscernlab/zenith/commits/main
