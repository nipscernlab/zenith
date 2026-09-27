# Working on ZENITH

This file is the single entry point for anyone who changes this repository, a person or a
coding agent. It says what ZENITH is, where each rule lives, and the one path every change
takes. It links to the documents rather than repeating them, so that there is one copy of
each rule.

## What ZENITH is

The terminal of Constellation, from NIPS-CERN: a full-screen application in which a
person talks to SOLAR, the API at the centre of Constellation. It starts
`solar serve --stdio` as a child process and speaks JSON-RPC 2.0 to it, and it is a client
of SOLAR and only a client ([ADR 0001](docs/adr/0001-a-client-of-solar-and-only-a-client.md)).
SOLAR is tested by hand through it
([ADR 0004](docs/adr/0004-solar-is-tested-by-hand-through-zenith.md)).

## The documents, and what each one decides

| Document | Decides |
| -------- | ------- |
| [`docs/DESIGN.md`](docs/DESIGN.md) | What ZENITH shows and does: the screens, the keys, the commands, the connection, the limits. A change in behaviour is a change here first. |
| [`docs/ADDING_A_FEATURE.md`](docs/ADDING_A_FEATURE.md) | The one path for adding a tab, a command or a view, step by step. |
| [`docs/STYLE.md`](docs/STYLE.md) | How the code, the comments, the messages on screen and the commits are written. |
| [`docs/adr/`](docs/adr/) | What the architect has settled. |
| [`docs/OPEN_QUESTIONS.md`](docs/OPEN_QUESTIONS.md) | What was decided without asking, and what ZENITH needed from SOLAR and did not find. |
| [`docs/TESTING_BY_HAND.md`](docs/TESTING_BY_HAND.md) | How a person who has never used Rust builds both programs and tests SOLAR through ZENITH. |
| [`docs/brand/README.md`](docs/brand/README.md) | ZENITH's mark: its geometry, colours and terminal forms, and where each of the two marks is drawn. |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | Setting up, the tools, the rules about dependencies, opening a pull request. |

SOLAR's contract is `docs/CONTRACT.md` in [nipscernlab/solar](https://github.com/nipscernlab/solar).
It is the only thing ZENITH may assume about SOLAR.

## The rules that are easiest to break

1. **Never read or link SOLAR's source.** What ZENITH needs and SOLAR does not offer goes
   into `docs/OPEN_QUESTIONS.md`, under the first section, and ZENITH does without it.
2. **Never write a fact about SOLAR that you have not verified** against its contract or
   its behaviour on the machine in front of you.
3. **Every action has a plain key or a `Ctrl` key** ([ADR 0005](docs/adr/0005-no-action-depends-on-alt.md)),
   and the help overlay is generated from the key table, so a key is added in one place.
4. **Nothing grows without end** ([ADR 0006](docs/adr/0006-memory-is-bounded.md)): a new
   collection that grows with use is a ring buffer with limits in entries and bytes.
5. **Colour never carries meaning alone**, and every screen is snapshotted in every theme.
6. **On Windows, `solar.exe` runs from a copy**, never where it was built
   ([ADR 0003](docs/adr/0003-solar-runs-from-a-copy-on-windows.md)). Tests go through the
   same code.
7. **Everything is in English**, British spelling, and nothing on screen or in the docs
   uses a dash as punctuation.
8. **Code and documentation move together.** This is a fixed rule for every project of the
   laboratory: every change updates, in the same commit, every document it affects, the
   README, `docs/DESIGN.md`, the guides, `STATUS.md` and `CHANGELOG.md`. An outdated
   document is a defect, like a failing test. CI checks what a machine can: every commit
   that changes code adds to `CHANGELOG.md`, the README's pictures are the snapshots they
   are drawn from, and every row of the table in `docs/TESTING_BY_HAND.md` is typed into
   ZENITH and does what it says. The pull request template lists the rest.
9. **Mutation testing runs only in CI**, weekly. Never run `cargo mutants` on a laptop: it
   copies the whole project for every job, and it filled the disk of the machine this was
   written on once already.

## The one command

```bash
cargo xtask ci
```

It runs what CI runs, in the same order, with the same flags. It is green before every
push. At the end of a block of work, `cargo build --release --locked` leaves the latest
`zenith` in `target/release`, which is on the architect's `PATH`.

## Where the code is

| Path | What it holds |
| ---- | ------------- |
| `crates/zenith-client/` | The SOLAR client: finding and starting `solar`, the protocol, the manifest, the schema validator. No terminal code. |
| `crates/zenith/` | The application: its state, the tabs, the commands, the keys, the themes and the drawing, as a library, and the binary that owns the terminal. |
| `xtask/` | `cargo xtask ci`, the changelog and screenshot checks, the coverage, the measurements, `perf` and `soak`, and the guide done by a machine, `walkthrough`. |
