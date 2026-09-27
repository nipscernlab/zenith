---
status: accepted
date: 2026-09-27
decision-makers: Chrysthofer Arthur Amaro Afonso
---

# ZENITH has its own schema validator

## Context and Problem Statement

The brief asks for the parameters of a call to be validated against the API's schema
before they are sent, with the error shown where it happened. SOLAR's schemas are JSON
Schema 2020-12 fragments that share the `$defs` of the manifest. What validates them?

## Decision Drivers

* The size of the binary every tester builds and runs
* Verdicts that agree with a complete implementation of JSON Schema 2020-12

## Considered Options

* The `jsonschema` crate, the most complete validator in Rust
* A validator of ZENITH's own for the assertions SOLAR's schemas use, held to
  `jsonschema`'s verdicts by tests

## Decision Outcome

Chosen option: "A validator of ZENITH's own", because adding `jsonschema` took the release
binary of an early build of ZENITH from 775 168 bytes to 4 598 272, six times the size,
measured on 27 September 2026 with the release profile of this repository, and most of
that is regular expressions and format checkers that SOLAR's schemas do not use.

The validator implements the assertions of JSON Schema 2020-12, with `$ref` resolved into
the manifest's `$defs`. `jsonschema` is a development dependency and never ships.

### Consequences

* Good, because the release binary stays near its size without a validator.
* Bad, because ZENITH maintains a validator. A keyword it does not implement, such as
  `pattern`, is reported as not checked and never fails a call, which
  `docs/OPEN_QUESTIONS.md` records.

### Confirmation

Property tests give both validators 4 096 generated schemas with generated values, and
every schema of a real manifest with values shaped like its members, and fail on any
verdict on which they differ: `crates/zenith-client/tests/validator_agrees_with_jsonschema.rs`.
