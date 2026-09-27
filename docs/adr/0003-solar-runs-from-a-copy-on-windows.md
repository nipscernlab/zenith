---
status: accepted
date: 2026-09-27
decision-makers: Chrysthofer Arthur Amaro Afonso
---

# On Windows, `solar.exe` runs from a copy keyed by its hash

## Context and Problem Statement

The `solar.exe` on the architect's `PATH` is `target\release\solar.exe` in SOLAR's
repository, and SOLAR's work rebuilds it at the end of every block. Windows cannot replace
an executable while it is running. A ZENITH that ran `solar.exe` in place would therefore
make SOLAR's build fail whenever ZENITH was open, or whenever ZENITH's tests were running.

## Considered Options

* Run `solar.exe` where it was found
* Run a copy of it, under the temporary directory, named by its hash

## Decision Outcome

Chosen option: "Run a copy", because it is the only option under which SOLAR's build can
always replace its own binary.

On Windows, ZENITH never runs `solar.exe` from the folder where it was found. It computes
the SHA-256 of the file, copies it to
`%TEMP%\zenith\solar\<the first 16 hex digits>\solar.exe` unless that copy already
exists, and starts the copy. ZENITH's tests start SOLAR through the same code.

The copy is written under a temporary name and renamed into place, so a second ZENITH
starting at the same moment either finds the whole file or writes its own. Copies of
other hashes are removed when a new one is made; a copy that is running cannot be
removed, which is what keeps a second ZENITH that is still using it safe.

On macOS and Linux a running binary can be replaced, so SOLAR is started where it is.

### Consequences

* Good, because SOLAR's build never fails because of ZENITH, and a new build of SOLAR is
  picked up by reconnecting, `Ctrl+R`, because the hash changes and so does the copy.
* Good, because `system.info` reports the copy as the path of the executable, which is the
  truth, and `/report` and `/version` give both paths, the one found and the one that
  runs, with the hash, so nobody has to guess which build answered.
* Bad, because a copy that cannot be made is a failure: ZENITH reports it with the path
  and the reason, and never falls back to running the original, because that fallback is
  the exact situation this record exists to prevent.
