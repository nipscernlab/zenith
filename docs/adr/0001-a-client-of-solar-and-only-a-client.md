# 1. ZENITH is a client of SOLAR and only a client

- **Status:** Accepted
- **Date:** 2026-09-27
- **Decided by:** Chrysthofer Arthur Amaro Afonso, architect of SOLAR, ZENITH and
  Constellation

## Context

SOLAR is the single boundary of Constellation: every interface talks to it and to nothing
else. ZENITH is one of those interfaces, the box drawn as SOLAR.CLI on the architecture
board. An interface written by the people who wrote SOLAR is tempting to build on SOLAR's
own crates, and doing so would hide every place where the protocol is not enough.

## Decision

ZENITH starts `solar serve --stdio` as a child process and speaks JSON-RPC 2.0 to it, one
message per line, exactly as any other interface would. It never links a SOLAR crate and
never reads SOLAR's source. Everything it shows about the APIs comes from SOLAR at run
time, starting with `solar.manifest`.

When ZENITH needs something SOLAR does not offer, that is a missing SOLAR API. It is
recorded in `docs/OPEN_QUESTIONS.md`, and ZENITH does without it rather than working
around it.

ZENITH starts no program other than `solar`, and has no AI features in this stage.

## Consequences

ZENITH is the proof that the protocol is enough: whatever it can do, any interface can.
The gaps it finds are listed where the architect reads them, and none of them is papered
over by a private channel.

A new SOLAR API appears in ZENITH with no change to ZENITH, because the catalogue, the
completion and the forms are built from the manifest. The cost is that ZENITH can only be
as good as the manifest: a layout that needs a fact the manifest does not state cannot be
drawn from it.
