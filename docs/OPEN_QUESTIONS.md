# Open questions

Everything here was decided without asking, because the brief said to take the most
conservative option and write down why. Each entry says what was chosen, what it rules
out, and what would make it worth revisiting.

A decision the architect confirms becomes a record in [`adr/`](adr/) and leaves this
file, so that this file is only ever the list of things still open to being overruled.

The first section is different in kind: it is what ZENITH needed from SOLAR and did not
find. Those are not ZENITH's to decide.

## What ZENITH needed from SOLAR and did not find

### The manifest does not say whether batches are accepted

SOLAR's contract adds batches to `solar/1` in section 3.2 and limits them to 64
elements. The manifest says neither: there is no member that tells a client this build
answers batches, nor what its limit is, and the protocol stayed `solar/1` when they
arrived. SOLAR 0.1.0 answers a batch with `UNIMPLEMENTED` / `BATCH_NOT_SUPPORTED`, and
SOLAR 0.2.0 with an array, which `cargo xtask walkthrough` saw on 27 September 2026,
both under the same protocol name.

**Chosen.** ZENITH sends no batches of its own. Finding out by sending a probe batch
would be working around a missing declaration, which ADR 0001 rules out. A person can
still send one by hand with `/raw [...]`, and ZENITH shows the answer as one card per
element, which is how batches are tested by hand.

**What would close it.** A member of the manifest, at its root, that declares the batch
support and its limit, for example `"batch": {"max_elements": 64}`, with a minor bump of
`schema_version`. ZENITH would then run the examples of an API as one batch with `a`,
which is the use it has for them.

### The log level cannot be changed while SOLAR runs

SOLAR reads `SOLAR_LOG` once, when it starts. The Log tab must show standard error "at a
level chosen in the interface", and there is no API to tell SOLAR a new level.

**Chosen.** ZENITH starts SOLAR at `trace` and filters in the interface, so choosing a
level is instant and never restarts SOLAR. `--solar-log <level>` starts it lower, for
whoever wants SOLAR to do less work; the tab then says what SOLAR was started with.

**The cost.** At `trace`, SOLAR writes every line it reads and every line it writes to
standard error, so the pipe carries each message twice. The cost per call is measured in
`STATUS.md`.

**What would close it.** An API such as `solar.set_log_level`, or a documented statement
that the level is fixed for the life of a session.

### The recording format of `solar serve --record` is not documented

The History must be exportable as a recording `solar replay` accepts "if SOLAR's
recording format is documented". SOLAR's README says a recording is NDJSON with every
line in, every line out and the time each crossed, and nothing more: no member names, no
version, no promise that the format will not change.

**Chosen.** ZENITH exports in a format of its own, specified in `docs/DESIGN.md`,
section 12. Writing SOLAR's format from what a recording looks like on this machine would
be inventing a fact about SOLAR from an observation.

**What would close it.** A section of SOLAR's contract, or a document beside it, that
specifies the recording, with a version. The exporter is one function.

## Naming

### The name `zenith` is shared with a system monitor

A system monitor written in Rust, `bvaisvil/zenith`, installs a binary called `zenith`
too. The brief keeps the name.

**How a person who has both tells them apart:**

- `zenith --version` prints `ZENITH 0.1.0, the terminal of Constellation, NIPS-CERN`.
  The monitor prints its own name in lower case with its own version.
- `zenith --help` starts with the same line and names SOLAR.
- Which one runs is decided by the order of the `PATH`. `where zenith` in Windows,
  `which -a zenith` in macOS and Linux, lists every `zenith` in that order.
- A person who needs both can put `target/release` of this repository first on the
  `PATH`, or call the other by its full path.

**Revisit when** the name has to be published somewhere names are unique, such as
crates.io, where `zenith` is taken.

### ZENITH has no mark of its own

The brief says a mark will come later. ZENITH writes its name in text, and draws only the
SOLAR mark, in the opening, exactly as SOLAR's brand rules say.

## The interface

### `Tab` switches tabs when the command line is empty

Every terminal delivers `Tab` and `Shift+Tab`, and none of the other candidates does:
`Alt` is not Alt in macOS Terminal, GNOME Terminal takes `Alt+1` and `F1`, and `Ctrl`
with a digit is not delivered by most terminals at all. On the Session tab `Tab` also
completes, so it switches tabs only when the line is empty, where there is nothing to
complete.

**The cost.** A person who presses `Tab` on an empty line expecting completion lands on
the APIs tab. The help overlay and the first line of `/help` say how the two meanings
split.

### `Enter` never accepts a completion

`Tab` inserts the highlighted candidate; `Enter` runs the line exactly as typed, even
while the menu is open. Claude Code accepts a slash command with `Enter`, and ZENITH also
completes parameter names inside JSON, where `Enter` accepting a candidate would run
something that is not on screen.

### A line without a slash is not run

A line that starts with an API name gets `Commands start with a slash. Did you mean /call
solar.ping?` rather than being run as a call. Running it would be a guess about what the
person meant, and the guess would be wrong the first time an API is named like a word.

### Five commands beyond the eight the brief names

`/raw`, `/reconnect`, `/clear`, `/export` and `/report`. `/report` was added to the brief
later; the other four exist because testing SOLAR by hand needs them: `/raw` is the only
way to send SOLAR something malformed and see its answer, and `/reconnect` picks up a new
build of SOLAR without leaving ZENITH.

### Nothing is written to disk unless asked

The command line history and the chosen theme are lost when ZENITH exits. Writing them
would mean choosing a configuration directory on three systems and a format for it, and
the brief asks for neither.

**Revisit when** testers ask for it; the natural places are the platform configuration
directories, `%APPDATA%`, `~/Library/Application Support` and `$XDG_CONFIG_HOME`.

### No mouse

Capturing the mouse takes text selection away from the terminal, and a person testing
SOLAR copies JSON out of the screen all the time. Every action has a key.

### The time connected shows seconds only in the first minute

After the first minute it shows minutes, so that an idle ZENITH wakes once a minute rather
than once a second. The idle measurement in `STATUS.md` depends on this.

### `Ctrl+C` clears the command line before it quits

When no call is in flight and the line has text, the first `Ctrl+C` clears it and arms
the second, which quits. That is what shells and Claude Code do, and it makes `Ctrl+C`
never quit on the first press.

## Validation

### ZENITH has its own schema validator

The brief asks for parameters to be validated against the schema before they are sent.
The `jsonschema` crate is the most complete validator in Rust, and adding it took the
release binary of an early build of ZENITH from 775 168 bytes to 4 598 272, six times the
size, measured on 27 September 2026 with the release profile of this repository. Most of
that is regular expressions and format checkers that SOLAR's schemas do not use.

**Chosen.** A validator of ZENITH's own for the assertions of JSON Schema 2020-12, with
`$ref` resolved into the manifest's `$defs`. It is tested against `jsonschema`, which is a
development dependency and never ships: every schema of the manifest, with generated
values, must get the same verdict from both.

### A keyword ZENITH does not check never fails a call

When a schema uses a keyword the validator does not implement, such as `pattern`,
ZENITH says that it did not check it and that SOLAR will, and sends the call. A tool that
tests SOLAR must never refuse what SOLAR would accept.

## The connection

### The handshake waits fifteen seconds

SOLAR answers the handshake in a few milliseconds on this machine, and fifteen seconds is
far more than it needs. It is that long because on Windows the first run of `solar.exe`
can take far longer than the next: the first of ten starts spent 475 ms reading SOLAR's
binary and starting it, against about 5 ms for the others, which looks like the
antivirus scanning the file, and a build seen for the first time may take longer still.
A ZENITH that gave up during such a start would report a failure that is not one.

### A response with `id: null` belongs to the oldest request waiting

The contract answers with `id: null` only when a request was too broken to have an id.
ZENITH never sends one except through `/raw`, and SOLAR answers in order apart from the
three answers section 9.1 of its contract sends at once, to `solar.cancel`, to a request
that finds the queue full and to an `id` already in flight, all of which carry an `id`.
So the oldest request still waiting is the one an `id: null` answers.

### `max_output_bytes` is shown, and not required

Section 8.3 of SOLAR's contract gives every API a `max_output_bytes`, and the manifest
layout stayed `2.0.0` when it arrived, so the manifest of SOLAR 0.1.0, which has none, is
as valid under that layout as one that has it. ZENITH shows the limit on the API's card
when the API declares it, and does not count its absence against the contract, as it does
for the members that were there from the start.

**What would change it.** A minor bump of `schema_version` whenever an entry gains a
member: ZENITH would then require it from the version that brought it.

### The Windows copy is named by the first sixteen hex digits of its SHA-256

Sixty-four bits make an accidental collision a matter of one in eighteen quintillion,
and the name stays short enough to read in a path. The copy lives under `%TEMP%`, which
the system cleans on its own schedule.

## Colour and characters

### The classic Windows console is drawn with 256 colours

The console of Windows 10 and 11 accepts 24-bit colour sequences, but nothing tells a
program which console it is in except the absence of `WT_SESSION`, and older consoles
approximate. The xterm 256-colour cube is accepted by all of them, and the brand gives a
256-colour value for gold. `--color truecolor` is there for whoever knows better.

### Muted text and errors are mixes of the palette

The brand has five colours, and an interface needs a few more roles: muted text, borders,
a selection, errors. Each of those is a mix of two palette colours, and the contrast of
every one of them is computed by a test. Errors in the dark themes are a mix of copper
and gold, because copper alone on night is 4.07:1, below the 4.5:1 that normal text
needs; they are told from the accent by the word `error` and the badge, never by colour.

### The light theme is red in sixteen colours

The brand gives a sixteen-colour value for gold, `ESC[33m`, and none for copper. Yellow
on a light background is unreadable in most sixteen-colour palettes, and the copper of the
light theme has no sixteen-colour counterpart, so the light theme's accent and the mark
are red, `ESC[31m`, at that depth. At every other depth they are the brand's copper.

**Revisit when** the brand gives copper a sixteen-colour value.

### The flag is `--color`, spelt the American way

Every command line tool spells it so, `git`, `cargo` and `ls` among them, and so does
`NO_COLOR`. The prose around it is British; the flag is an identifier a person already
knows, and `typos.toml` says so.

### `--color` beats `NO_COLOR`, and `NO_COLOR` beats `ZENITH_COLOR`

no-color.org asks that `NO_COLOR` be overridden only by a per-invocation argument. A
variable of ZENITH's own is set once and forgotten, so it does not override a standard
the person may have set on purpose.

### Unicode is limited to what the classic console has

Rounded corners, check marks and emoji draw as boxes in the fonts the classic Windows
console ships with. ZENITH draws only box drawing, blocks and the few shapes of code page
437. ASCII is a switch, `--ascii`, and not a guess, for the reason SOLAR gives: no
terminal can be asked reliably whether it has a glyph.

## Memory

### The limits

| Collection | Entries | Bytes | Why this size |
| ---------- | ------- | ----- | ------------- |
| History | 10 000 calls | 32 MiB | A day of testing by hand is a few thousand calls; 32 MiB holds ten thousand typical calls with room for large responses |
| Log | 20 000 lines | 16 MiB | At `trace`, two lines per call and a few more: the last several thousand calls |
| Transcript | 1 000 entries | 16 MiB | Nobody scrolls back further than that on screen; the History keeps more |
| Command line history | 500 lines | 256 KiB | A convenience |
| Calls in flight | 256 | | More would be a script, not a person |

They are constants in `crates/zenith/src/limits.rs`, each with this reason beside it.

### The soak judges growth against the figure at twenty thousand calls

By twenty thousand calls every ring buffer is full, the History at ten thousand calls and
the Log at twenty thousand lines, so from there on the memory can only stay level.
`cargo xtask soak` fails when its last figure is more than a fifth above that one; the
fifth is room for the allocator, which does not hand pages back at once.

## Measuring and testing

### The coverage floor is 88 % of lines

On 27 September 2026 the tests ran 88.46 % of the lines of `zenith-client` and `zenith`,
measured on Windows with SOLAR 0.2.0 present, leaving out the test double, `main.rs` and
`xtask`. CI measures the same on Linux, where the code for Windows is not compiled and the
code for Unix is, and its first run measured 88.15 %. The floor only rises: whoever raises
the coverage may raise it, and nobody lowers it.

### Mutation testing reports, and does not gate

`cargo mutants` runs weekly in CI, and never on a laptop, for the reason `AGENTS.md`
gives. Its survivors go to the job's artefact and the job succeeds either way: the first
run has no baseline to compare with, and a gate that fails on survivors nobody has
looked at yet would be switched off rather than read.

**What would change it.** A first run looked at survivor by survivor: once each one is
killed by a test or recorded as a mutation that changes nothing, the job can fail on a
new one.

### Development builds keep line tables only

Full debug information made `target/debug` 3.6 GB on the machine ZENITH was written on,
whose disk filled once. Development builds keep file and line for backtraces and the
dependencies keep no debug information at all; nobody steps through them. `[profile.dev]`
in `Cargo.toml` says so beside the setting.

### The README's session was recorded with the user name replaced

The pictures in the README are drawn from a session recorded against a real SOLAR, and
`system.info` reports paths that hold the user name of whoever recorded it. The
recording has `lab` in its place, the one change made to it by hand, so that nobody's home
directory is published. `crates/zenith/tests/fixtures/README.md` says how to record it
again.
