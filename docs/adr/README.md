# Decision records

One file per decision, in the [MADR](https://adr.github.io/madr/) format. A record says
what was decided, what it rules out, and why, in enough detail that somebody who was not
there can disagree with it on the merits.

A decision made without asking arrives in [`../OPEN_QUESTIONS.md`](../OPEN_QUESTIONS.md),
which is the list of things settled by whoever wrote the code and still open to being
overruled. When the architect confirms one, it moves here and leaves that file. The
records below were decided by the architect in the brief of the first stage, so they
start here.

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
