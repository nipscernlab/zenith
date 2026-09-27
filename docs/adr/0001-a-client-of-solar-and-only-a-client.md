---
status: accepted
date: 2026-09-27
decision-makers: Chrysthofer Arthur Amaro Afonso, architect of SOLAR, ZENITH and Constellation
---

# ZENITH is a client of SOLAR and only a client

## Context and Problem Statement

SOLAR is the single boundary of Constellation: every interface talks to it and to nothing
else. ZENITH is one of those interfaces, the box drawn as SOLAR.CLI on the architecture
board. An interface written by the people who wrote SOLAR is tempting to build on SOLAR's
own crates, and doing so would hide every place where the protocol is not enough. How
does ZENITH reach SOLAR?

## Considered Options

* A client of SOLAR's protocol, starting `solar serve --stdio` as any interface would
* An interface built on SOLAR's own crates

## Decision Outcome

Chosen option: "A client of SOLAR's protocol", because it is the only way for ZENITH to
prove that the protocol is enough.

ZENITH starts `solar serve --stdio` as a child process and speaks JSON-RPC 2.0 to it, one
message per line, exactly as any other interface would. It never links a SOLAR crate and
never reads SOLAR's source. Everything it shows about the APIs comes from SOLAR at run
time, starting with `solar.manifest`.

When ZENITH needs something SOLAR does not offer, that is a missing SOLAR API. It is
recorded in `docs/OPEN_QUESTIONS.md`, and ZENITH does without it rather than working
around it.

ZENITH starts no program other than `solar`, and has no AI features in this stage.

### Consequences

* Good, because whatever ZENITH can do, any interface can, and the gaps it finds are
  listed where the architect reads them instead of being papered over by a private
  channel.
* Good, because a new SOLAR API appears in ZENITH with no change to ZENITH: the
  catalogue, the completion and the forms are built from the manifest.
* Bad, because ZENITH can only be as good as the manifest: a layout that needs a fact the
  manifest does not state cannot be drawn from it.
