# Decision records

One file per decision, in the [MADR](https://adr.github.io/madr/) 4.0.0 format: the status,
the date and who decided in the front matter, then the context, the options considered,
the one chosen and why, and how the decision is confirmed. A record says what was decided,
what it rules out, and why, in enough detail that somebody who was not there can disagree
with it on the merits.

A decision made without asking arrives in [`../OPEN_QUESTIONS.md`](../OPEN_QUESTIONS.md),
which is the list of things settled by whoever wrote the code and still open to being
overruled. When the architect confirms one, it moves here and leaves that file. Records
0001 to 0006 were decided by the architect in the brief of the first stage, so they
started here; 0007 to 0013 were decisions taken without asking in the first stage, which
the architect confirmed on 27 September 2026; 0014 is a decision the architect made that
day for the second stage.

- **`OPEN_QUESTIONS.md`** is what is still open.
- **`docs/adr/`** is what is settled.

A record is never edited to say something different. A decision that is reversed gets a
new record that supersedes the old one, and the old one is marked as superseded, because
the reasoning that was once persuasive is part of the history.

| Record | Decision | Status |
| ------ | -------- | ------ |
| [0001](0001-a-client-of-solar-and-only-a-client.md) | ZENITH is a client of SOLAR and only a client | Accepted |
| [0002](0002-ratatui-and-crossterm.md) | The interface is drawn with ratatui on crossterm | Accepted |
| [0003](0003-solar-runs-from-a-copy-on-windows.md) | On Windows, `solar.exe` runs from a copy keyed by its hash | Accepted |
| [0004](0004-solar-is-tested-by-hand-through-zenith.md) | SOLAR is tested by hand through ZENITH | Accepted |
| [0005](0005-no-action-depends-on-alt.md) | No action depends on Alt | Accepted |
| [0006](0006-memory-is-bounded.md) | Every collection is bounded, and says what it dropped | Accepted |
| [0007](0007-zenith-has-its-own-schema-validator.md) | ZENITH has its own schema validator | Accepted |
| [0008](0008-tab-switches-tabs-on-an-empty-command-line.md) | `Tab` switches tabs on an empty command line | Accepted |
| [0009](0009-five-commands-beyond-the-brief.md) | Five commands beyond the eight the brief names | Accepted |
| [0010](0010-the-handshake-waits-fifteen-seconds.md) | The handshake waits fifteen seconds | Accepted |
| [0011](0011-the-coverage-floor-is-88-percent.md) | The coverage floor is 88 % of lines, and it only rises | Accepted |
| [0012](0012-the-name-zenith-is-shared-with-a-system-monitor.md) | The name `zenith` is shared with a system monitor | Accepted |
| [0013](0013-no-mouse-for-now.md) | No mouse, for now | Accepted |
| [0014](0014-the-command-line-history-survives-between-sessions.md) | The command line history survives between sessions | Accepted |
