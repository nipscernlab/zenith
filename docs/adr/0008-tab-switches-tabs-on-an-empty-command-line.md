---
status: accepted
date: 2026-09-27
decision-makers: Chrysthofer Arthur Amaro Afonso
---

# `Tab` switches tabs on an empty command line

## Context and Problem Statement

ZENITH has four tabs and a command line that completes with `Tab`. The key that moves
between tabs must reach ZENITH in every terminal, and [ADR 0005](0005-no-action-depends-on-alt.md)
rules out depending on `Alt` or a function key. Which key switches tabs?

## Considered Options

* `Alt` with a digit, or the function keys
* `Ctrl` with a digit
* `Tab` and `Shift+Tab`, switching only when the command line is empty

## Decision Outcome

Chosen option: "`Tab` and `Shift+Tab` on an empty command line", because every terminal
delivers them and none of the other candidates does: `Alt` is not Alt in macOS Terminal,
GNOME Terminal takes `Alt+1` and `F1`, and `Ctrl` with a digit is not delivered by most
terminals at all. On the Session tab `Tab` also completes, so it switches tabs only when
the line is empty, where there is nothing to complete.

### Consequences

* Good, because moving between tabs works the same in every terminal.
* Bad, because a person who presses `Tab` on an empty line expecting completion lands on
  the APIs tab. The help overlay and the first line of `/help` say how the two meanings
  split.

### Confirmation

The key table is the single source for the dispatch and the help overlay, and its tests
check both meanings of `Tab` and that no action depends on `Alt` or a function key.
