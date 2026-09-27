# 6. Every collection is bounded, and says what it dropped

- **Status:** Accepted
- **Date:** 2026-09-27
- **Decided by:** Chrysthofer Arthur Amaro Afonso

## Context

A terminal stays open for hours, and a person testing SOLAR may make tens of thousands of
calls in one session. Anything that keeps every call, every log line or every response
forever will eventually take the machine with it.

## Decision

The History and the Log are ring buffers with declared limits, in entries and in bytes.
When an old entry is dropped, the tab says so. The Session transcript and the command
line history follow the same rule, because nothing in ZENITH may grow without end.

Exporting writes to the file as it goes, never by building the file in memory first.

The memory is measured at idle and after a scripted session of one hundred thousand
calls, sampled as it runs, and the result is recorded in `STATUS.md`.

## Consequences

A long session loses its oldest calls from the History, and the History says how many.
The limits are in `docs/DESIGN.md`, section 13, with the reasons for their sizes.
