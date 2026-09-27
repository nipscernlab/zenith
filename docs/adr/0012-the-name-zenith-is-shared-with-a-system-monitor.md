---
status: accepted
date: 2026-09-27
decision-makers: Chrysthofer Arthur Amaro Afonso
---

# The name `zenith` is shared with a system monitor

## Context and Problem Statement

A system monitor written in Rust, `bvaisvil/zenith`, installs a binary called `zenith`
too. The brief names the command `zenith`. Does ZENITH keep the name?

## Considered Options

* Keep `zenith`, and make it easy to tell the two apart
* Rename the binary

## Decision Outcome

Chosen option: "Keep `zenith`", because the brief keeps the name.

### Consequences

* Good, because the command is the name of the program.
* Bad, because a person who has both runs whichever comes first on the `PATH`. They tell
  them apart this way:
  * `zenith --version` prints `ZENITH <version>, the terminal of Constellation,
    NIPS-CERN`; the monitor prints its own name in lower case with its own version.
  * `zenith --help` starts with the same line and names SOLAR.
  * `where zenith` in Windows, `which -a zenith` in macOS and Linux, lists every `zenith`
    in the order of the `PATH`.
  * A person who needs both can put `target/release` of this repository first on the
    `PATH`, or call the other by its full path.

## More Information

Revisit when the name has to be published somewhere names are unique, such as
crates.io, where `zenith` is taken.
