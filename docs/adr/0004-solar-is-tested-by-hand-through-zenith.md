---
status: accepted
date: 2026-09-27
decision-makers: Chrysthofer Arthur Amaro Afonso
---

# SOLAR is tested by hand through ZENITH

## Context and Problem Statement

SOLAR's automated checks prove the contract. A person still has to use SOLAR, on
Windows, on Linux and on the laboratory Mac, before a version is trusted, and needs a way
to do that which does not change with every API SOLAR adds.

## Considered Options

* Test SOLAR by hand through ZENITH, on every system, with one guide for everyone

## Decision Outcome

Chosen option: "Through ZENITH", because it is the interface that shows every API from
the manifest without being changed for it.

SOLAR is tested by hand through ZENITH, interactively, on Windows and Linux, and on the
laboratory Mac by Arthur, a student of the group. Every SOLAR API must be usable from
ZENITH without changing ZENITH, and a test walks every API in the manifest to prove it:
it opens each one as a person would, runs each of its examples through the same path the
keys use, and checks each result against the example.

`docs/TESTING_BY_HAND.md` is the guide, written for someone who has never used Rust.
SOLAR's own guide covers its automated checks and points here for the interactive part.

### Consequences

* Good, because ZENITH has what a tester needs and a pure viewer would not: `/raw` sends a
  line exactly as typed, so a tester can see how SOLAR answers a malformed envelope or a
  batch; `/reconnect` picks up a new build; `/report` writes the one file a tester sends
  back.
* Bad, because ZENITH has to show a response that breaks the contract differently from
  one that keeps it, so every envelope is checked against the contract as it arrives.

### Confirmation

`cargo xtask walkthrough` types every row of the table in `docs/TESTING_BY_HAND.md` into
the real binary, in CI on the three systems, and a test holds the table and the steps
together.
