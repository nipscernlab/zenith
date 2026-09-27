---
status: accepted
date: 2026-09-27
decision-makers: Chrysthofer Arthur Amaro Afonso
---

# The handshake waits fifteen seconds

## Context and Problem Statement

ZENITH connects by asking SOLAR for `solar.version` and `solar.manifest`, and a SOLAR that
never answers must be reported as such. SOLAR answers the handshake in a few milliseconds
on the machine ZENITH was written on. How long does ZENITH wait?

## Considered Options

* A few hundred milliseconds, close to what SOLAR needs
* Fifteen seconds

## Decision Outcome

Chosen option: "Fifteen seconds", because on Windows the first run of `solar.exe` can take
far longer than the next: on that machine the first of ten starts spent 475 ms reading
SOLAR's binary and starting it, against about 5 ms for the others, which looks like the
antivirus scanning the file, and a build seen for the first time may take longer still. A
ZENITH that gave up during such a start would report a failure that is not one.

### Consequences

* Good, because a slow first start is never reported as a failure.
* Bad, because a SOLAR that hangs before answering is reported only after fifteen
  seconds. The opening says that ZENITH is connecting meanwhile, and any key skips it.

### Confirmation

A test drives a handshake that gets no answer and checks that it fails after fifteen
seconds and not before.
