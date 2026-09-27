# 2. The interface is drawn with ratatui on crossterm

- **Status:** Accepted
- **Date:** 2026-09-27
- **Decided by:** Chrysthofer Arthur Amaro Afonso, in the brief of the first stage, on
  condition that both are maintained

## Context

ZENITH is a full-screen terminal application that must work in Windows Terminal, the
classic Windows console, PowerShell, macOS Terminal, iTerm2 and the common Linux
terminals, written in the Rust that SOLAR pins.

## Decision

`ratatui` for the layout and the widgets, on its `crossterm` backend for the terminal.

The condition was checked on 27 September 2026, before any code was written:

| Crate | Latest release | Activity |
| ----- | -------------- | -------- |
| `ratatui` | 0.30.2, 19 June 2026 | commits on 25 September 2026, a release every few weeks |
| `crossterm` | 0.29.0, 5 April 2025 | commits on 14 September and 21 August 2026, fixes and features merged |

`crossterm` has not released in eighteen months, and its repository is active. That is
maintained, if slowly released. `ratatui` 0.30 depends on `crossterm` 0.29 through
`ratatui-crossterm`, and ZENITH uses the `crossterm` that `ratatui` re-exports, so the
two can never disagree on a version.

## Consequences

`crossterm` is the one backend that covers the classic Windows console as well as the
Unix terminals, which is the reason for it. `ratatui` draws into a buffer and writes only
the cells that changed, so a redraw costs what changed and not the whole screen, and its
`TestBackend` is what makes every screen testable as a snapshot.

If `crossterm` stops being maintained, `ratatui` has other backends, but none of them
covers the classic Windows console, so the question would be reopened rather than
answered by a swap.
