---
status: accepted
date: 2026-09-27
decision-makers: Chrysthofer Arthur Amaro Afonso
---

# No action depends on Alt

## Context and Problem Statement

In macOS Terminal, Option does not act as Alt unless the person changes a setting, so a
key like `Alt+1` types a character there instead. Other terminals keep keys of their own:
`Alt+1` switches tabs in GNOME Terminal, `F1` opens its help, and macOS Terminal keeps
`Home`, `End`, `PgUp` and `PgDn` for its scrollback. Which keys may an action depend on?

## Considered Options

* Any key the platform's convention suggests, `Alt` and the function keys included
* A plain key or a `Ctrl` key for every action, with `Alt` and the function keys as extra
  routes only

## Decision Outcome

Chosen option: "A plain key or a `Ctrl` key for every action", because it is the only one
under which every action works in macOS Terminal as it comes.

Every action has a key with `Ctrl` or a plain key. `Alt` and the function keys are extra
routes where a terminal delivers them, never the only route to anything.

### Consequences

* Good, because tabs are switched with `Tab` and `Shift+Tab`, which every terminal
  delivers, and also with `F1` to `F4` and `Alt+1` to `Alt+4`; scrolling has `Ctrl+B` and
  `Ctrl+F` beside `PgUp` and `PgDn`, and lists have `g` and `G` beside `Home` and `End`.
* Bad, because `Tab` means two things on the Session tab: it completes when the command
  line has text, and switches tabs when it is empty. The help overlay says so, and
  [ADR 0008](0008-tab-switches-tabs-on-an-empty-command-line.md) records that choice.

### Confirmation

The key table in the code is the single source for the dispatch and for the help overlay,
and a test checks that no action is reachable only through `Alt` or a function key.
