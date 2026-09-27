---
status: accepted
date: 2026-09-27
decision-makers: Chrysthofer Arthur Amaro Afonso
---

# The command line history survives between sessions

## Context and Problem Statement

In the first stage ZENITH wrote nothing to disk unless asked, so the lines a person typed
on the command line were gone when ZENITH exited, and a tester who ran the same calls every
day typed them again every day. Does the history survive between sessions?

## Considered Options

* The history is kept in memory for the session only, as in the first stage
* The history is kept in a file, between sessions

## Decision Outcome

Chosen option: "Kept in a file", with these conditions:

* in the directory each system keeps per-user data in;
* bounded by a declared number of entries;
* written atomically, so that a crash never corrupts it;
* in a documented format;
* holding only the command lines, never a response;
* with a way to turn it off at the start, and a command to clear it.

### Consequences

* Good, because a session starts where the last one left off, and `↑` brings back what was
  run yesterday.
* Bad, because ZENITH now writes a file without being asked, in a directory of the user's.
  It holds only command lines, which is what a shell's history holds, and it can be turned
  off and cleared.

### Confirmation

`docs/DESIGN.md`, section 12.1, specifies where the file is, its format and how it is
written, and `docs/OPEN_QUESTIONS.md` has what was decided without asking about it: the
names, the flag and the command. Tests write, read, damage and refuse the file on each
system in CI, and the walkthrough of `docs/TESTING_BY_HAND.md` quits the real binary,
starts it again, and finds the last line with `↑`.
