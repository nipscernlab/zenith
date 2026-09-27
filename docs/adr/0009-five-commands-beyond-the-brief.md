---
status: accepted
date: 2026-09-27
decision-makers: Chrysthofer Arthur Amaro Afonso
---

# Five commands beyond the eight the brief names

## Context and Problem Statement

The brief of the first stage names eight commands: `/list`, `/describe`, `/call`,
`/ping`, `/version`, `/theme`, `/help` and `/quit`. Testing SOLAR by hand needs a few
things none of them does. Does ZENITH have more?

## Considered Options

* The eight commands and no others
* The eight, and `/raw`, `/reconnect`, `/clear`, `/export` and `/report`

## Decision Outcome

Chosen option: "The eight and five more", because testing SOLAR by hand needs them.
`/report` was added to the brief later. `/raw` is the only way to send SOLAR something
malformed and see its answer; `/reconnect` picks up a new build of SOLAR without leaving
ZENITH; `/clear` empties the transcript while the History keeps everything; `/export`
writes the History to a file.

### Consequences

* Good, because a tester can reach every behaviour of SOLAR, the broken envelopes and the
  batches included, from ZENITH.
* Bad, because there are thirteen commands to learn rather than eight. `/help` lists
  them, and completion offers them after `/`.

### Confirmation

`docs/DESIGN.md`, section 10, has the table of commands, and a test parses every command of
the code's table from its usage. The tests against SOLAR run `/raw`, `/export` and
`/report`, and the walkthrough of `docs/TESTING_BY_HAND.md` runs `/raw` and `/clear` in
the real binary.
