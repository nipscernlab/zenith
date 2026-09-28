# Contributing to ZENITH

ZENITH is the terminal of Constellation, and SOLAR is tested by hand through it. It will be
used by people who cannot ask you what you meant, in laboratories that are not this one,
so the rules below are about making that possible rather than about ceremony.

Start with [`AGENTS.md`](AGENTS.md), which says where every rule lives, and read
[`docs/STYLE.md`](docs/STYLE.md) once. To add a tab, a command or a view, follow
[`docs/ADDING_A_FEATURE.md`](docs/ADDING_A_FEATURE.md).

## Setting up

You need Rust. `rust-toolchain.toml` pins the exact version, the same SOLAR pins, so
`rustup` installs it the first time you build.

```bash
git clone https://github.com/nipscernlab/zenith
cd zenith
cargo build
```

You also need a SOLAR to test against. Build it beside ZENITH, and tell the tests where it
is with `ZENITH_SOLAR`; without it, the tests against SOLAR say that they skipped.

```bash
git clone https://github.com/nipscernlab/solar ../solar
(cd ../solar && cargo build --release --locked -p solar-cli)
export ZENITH_SOLAR="$PWD/../solar/target/release/solar"
```

In Windows PowerShell the last line is
`$env:ZENITH_SOLAR = "$PWD\..\solar\target\release\solar.exe"`.

The checks use five tools. Install them once:

```bash
cargo install cargo-nextest cargo-deny cargo-llvm-cov --locked
cargo install typos-cli taplo-cli --locked
```

## The one command

```bash
cargo xtask ci
```

It runs what CI runs, in the same order, with the same flags: formatting, TOML
formatting, spelling, lints, tests, doctests, documentation, the changelog check, the
screenshot check, the guide done in a pseudo-terminal, the supply chain, and the coverage
floor. It runs every step even after one fails, and ends with a table. `cargo xtask ci --fast` skips the coverage, and says so.
A test holds the task and `.github/workflows/ci.yml` together.

**Run it before you push.**

## Code and documentation move together

A fixed rule for every project of the laboratory. Every change updates, in the same
commit, every document it affects: the README, `docs/DESIGN.md`, the guides, `STATUS.md`
and `CHANGELOG.md`. An outdated document is a defect, like a failing test.

CI checks what a machine can:

- **Every commit that changes code adds to `CHANGELOG.md`**, under `## [Unreleased]`.
  `cargo xtask changelog` checks every commit that has not been pushed.
- **The README's pictures are the snapshots.** They are drawn from snapshots of a session
  recorded against a real SOLAR, and `cargo xtask screenshots --check` fails when one
  differs. After changing what a screen draws, review the snapshots with
  `cargo insta review` and redraw the pictures with `cargo xtask screenshots`.
- **The guide for testing by hand says what ZENITH does.** `cargo xtask walkthrough`
  types every row of the table in section 4 of `docs/TESTING_BY_HAND.md` into ZENITH in a
  pseudo-terminal, against SOLAR, and waits for what the row says should happen. A test
  fails when a row has no step, so a row added to the table needs its step in
  `xtask/src/walkthrough.rs`.

The pull request template lists the rest.

## Screens and snapshots

Every screen and state is snapshotted with `insta`, with the colour of every cell, at
80 × 24 and 160 × 48 in each theme, without colour, and in ASCII. A change to what a screen
draws changes its snapshots, and that is the review: look at every changed snapshot with
`cargo insta review` before accepting it. A snapshot accepted without being looked at is a
bug waiting for a user.

## The rules about dependencies

**A runtime dependency must justify itself in its pull request, with numbers.** Measure the
size of the release binary before and after, and the startup with `cargo xtask perf`:

```bash
cargo build --release --locked -p zenith && ls -l target/release/zenith
```

Put both figures in the description. The one time this was measured before a decision,
the `jsonschema` crate would have taken the binary from 775 KB to 4.6 MB, and ZENITH has a
validator of its own instead, held by property tests to `jsonschema`'s verdicts.

**Development dependencies are free.** A test runner, a snapshot library, a
pseudo-terminal for the measurements: none of them ship.

`cargo deny check` refuses a licence that is not on the list, a source that is not
crates.io, and anything with a RustSec advisory against it.

## The measurements

```bash
cargo xtask perf
cargo xtask soak
cargo bench -p zenith --bench draw
```

`perf` starts the release binary ten times in a pseudo-terminal against `ZENITH_SOLAR`,
types two hundred keys into it, leaves it idle for a minute after its first minute, and
times two thousand `solar.ping` through ZENITH's connection code with SOLAR at `trace` and
at `off`. `soak` types a hundred thousand `/ping` into it and samples its memory every ten
thousand. The bench draws each tab and the help overlay in process, at two sizes. The
figures go to `target/perf/`, and the ones that matter to `STATUS.md`.

On Windows the pseudo-terminal is ConPTY, which asks for the position of the cursor
before it starts the program and waits for the answer; the harness answers, as a terminal
would.

## Mutation testing, in CI only

Coverage says which lines ran; mutation testing says whether anything checked what they
did. It runs weekly in `.github/workflows/scheduled.yml`, and whenever it is started by
hand from the Actions tab or with `gh workflow run scheduled.yml`, with the configuration
in `.cargo/mutants.toml`. The mutants are split into twenty shards; the job called
`mutation score` adds them up with `cargo xtask mutants`, shows the report on the run,
with every survivor by file, and keeps it as the artefact `mutants-report`. It does not
gate yet: the ceiling is set from the first run that finishes all twenty shards, as
`docs/OPEN_QUESTIONS.md` says.

To read a run here without running anything:

```bash
gh run download <run id> --pattern 'mutants-shard-*' --dir mutants-shards
cargo xtask mutants mutants-shards --shards 20
```

To run some shards again, for example one whose runner went down, name them; the summary
of such a run fails, because a part of the mutants is not a measurement of all of them:

```bash
gh workflow run scheduled.yml -f shards='[14]'
```

Every process of the mutation job may take six GiB of address space. A mutant that takes
away one of ZENITH's limits on memory then fails its test, where without the limit it took
all of the runner's memory and the runner with it.

**Never run `cargo mutants` locally.** It rebuilds the project for every mutant, and on
the machine ZENITH was written on a copy of the project per job filled the disk once
already.

## Reporting a bug

In ZENITH, `/report` writes one file with the versions of ZENITH and SOLAR, the system as
`system.info` reports it, the recent log and the calls of the session. Attach it to the
report. [`docs/TESTING_BY_HAND.md`](docs/TESTING_BY_HAND.md) says what else to send.

## Opening a pull request

Commits follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/), one
per finished and tested step, with a body that says **why**. See `docs/STYLE.md`.

Chrysthofer Arthur Amaro Afonso reviews everything: `CODEOWNERS` says so, and he is the
architect of ZENITH. Write the description for him, and for whoever reads it in two years.

## Reporting something exploitable

Not here. [`SECURITY.md`](SECURITY.md) says where.
