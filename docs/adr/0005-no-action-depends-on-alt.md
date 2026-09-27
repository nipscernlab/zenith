# 5. No action depends on Alt

- **Status:** Accepted
- **Date:** 2026-09-27
- **Decided by:** Chrysthofer Arthur Amaro Afonso

## Context

In macOS Terminal, Option does not act as Alt unless the person changes a setting, so a
key like `Alt+1` types a character there instead. Other terminals keep keys of their own:
`Alt+1` switches tabs in GNOME Terminal, `F1` opens its help, and macOS Terminal keeps
`Home`, `End`, `PgUp` and `PgDn` for its scrollback.

## Decision

Every action has a key with `Ctrl` or a plain key. `Alt` and the function keys are extra
routes where a terminal delivers them, never the only route to anything.

## Consequences

Tabs are switched with `Tab` and `Shift+Tab`, which every terminal delivers, and also
with `F1` to `F4` and `Alt+1` to `Alt+4`. Scrolling has `Ctrl+B` and `Ctrl+F` beside
`PgUp` and `PgDn`, and lists have `g` and `G` beside `Home` and `End`.

`Tab` therefore means two things on the Session tab: it completes when the command line
has text, and switches tabs when it is empty. The help overlay says so.

The key table in the code is the single source for the dispatch and for the help overlay,
and a test checks that no action is reachable only through `Alt` or a function key.
