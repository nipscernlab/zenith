---
status: accepted
date: 2026-09-27
decision-makers: Chrysthofer Arthur Amaro Afonso, in the brief of the first stage, on condition that both are maintained
---

# The interface is drawn with ratatui on crossterm

## Context and Problem Statement

ZENITH is a full-screen terminal application that must work in Windows Terminal, the
classic Windows console, PowerShell, macOS Terminal, iTerm2 and the common Linux
terminals, written in the Rust that SOLAR pins. What draws it?

## Considered Options

* `ratatui` for the layout and the widgets, on its `crossterm` backend, as the brief
  named them

## Decision Outcome

Chosen option: "`ratatui` on `crossterm`", because the brief named it, and the condition
it set, that both be maintained, was checked on 27 September 2026 before any code was
written:

| Crate | Latest release | Activity |
| ----- | -------------- | -------- |
| `ratatui` | 0.30.2, 19 June 2026 | commits on 25 September 2026, a release every few weeks |
| `crossterm` | 0.29.0, 5 April 2025 | commits on 14 September and 21 August 2026, fixes and features merged |

`crossterm` had not released in eighteen months, and its repository was active: that is
maintained, if slowly released. `ratatui` 0.30 depends on `crossterm` 0.29 through
`ratatui-crossterm`, and ZENITH uses the `crossterm` that `ratatui` re-exports, so the
two can never disagree on a version.

### Consequences

* Good, because `crossterm` is the one backend that covers the classic Windows console as
  well as the Unix terminals.
* Good, because `ratatui` draws into a buffer and writes only the cells that changed, so a
  redraw costs what changed, and its `TestBackend` makes every screen testable as a
  snapshot.
* Bad, because if `crossterm` stops being maintained, none of `ratatui`'s other backends
  covers the classic Windows console, so the question would be reopened rather than
  answered by a swap.
