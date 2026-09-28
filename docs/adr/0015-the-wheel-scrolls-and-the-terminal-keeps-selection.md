---
status: accepted
date: 2026-09-27
decision-makers: Chrysthofer Arthur Amaro Afonso
---

# The wheel scrolls, and the terminal keeps selection with a modifier

This record supersedes [ADR 0013](0013-no-mouse-for-now.md).

## Context and Problem Statement

[ADR 0013](0013-no-mouse-for-now.md) left the mouse to the terminal so that selecting text
worked as usual. Testing ZENITH by hand on 27 September 2026, the architect found what that
costs: on the alternate screen a terminal turns the wheel into arrow keys, so the wheel
walked the command history instead of scrolling the page. That cannot ship. Record 0013
asked that a record superseding it say how selection is kept, because a person testing
SOLAR copies JSON out of the screen all the time.

How does ZENITH make the wheel scroll and still let text be selected?

## Considered Options

* Leave the mouse to the terminal, as 0013 decided
* Take the mouse for the wheel only, and leave selecting to the terminal's own modifier,
  with a way to give the mouse back
* Take the mouse, and select text in ZENITH itself

## Decision Outcome

Chosen option: "Take the mouse for the wheel only", because every terminal that reports
the mouse to a program also has a way to select text over it, and ZENITH has no business
redoing what the terminal already does well. Selection, copying and the clipboard stay the
terminal's.

* ZENITH asks the terminal for its buttons, the wheel's notches among them, and not for
  movement, which it has no use for and which would only wake it.
* A notch of the wheel scrolls the view under the pointer: the transcript, the completion
  menu, the list of APIs and the entry beside it, the Log, the History, and the help or
  the viewer from anywhere while one is open. `↑` and `↓` keep walking the command history;
  the wheel never does. `PgUp` and `PgDn` stay as they are.
* Clicks do nothing, in this version.
* Text is selected with the terminal's modifier held: `Shift` in Windows Terminal, in WSL
  in it and in most Linux terminals, `Option` in iTerm2. `docs/DESIGN.md`, section 11, has
  each terminal's, with where it is documented.
* The mouse can be given back: `/mouse off` and `/mouse on` while ZENITH runs, `/mouse`
  alone to switch, and `--no-mouse` or `ZENITH_NO_MOUSE` at the start. ZENITH gives it back
  with the rest of the terminal when it exits, on a signal and after a panic.

### Consequences

* Good, because the wheel scrolls, as the architect needs, and the history is walked only
  by the keys that walk it.
* Good, because selection is still the terminal's, with the terminal's own clipboard and
  its own rules for what a selection is.
* Bad, because selecting needs a key held, and the key differs between terminals; the help
  and the notice of `/mouse` say the common ones, and `/mouse off` works in every
  terminal.
* Bad, because in the classic Windows console, taking the mouse also turns off its Quick
  Edit selection while ZENITH has it; `/mouse off` and `--no-mouse` give it back.

### Confirmation

Tests of the state machine scroll every view with the wheel, check that nothing scrolls
past its end and that the history is never walked; a test starts the real binary in a
pseudo-terminal on Unix, checks the mouse modes it asks for, scrolls the help with the
wheel, gives the mouse back with `/mouse off` and on quitting; and the walkthrough of
`docs/TESTING_BY_HAND.md` does the same in the real binary on Windows, Linux and macOS.
Selecting with the modifier is the terminal's, and a person checks it with the row of the
guide that names it.
