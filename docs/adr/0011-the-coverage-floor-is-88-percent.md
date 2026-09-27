---
status: accepted
date: 2026-09-27
decision-makers: Chrysthofer Arthur Amaro Afonso
---

# The coverage floor is 88 % of lines, and it only rises

## Context and Problem Statement

Coverage is a list of the lines no test has ever run, and its value is that it cannot
fall unnoticed. CI measures it on every change. Where is the floor, and what may move it?

## Considered Options

* A round number chosen in advance
* The coverage measured when the floor was set, rounded down, raised by whoever raises
  the coverage and lowered by nobody

## Decision Outcome

Chosen option: "The measured coverage, rounded down", because a floor above what the tests
reach fails every change, and one far below it allows the coverage to fall.

On 27 September 2026 the tests ran 88.46 % of the lines of `zenith-client` and `zenith`,
measured on Windows with SOLAR 0.2.0 present, leaving out the test double, `main.rs` and
`xtask`. The floor is 88 %. CI measures the same on Linux, where the code for Windows is
not compiled and the code for Unix is.

### Consequences

* Good, because a change that leaves new code without tests fails in CI.
* Bad, because the floor is close to the coverage, so a change that adds code must add
  its tests in the same pull request.

### Confirmation

`FLOOR` in `xtask/src/coverage.rs`, which `cargo xtask coverage` and the `coverage` job of
CI enforce with `--fail-under-lines`.
