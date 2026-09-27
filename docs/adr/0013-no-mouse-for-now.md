---
status: accepted
date: 2026-09-27
decision-makers: Chrysthofer Arthur Amaro Afonso
---

# No mouse, for now

## Context and Problem Statement

A terminal application can capture the mouse to make tabs, lists and fields clickable.
Does ZENITH?

## Considered Options

* Capture the mouse
* No mouse, with every action on a key

## Decision Outcome

Chosen option: "No mouse", because capturing the mouse takes text selection away from the
terminal, and a person testing SOLAR copies JSON out of the screen all the time. Every
action has a key.

### Consequences

* Good, because selecting and copying text works as it does in any terminal.
* Bad, because nothing can be clicked.

## More Information

Decided for now: the question may be reopened, and a record that superseded this one
would say how selection is kept.
