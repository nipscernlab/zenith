# Reporting a vulnerability in ZENITH

Write to **chrysthofer.afonso@cern.ch**. Say what you found, what it lets someone do, and
how to reproduce it. A proof of concept, however rough, is worth more than a careful
description.

Please do not open a public issue for something exploitable. There is no embargo policy
to negotiate and no bounty to claim: this is a laboratory, the fix will be written as soon
as it is understood, and you will be credited in `CHANGELOG.md` unless you would rather
not be.

You should get an answer within five working days. If you do not, write again to
chrysthofer.afonso@cern.ch and copy the group: Chrysthofer Arthur Amaro Afonso is the
technical coordinator of NIPS-CERN, and Prof. Luciano Manhães de Andrade Filho heads the
group.

## What is in scope

- The `zenith` binary and the crates of this repository.
- Anything a response from SOLAR can make ZENITH do beyond drawing it: write a file it was
  not asked to write, start a program, or leave the terminal in a state the person has to
  repair by hand.
- Escape sequences inside what SOLAR returns. ZENITH replaces every control character in
  text that comes from SOLAR with a visible escape, such as `\u{1b}`, before it reaches the
  screen, so a response cannot drive the terminal. A way around that is a vulnerability.
- The copy of `solar.exe` that ZENITH makes on Windows, if it can be made to run something
  other than the file it hashed.

## What is not, yet

- SOLAR itself, which has its own `SECURITY.md`, and the other projects of the laboratory.
  The same address reaches the same person.

## Versions

ZENITH is `0.1.0`. Until there are releases, the fix goes on `main` and into the next tag.
