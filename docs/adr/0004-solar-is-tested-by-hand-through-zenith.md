# 4. SOLAR is tested by hand through ZENITH

- **Status:** Accepted
- **Date:** 2026-09-27
- **Decided by:** Chrysthofer Arthur Amaro Afonso

## Context

SOLAR's automated checks prove the contract. A person still has to use SOLAR, on
Windows, on Linux and on the laboratory Mac, before a version is trusted, and needs a way
to do that which does not change with every API SOLAR adds.

## Decision

SOLAR is tested by hand through ZENITH, interactively, on Windows and Linux, and on the
laboratory Mac by Arthur, a student of the group. Every SOLAR API must be usable from
ZENITH without changing ZENITH, and a test walks every API in the manifest to prove it:
it opens each one as a person would, runs each of its examples through the same path the
keys use, and checks each result against the example.

`docs/TESTING_BY_HAND.md` is the guide, written for someone who has never used Rust.
SOLAR's own guide covers its automated checks and points here for the interactive part.

## Consequences

ZENITH has commands a pure viewer would not need: `/raw` sends a line exactly as typed, so
a tester can see how SOLAR answers a malformed envelope or a batch; `/reconnect` picks up a
new build; `/report` writes the one file a tester sends back.

ZENITH has to show a response that breaks the contract differently from one that keeps
it, which is why every envelope is checked against the contract as it arrives.
