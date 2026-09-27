---
status: accepted
date: 2026-09-27
decision-makers: Chrysthofer Arthur Amaro Afonso
---

# Every collection is bounded, and says what it dropped

## Context and Problem Statement

A terminal stays open for hours, and a person testing SOLAR may make tens of thousands of
calls in one session. Anything that keeps every call, every log line or every response
forever will eventually take the machine with it.

## Considered Options

* Keep everything for the life of the session
* Ring buffers with declared limits, in entries and in bytes, that say what they dropped

## Decision Outcome

Chosen option: "Ring buffers with declared limits", because nothing in ZENITH may grow
without end.

The History and the Log are ring buffers with declared limits, in entries and in bytes.
When an old entry is dropped, the tab says so. The Session transcript and the command
line history follow the same rule.

Exporting writes to the file as it goes, never by building the file in memory first.

### Consequences

* Good, because the memory of a long session stays level once the buffers are full.
* Bad, because a long session loses its oldest calls from the History, and the History
  says how many. The limits are in `docs/DESIGN.md`, section 13, with the reasons for
  their sizes.

### Confirmation

The memory is measured at idle and after a scripted session of one hundred thousand
calls, sampled as it runs, by `cargo xtask soak`, and the result is recorded in
`STATUS.md`. A test holds the tables of limits in the documents to the constants of the
code.
