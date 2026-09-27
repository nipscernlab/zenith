# Open questions

Everything here was decided without asking, because the brief said to take the most
conservative option and write down why. Each entry says what was chosen, what it rules
out, and what would make it worth revisiting.

A decision the architect confirms becomes a record in [`adr/`](adr/) and leaves this
file, so that this file is only ever the list of things still open to being overruled.

The first section is different in kind: it is what ZENITH needed from SOLAR and did not
find. Those are not ZENITH's to decide.

## What ZENITH needed from SOLAR and did not find

### The exit codes of `solar replay` are not in the contract

Section 14 of SOLAR's contract gives the exit codes of `solar call` and `solar serve`,
and none for `solar replay`. On 27 September 2026, with SOLAR 0.3.0 on this machine, it
exited 0 when every answer was the same, 5 when one differed or had no recorded answer,
and 2 when the file declared a major version of the format it does not read.

**Chosen.** The test that proves `solar replay` accepts what `/export` writes, and the
walkthrough's row for `/export`, take exit 0 as every answer being the same, which is
what was seen.

**What would close it.** The exit codes of `solar replay` in section 14 of the contract.

### `solar replay` reports a batch's answer as different when only its times differ

Section 12 of SOLAR's contract says replay ignores the members of `meta` that differ
between any two runs and every timestamp anywhere in a response. On 27 September 2026,
SOLAR 0.3.0, built at `341484d90c78` with uncommitted changes, replayed a recording it had
written itself, with one batch in it, and reported the batch's answer as different: the
only differences were `received_at`, `started_at` and `duration_us` inside the array. The
same members outside a batch were ignored.

**Chosen.** Nothing in ZENITH works around it. The test of the replay records no batch,
so that it tests ZENITH's recording and not this.

**What would close it.** Replay ignoring those members inside the answer to a batch too,
or the contract saying that it does not.

## Naming

### ZENITH's mark, drawn without a designer

The brief asked for a mark of ZENITH's own on the discipline of SOLAR's brand, meaning the
zenith, and never to be taken for SOLAR's. What it did not say was decided here, and
[`docs/brand/README.md`](brand/README.md) is the result:

- **The drawing**: the dome of an observatory with its slit open straight up, at the
  zenith, on SOLAR's module and in SOLAR's five colours. A first drawing, a disc with a
  round opening, looked like an eye at 16 px and was dropped.
- **The terminal form** is 16 × 4 cells, half the height of SOLAR's, and the four lines
  beside it are the name, level with the top of the dome where the slit opens, then what
  ZENITH is, the laboratory and the version, which is the order of SOLAR's example.
- **The opening** shows ZENITH's mark, and its last line, the state of the connection,
  moved below the mark, where the version of ZENITH was.
- **SOLAR's mark starts every connection in the Session tab and is on the card of
  `/version`**, the places where ZENITH shows SOLAR itself, each with the version of the
  SOLAR that answered where SOLAR's brand puts the version. `/call solar.version` keeps
  the generic layout. The mark starts every connection because the opening lasts as long
  as the connection, 32 ms on the architect's machine, and the architect saw no mark at
  all; the opening stays as short as the connection, as section 3 of `docs/DESIGN.md`
  requires.
- **The lockups** set the name in Martian Mono SemiBold, as SOLAR's do, with its cap
  height from the apex to the horizon and at the same place as SOLAR's name.

**Revisit when** a designer redraws it, or the architect wants a different drawing. No
trademark search has been made.

## The interface

### `Enter` never accepts a completion

`Tab` inserts the highlighted candidate; `Enter` runs the line exactly as typed, even
while the menu is open. Claude Code accepts a slash command with `Enter`, and ZENITH also
completes parameter names inside JSON, where `Enter` accepting a candidate would run
something that is not on screen.

### A line without a slash is not run

A line that starts with an API name gets `Commands start with a slash. Did you mean /call
solar.ping?` rather than being run as a call. Running it would be a guess about what the
person meant, and the guess would be wrong the first time an API is named like a word.

### The theme is not remembered between sessions

The command line history is, by the architect's decision, ADR 0014. The theme chosen with
`/theme` is lost when ZENITH exits, and `--theme` or `ZENITH_THEME` choose it at the start.
Remembering it would make a configuration file, which nothing else needs yet.

### Where the command line history is kept, and what it is called

ADR 0014 asks for the per-user data directory of each system and a documented format;
the rest was decided without asking:

- **The directory** is `nipscern-zenith` inside the system's own: a name that says whose it
  is, since a system monitor also installs a program called `zenith`.
- **The file** is `command-history.ndjson`, of the format `zenith-command-history`, version
  1, specified in `docs/DESIGN.md`, section 12.1.
- **`ZENITH_DATA_DIR`** puts it elsewhere, which is how the walkthrough and the
  measurements keep what they type out of the history of whoever runs them.
- **`--no-history`**, and **`ZENITH_NO_HISTORY`** for whoever always wants it, turn it off
  at the start, like the other flags that have a variable.
- **`/forget`** clears it. `/clear` already empties the transcript, and the word the History
  tab uses, history, was taken.
- **Two ZENITHs at once** each write their own history whole, so the file holds the lines of
  whichever wrote last. Merging them would need a lock or a format that appends, for a case
  that testing by hand rarely meets.
- **A file ZENITH cannot read**, of another format, a newer version or refused by the
  system, is left as it is, and the session keeps its history in memory only.

**Revisit when** the theme, or anything else, needs to be remembered: the same directory
would hold a configuration file.

### The time connected shows seconds only in the first minute

After the first minute it shows minutes, so that an idle ZENITH wakes once a minute rather
than once a second. The idle measurement in `STATUS.md` depends on this.

### `Ctrl+C` clears the command line before it quits

When no call is in flight and the line has text, the first `Ctrl+C` clears it and arms
the second, which quits. That is what shells and Claude Code do, and it makes `Ctrl+C`
never quit on the first press.

### The Log tab's keys set SOLAR's own level

The brief asks the Log tab to change SOLAR's level at run time, and a SOLAR with
`solar.set_log_level` can. There is one level, not two: the level chosen in the tab is the
level SOLAR logs at, so choosing `info` stops SOLAR writing what is below it, and choosing
`trace` afterwards shows only what comes from then on. A restart starts the next SOLAR at
the level the last one logged at, rather than at `--solar-log`.

**Rules out.** Keeping SOLAR at `trace` while the tab shows less, which was what ZENITH
did before SOLAR could change its level, and what it still does for a SOLAR that cannot.

**Revisit when** someone wants to read what SOLAR wrote below the level they chose: the
filter and SOLAR's level would then be two settings.

## Validation

### A keyword ZENITH does not check never fails a call

The validator is ZENITH's own, [ADR 0007](adr/0007-zenith-has-its-own-schema-validator.md).

When a schema uses a keyword the validator does not implement, such as `pattern`,
ZENITH says that it did not check it and that SOLAR will, and sends the call. A tool that
tests SOLAR must never refuse what SOLAR would accept.

## The connection

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

### `/export` writes the current connection, in SOLAR's recording format

SOLAR's recording format is one file per session, and each connection is a session of its
own, with ids that start from one again. So `/export` writes the calls of the current
connection only, and says how many of earlier connections it left out. A request that was
never answered is written without its answer, which is what happened; a call whose answer
was too long for ZENITH to keep is left out whole, and counted, because its answer cannot
be written as it crossed. The header names the writer `ZENITH <version>`, as
`docs/RECORDING.md` asks of a writer that is not SOLAR. ZENITH's own history format of
0.1.0, `zenith-history`, is gone: the report keeps ZENITH's own format, which holds more
than a recording can.

**Revisit when** one file per connection, for every connection of a session, is wanted.

### A request longer than SOLAR declares is not sent, unless it is sent by hand

When the manifest declares `limits.max_request_bytes`, a call ZENITH builds whose line is
longer is refused before it is sent, and the band says so. A line sent with `/raw` is sent
whatever its length, because SOLAR's answer to it, `MESSAGE_TOO_LARGE`, is what sending it
by hand is for. The other limits are shown on the card of `/list` and not enforced: ZENITH
lets at most 256 calls wait at once, which is also the number of requests SOLAR 0.3.0
declares it queues.

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

### A coverage run seeds the property tests

The floor itself is [ADR 0011](adr/0011-the-coverage-floor-is-88-percent.md). A coverage
run draws the random cases of the property tests from one fixed seed. With a new seed
each run, the figure moved with chance, because those cases were the only tests
that reached some of the interaction code: CI measured 88.15 % and then 87.87 % with the
same tests. The code they had reached by chance now has tests of its own.

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
