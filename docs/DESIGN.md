# The design of ZENITH

**Status:** the design of version 0.1.0, written before the code and kept true to it.
**Applies to:** ZENITH 0.1.0, talking to SOLAR through `solar/1` with a manifest whose
`schema_version` has major 2.

ZENITH is the terminal of the Constellation project: a full-screen application in which a
person talks to SOLAR, the API at the centre of Constellation, through tabs and commands.
It is a client of SOLAR and only a client. This document says what it shows, how it
behaves, and why, in enough detail that a change to any of it is a change to this file
first.

Where this document and the code disagree, the code is wrong, and the snapshots in
`crates/zenith/tests/snapshots/` are how that is found.

## 1. Principles

Five rules decide every question below that is not settled by the brief.

1. **SOLAR is the only source.** Everything ZENITH shows about an API comes from SOLAR at
   run time: the catalogue, the completion, the forms and the validation are built from
   `solar.manifest`. A new SOLAR API appears in ZENITH with no change to ZENITH, and a
   test walks every API in the manifest to prove it.
2. **ZENITH never works around SOLAR.** When ZENITH needs something SOLAR does not
   offer, that is a missing SOLAR API. It goes into `docs/OPEN_QUESTIONS.md`, and ZENITH
   does without.
3. **Every key works everywhere.** No action depends on Alt, because Option is not Alt
   in macOS Terminal by default, and none depends on a key that some terminal keeps for
   itself. Every action has a plain key or a `Ctrl` key.
4. **Colour is never the only carrier of meaning.** Every state that has a colour also
   has a word or a shape: `ok`, `error`, `connected`, a marker beside a selected row.
5. **Nothing grows without end.** A terminal stays open for hours. Every collection has
   a declared limit in entries and in bytes, and says so when it drops something.

## 2. The screen

At the minimum size, 80 columns by 24 rows, the screen is four bands.

```text
 ZENITH  1 Session  2 APIs  3 Log  4 History      ·    ∙        *   ·   ? keys
 › /ping
   ok  pong in 0.52 ms · 0.17 ms in SOLAR · solar.ping 1.0.0 · #3
 › /call solar.ping {"message":"hi"}
   ok  solar.ping 1.0.0 · 0.49 ms · 0.15 ms in SOLAR · #4
   echo         "hi"
   pong         true
   received_at  2026-09-27T15:47:00.906833Z





 ┌─────────────────────────────────────────────────────────────────────────────┐
 │ › /describe solar.p                                                         │
 └─────────────────────────────────────────────────────────────────────────────┘
 ──●── SOLAR 0.1.0 · solar/1 ─── last call 0.49 ms ─── in orbit 2m ── connected ─
```

| Band | Rows | What it holds |
| ---- | ---- | ------------- |
| Header | 1 | The name, the four tabs with the number that selects each, a scatter of stars in the space left over, and `? keys`. |
| Content | the rest | The current tab. |
| Command line | 3 | Only on the Session tab: a box with the prompt `›`, the text and the cursor. |
| Status bar | 1 | SOLAR as a body in orbit. |

**The header.** `ZENITH` in the accent colour and bold. The active tab is bold,
underlined and in the accent colour, so it stays distinct with no colour at all. The
stars in the header are placed by a fixed seed, so the header is the same on every run
and in every snapshot; they never move and never cost a redraw.

**The status bar** is drawn as an orbit: a line across the whole width with SOLAR on it.

| State | Body | Text |
| ----- | ---- | ---- |
| Connecting | `○` | `SOLAR connecting · 0.3 s` |
| Connected | `●` | `SOLAR 0.1.0 · solar/1`, then `last call 0.49 ms`, then `in orbit 2m`, then `connected` |
| Disconnected | `○` | `SOLAR disconnected · exited with code 1`, then `Ctrl+R reconnect` |

`last call` is the round trip ZENITH measured, from writing the request to reading the
response. What SOLAR measured of its own work, `meta.duration_us`, is shown beside each
response instead, because the two answer different questions: one is what a caller
waits, the other is what SOLAR costs.

`in orbit` is the time since the handshake finished. It shows seconds for the first
minute and minutes after that, so that an idle ZENITH wakes once a minute rather than
once a second (section 13).

When the terminal is smaller than 80 × 24, ZENITH draws one centred sentence instead of
the screen, `ZENITH needs 80 × 24, and this terminal is 72 × 20.`, and goes back to the
screen the moment the terminal is large enough. Nothing is lost while it is small.

## 3. The opening

While SOLAR starts and answers the handshake, ZENITH draws a starfield with the SOLAR
mark in the middle, exactly as `docs/brand/README.md` of SOLAR lays it out: the symbol of
`banner.txt`, 16 × 8 cells, and the text starting three columns after it, with the name on
the row whose slot opens to the right.

```text
   ▄▄██████▄▄
 ▄████████████▄
▄████              SOLAR
████████████████   The central API of the Constellation
████████████████   NIPS-CERN
           ████▀   connecting
 ▀████████████▀
   ▀▀██████▀▀

            ZENITH 0.1.0, the terminal of Constellation
```

- **It lasts exactly as long as the connection.** The moment the handshake finishes,
  the opening gives way to the tabs. On a fast machine that is a few milliseconds and the
  opening is barely seen, which is the requirement: it is never longer than the
  connection itself.
- **Any key skips it.** The connection carries on, and the status bar shows it.
- **The stars twinkle** at twelve frames a second, and only during the opening. No star is
  drawn within two cells of the mark, which is the clear space the brand rules ask for.
- **The mark is never stretched, mirrored or recoloured outside the palette.** It is gold
  on the night background, copper on the light one, and the terminal's own foreground
  when there is no colour. With `--ascii` it is `banner-ascii.txt`.
- **ZENITH has no mark of its own yet.** Its name is written in text, and a mark is an
  open question.

The two drawings are copied byte for byte from SOLAR's `docs/brand` into
`crates/zenith/assets/brand/`, and a test compares them with SOLAR's when SOLAR's checkout
is available, which it is in CI.

If the connection fails, the opening stops animating and shows what happened in its
place (section 9). The stars stay, still.

## 4. The Session tab

The Session tab is a transcript above a command line. Every command typed, and every
response to it, goes into the transcript, laid out for a person.

**A command** is echoed as `› /ping`, in the accent colour.

**A response** is a card that starts with a badge and a summary line:

```text
   ok  solar.ping 1.0.0 · 0.49 ms · 0.15 ms in SOLAR · #4
   echo         "hi"
   pong         true
   received_at  2026-09-27T15:47:00.906833Z
```

```text
   error  INVALID_ARGUMENT · UNKNOWN_FIELD · solar.ping · 0.31 ms · #5
   Invalid params for solar.ping: unknown field `mesage`, expected `message`.
   /mesage   expected  one of: message
             received  "hi"
             hint      There is no mesage parameter. Did you mean message? A call
                       that works: {"message":"hi"}.
             docs      docs/ERRORS.md#invalid_argument
```

- The badge is the word `ok` or `error`, drawn in reverse so it reads with no colour.
- An error lists every entry of `details` with its five members, in the order the
  contract gives them, which is the order its reader needs: where, what was expected,
  what arrived, what to do.
- `warnings` follow the data, one per line, with their code.
- `data` is laid out as aligned keys and values. Nested objects are indented under their
  key; arrays of scalars stay on one line when they fit; long strings wrap. `null` is
  written `null`, dimmed.
- **The contract is checked on every response.** When an envelope does not have the
  shape `docs/CONTRACT.md` gives it, for example a `result` without `warnings` or an error
  whose `details` is empty, the card ends with a `contract` line saying what is missing
  and which section requires it. ZENITH is how SOLAR is tested by hand, so a response
  that breaks the contract must not look like one that keeps it.
- **Five commands have layouts of their own**, because they are ZENITH's commands and not
  just calls: `/list` is a table of the APIs, `/describe` is the entry of one API laid out
  like the APIs tab, `/ping` is one line, `/version` is the three versions and the build,
  and `/help` is the table of commands. If the data does not have the shape the layout
  expects, the card falls back to the generic layout rather than failing.
- **A call in flight** is a line `… solar.ping, waiting 0.3 s` whose timer moves while
  the call is out. When SOLAR's own budget for that API, `timeout_ms`, has passed with no
  answer, the line says so, because SOLAR promises an answer by then.

**The full envelope is one key away.** `Ctrl+O` opens the envelope viewer on the latest
response: the request line and the response line exactly as they crossed the pipe, laid
out as indented JSON. In the viewer, `←` and `→` move to the previous and the next call.

**The transcript** follows the bottom until the person scrolls up with `PgUp` or
`Ctrl+B`; a marker `▼ 3 new` then counts what arrived below. `PgDn` or `Ctrl+F` at the
bottom follows again. `/clear` empties the transcript and leaves the History alone.

## 5. The command line

A single line with a prompt, editing keys a shell user already has, history and
completion.

**Completion** comes from what is being typed, and the candidates from the manifest:

| Where the cursor is | Candidates |
| ------------------- | ---------- |
| In the first word, after `/` | The commands, with their one line description |
| In the argument of `/describe` or the first argument of `/call` | The API names, with their summaries |
| At a key position inside the JSON of `/call` | The parameters the schema declares at that position and that are not there yet, with their types and descriptions |
| At a value position inside the JSON of `/call` | The values the schema allows there: `true` and `false`, the members of an `enum`, the `const` of each `oneOf` branch, `null` when the type admits it |
| In the argument of `/theme` or `/help` | The themes, the commands |

The candidates appear in a menu above the line as soon as there are any. `Tab` inserts the
highlighted one and `Shift+Tab` moves back; `↑` and `↓` move through the menu while it is
open; `Esc` closes it. `Enter` always runs the line as it is typed: it never accepts a
candidate, so what runs is what is on screen.

Inside JSON, the position is found by a tolerant scanner that reads the text up to the
cursor, keeps track of the objects and arrays it is in, and follows the same path through
the schema, resolving `$ref` into the manifest's `$defs`. Nested objects complete like the
top level.

**Validation happens before anything is sent.** When `Enter` is pressed on a `/call`:

1. The JSON is parsed. A syntax error is shown with a caret under the column where the
   parser stopped.
2. It must be an object, because the contract requires `params` to be one.
3. It is validated against the API's `params_schema`. Each failure names the JSON pointer,
   what the schema expected and what was there, and the part of the line it refers to is
   underlined: the value for a wrong type, the key for a parameter the schema does not
   declare, with the closest declared name as a suggestion, and the braces of the object
   for a missing required parameter.

The errors are drawn in a band between the transcript and the command line, and the line
keeps its text so it can be fixed in place. Nothing is sent and nothing enters the
transcript until the line is valid.

The validator is ZENITH's own and implements the assertions of JSON Schema 2020-12. When a
schema uses a keyword the validator does not check, the band says which one, and that
SOLAR checks it. A tool that tests SOLAR must never refuse something SOLAR would accept,
so an unknown keyword is never treated as a failure. Why the validator is not the
`jsonschema` crate, with the measurement, is in `docs/OPEN_QUESTIONS.md`.

**The history** of the command line holds the lines that were run, without consecutive
repeats, and `↑` and `↓` walk it when the completion menu is closed. It is kept in
memory only: nothing is written to disk in this version.

A line that does not start with `/` is not run. When its first word is an API name, the
band says `Commands start with a slash. Did you mean /call solar.ping?`; otherwise it
points at `/help`.

## 6. The APIs tab

The catalogue of every API in the manifest, a list on the left and the entry of the
selected API on the right.

```text
 ZENITH  1 Session  2 APIs  3 Log  4 History                            ? keys
 5 APIs · manifest 2.0.0     │ solar.ping 1.0.0 · experimental · since 0.1.0
 › solar.describe    1.0.0   │ Answers immediately, to prove SOLAR is there
   solar.manifest    1.1.0   │ side effects none · idempotent · budget 1000 ms
   solar.ping        1.0.0   │
   solar.version     1.0.0   │ parameters
   system.info       1.1.0   │   message  string or null  optional
                             │            Anything the caller wants back, to tell
                             │            one ping from another.
                             │ errors     none beyond dispatch
                             │ examples
                             │   1 bare             {}
                             │   2 with_a_message   {"message":"hi"}
                             │ description
                             │   The cheapest call SOLAR has. It touches nothing, ...
```

- **Every section comes from the manifest entry:** summary, version, stability, `since`,
  side effects, idempotence, `timeout_ms`, the parameters from `params_schema`, the
  declared errors as status and reason, the examples, the description, and the members of
  `output_schema`. A member of the entry that ZENITH does not know is listed at the end
  under its own name, so a field SOLAR adds later is visible before ZENITH learns it.
- **Each example runs with one key:** `1` to `9` run the example with that number. The
  result is shown under the example, compared with the response the example declares,
  by the rule it declares: `exact` or `subset`, with `"$any"` matching anything, as
  section 8.2 of the contract defines. `matches` or `differs at /echo: expected "hi",
  received null`. The call also goes to the Session transcript and to the History, like
  any other.
- **`a` runs every example of the selected API**, one call each.
- **`Enter` opens the parameter form** built from `params_schema`: one field per
  property, with its type, whether it is required, its default and its description. A
  string field takes text as typed; a number or integer field takes a number; a boolean
  field toggles with `Space`; a field whose schema offers fixed values cycles through them
  with `←` and `→`; anything else takes JSON. An empty optional field is left out of the
  call. `Enter` validates the whole object against the schema and runs it; a failure is
  drawn at the field it belongs to. The line under the form shows the command it is
  equivalent to, `/call solar.ping {"message":"hi"}`, which is how a person learns the
  command line from the form.
- **`e`** puts `/call <api> <first example>` on the command line and goes to the Session
  tab, to edit it there.
- **`f`** filters the list by a part of the name; `Esc` clears the filter.
- **`R`** asks SOLAR for the manifest again and rebuilds the catalogue.

## 7. The Log tab

SOLAR's standard error, at a level chosen here.

```text
 level  error  warn  [info]  debug  trace          1 204 lines · 0 dropped
 15:47:00.906  TRACE  --> {"jsonrpc":"2.0","id":3,"method":"solar.ping"}
 15:47:00.907  TRACE  <-- {"jsonrpc":"2.0","id":3,"result":{"data":{"echo":nu…
 15:47:02.114  INFO   the session ended after 3 calls
```

SOLAR reads its level once, from `SOLAR_LOG`, when it starts, and it has no API to change
it afterwards. ZENITH therefore starts SOLAR at `trace` with `SOLAR_LOG_FORMAT=json` and
chooses the level itself, by filtering what arrives. Choosing a level is instant and
never restarts SOLAR. `--solar-log <level>` starts SOLAR at a lower level for someone who
wants SOLAR to do less work; the tab then says what SOLAR was started with, so a level
that cannot show anything is not mistaken for silence. The missing API is recorded in
`docs/OPEN_QUESTIONS.md`.

- `e`, `w`, `i`, `d` and `t` choose error, warn, info, debug and trace; `←` and `→`
  lower and raise it. The level of every line is written in words, so the level is never
  shown by colour alone.
- A line that is not JSON, such as the text a panic prints, is shown as it arrived, under
  the level `raw`, whatever the chosen level.
- `Enter` expands the selected line to its whole message, wrapped.
- `End` or `G` follows the newest line; `c` clears the tab.

## 8. The History tab

Every request and response of the session, including the two of the handshake, with
their timing.

```text
 12 calls · 0 dropped · kept 10 000 calls or 32 MiB
   #  sent          method           outcome                    round trip  in SOLAR
   1  15:46:58.120  solar.version    ok                             1.21 ms    0.09 ms
   2  15:46:58.120  solar.manifest   ok                             1.84 ms    0.66 ms
 › 5  15:47:03.502  solar.ping       error INVALID_ARGUMENT          0.31 ms    0.07 ms
```

- `Enter` opens the envelope viewer on the selected call.
- `r` sends the selected request again, as a new call.
- `e` puts it on the command line as `/call`.
- `x` exports the tab to a file, the same as `/export`.

**The export** writes one JSON object per line, in the format of section 12, straight to
the file as it walks the buffer: nothing is assembled in memory first.

It is **not** a recording that `solar replay` accepts. SOLAR's README says a recording is
NDJSON with every line in, every line out and the time each crossed, and documents nothing
more: no member names, no version. Writing that format from what can be observed would be
inventing a fact about SOLAR, so ZENITH writes a format of its own, documented here, and
the missing documentation is an open question. When SOLAR documents its recording format,
the exporter is one function.

## 9. The connection to SOLAR

### 9.1 Finding the binary

In this order, the first that is given wins:

1. `--solar <path>` on the command line;
2. the environment variable `ZENITH_SOLAR`;
3. `solar` on the `PATH`, found the way the system would find it.

**On Windows, `solar.exe` is never run from where it was found.** ZENITH reads the file,
computes its SHA-256, copies it to `%TEMP%\zenith\solar\<the first 16 hex digits>\solar.exe`
unless a copy with that name is already there, and starts the copy. The reason is
practical: SOLAR's build replaces `target\release\solar.exe`, and Windows cannot replace
an executable while it is running, so running it in place would break SOLAR's build every
time ZENITH was open. The copy is written under a temporary name and renamed into place,
so two ZENITH processes starting at once never see half a file, and read back and hashed
again, so that a copy made while SOLAR's build was rewriting the file is thrown away
rather than kept under a name that lies about it. Copies of other hashes of the same
program are removed on the way, and a copy that is running cannot be removed, which is
what keeps a second ZENITH safe. Each program has its own directory, so the copies of
ZENITH's test double never touch SOLAR's. `system.info` reports the copy as its
`executable`, which is true.

On macOS and Linux a running binary can be replaced, so SOLAR is started where it is.

### 9.2 The handshake

ZENITH starts `solar serve --stdio`, with `SOLAR_LOG` and `SOLAR_LOG_FORMAT=json` in its
environment, and writes two requests at once, without waiting between them:

1. `solar.version`, with id `1`;
2. `solar.manifest`, with id `2`.

Then it checks, in this order, and stops at the first that fails:

| Check | ZENITH knows | When it does not match |
| ----- | ------------ | ---------------------- |
| `protocol` of `solar.version` | `solar/1` | `SOLAR 0.3.0 speaks solar/2, and ZENITH 0.1.0 speaks solar/1. Use a ZENITH that speaks solar/2, or a SOLAR that speaks solar/1.` |
| `manifest_schema_version` of `solar.version`, and `schema_version` of the manifest | major `2` | `SOLAR 0.3.0 writes its manifest in layout 3.0.0, and ZENITH 0.1.0 reads layout 2. ...` |
| The manifest's `apis` and `$defs` | present, with the shape section 8 of the contract gives | What was missing, and where. |

A manifest whose major differs is refused, as section 10 of the contract requires of every
consumer. A minor that is newer is accepted: members ZENITH does not know are shown under
their names (section 6), never dropped.

The time connected starts when both checks pass.

### 9.3 What ZENITH detects in the manifest

| Capability | How it is detected | When present | When absent |
| ---------- | ------------------ | ------------ | ----------- |
| Cancellation | An API named `solar.cancel` whose `params_schema` declares `id` | `Ctrl+C` sends `solar.cancel` with the id of the latest call in flight, and the transcript shows the `outcome` SOLAR reports | `Ctrl+C` says that this SOLAR offers no `solar.cancel`, and how long the call may still take by its `timeout_ms` |
| Batches | Not detectable: the manifest does not say whether batches are accepted, or how many elements | | ZENITH sends one request per line, and `/raw` sends a batch by hand |

Batches are in the draft of SOLAR's contract, section 3.2, which also fixes their limit
at 64 elements, but the manifest does not declare either. Learning it by sending a probe
batch would be working around a missing declaration, which principle 2 rules out, so it
is recorded in `docs/OPEN_QUESTIONS.md` and ZENITH uses no batches of its own. A batch
sent with `/raw` is shown as one card per element, matched by position.

### 9.4 Requests and responses

- Requests carry integer ids, from `1` in each connection, never reused, which is what
  section 9.5 of SOLAR's draft contract asks of a caller that cancels.
- Responses are matched by `id`, never by order: once SOLAR answers `solar.cancel` ahead
  of the call it cancels, order is no longer a promise.
- A response with `id: null` belongs to the oldest request still waiting. It only happens
  when a request was too broken to have an id, which ZENITH never sends except through
  `/raw`.
- A response that matches no request is shown in the transcript as `unexpected`, with
  its whole envelope, because it is a bug in one of the two programs. A line that is not
  JSON at all answers nothing, for the same reason: guessing which call it belongs to
  would hide the bug.
- A line sent with `/raw` whose `id` is the id of a call still waiting is refused before
  it is sent, because neither program could tell the two answers apart. ZENITH never gives
  its own calls an id that a line sent by hand is waiting on.
- One response line may be up to 16 MiB, the same limit SOLAR puts on a request line. A
  longer one is discarded up to its newline, and the call it answered is marked
  `response too large`, with the size that was reached.
- At most 256 calls may be waiting at once; a 257th is refused in the command line with
  that number.

### 9.5 When SOLAR fails

ZENITH says what happened, in one sentence, and what to do, in another.

| What happened | What ZENITH says | What it offers |
| ------------- | ---------------- | -------------- |
| `solar` is not on the `PATH` | `ZENITH could not find solar on the PATH, which has 23 directories.` and how to put SOLAR's `target/release` on it, or `--solar`, or `ZENITH_SOLAR` | Reconnect, quit |
| The given path does not exist | `The path given by --solar does not exist: <path>.` | Reconnect, quit |
| It exists and cannot be started | The system's reason, for example `permission denied`, and on Unix `chmod +x <path>` | Reconnect, quit |
| The Windows copy cannot be made | The path of the copy and the system's reason | Reconnect, quit |
| It started and exited before answering | The exit code or signal, and the last lines of its standard error | Reconnect, quit, and the Log tab |
| It answered something that is not the contract | What was wrong, quoted | Reconnect, quit |
| It did not answer the handshake within 15 seconds | That, and that the Log tab has what it wrote | Reconnect, quit |
| It speaks a protocol or a manifest layout ZENITH does not know | Both versions, as in 9.2 | Quit |
| It died after connecting | The exit code or signal, how long it had been connected, and which calls were still waiting, each of which is closed as `the connection ended before SOLAR answered` | Reconnect; the tabs stay usable |

At startup the sentence replaces the connecting line of the opening, and `r` reconnects,
`Enter` goes to the tabs anyway, so the Log can be read, and `q` quits. After connecting,
the failure is a card in the transcript and the status bar turns to disconnected; the
History and the Log keep everything.

**`Ctrl+R` or `/reconnect` restarts SOLAR at any time**, finding the binary again and
copying it again if it changed. That is how a new build of SOLAR is picked up without
leaving ZENITH.

### 9.6 Ending

On quit, ZENITH closes SOLAR's standard input, which ends the session the way the contract
says a session ends, and waits up to half a second for SOLAR to exit before killing it.

## 10. Commands

| Command | What it does | The call |
| ------- | ------------ | -------- |
| `/list` | The table of every API | `solar.manifest`, which also refreshes the catalogue |
| `/describe <api>` | One API, laid out | `solar.describe` with `{"api": ...}` |
| `/call <api> [json]` | Any call; absent JSON means `{}` | `<api>` with the JSON, validated first |
| `/ping [message]` | One line: the round trip and SOLAR's own time | `solar.ping` |
| `/version` | The three versions and the build | `solar.version` |
| `/theme [name]` | `night`, `light` or `high-contrast`; with no name, the next one | none |
| `/help [command]` | The commands, or one of them | none |
| `/quit` | Leaves ZENITH | none |
| `/raw <line>` | Sends the line exactly as typed, unchecked: a broken envelope, a batch, a notification | the line |
| `/reconnect` | Restarts SOLAR, as `Ctrl+R` | the handshake |
| `/clear` | Empties the transcript | none |
| `/export [path]` | Writes the History to a file (section 12) | none |
| `/report [path]` | Writes a bug report to a file (section 12) | `system.info` |

The first eight are the ones the brief names. The other five exist because testing SOLAR
by hand needs them: `/raw` is the only way to see how SOLAR answers something malformed,
`/reconnect` picks up a new build, and `/report` is what a tester sends back.

`/raw` is the one command whose line is not validated, because its purpose is to send what
validation would refuse. Its response is matched like any other.

## 11. Keys

Every action has a plain key or a `Ctrl` key. `Alt` and the function keys are extra
routes for terminals that deliver them, never the only one. The help overlay is drawn
from the same table the keys are dispatched from, so a key cannot exist without being
listed, and a test holds this document to that table.

**Everywhere**, unless a text field has the keys:

| Key | Action |
| --- | ------ |
| `?` | Help: every key, by where it works |
| `Tab`, `Shift+Tab` | Next tab, previous tab |
| `F1` to `F4`, `Alt+1` to `Alt+4` | Go to that tab, where the terminal delivers them |
| `/` | Go to the Session tab with `/` on the command line |
| `Ctrl+O` | The envelope viewer, on the latest call |
| `Ctrl+R` | Restart SOLAR |
| `Ctrl+L` | Draw the whole screen again |
| `Ctrl+C` | Cancel the latest call in flight, when SOLAR offers `solar.cancel`; pressed again, quit |
| `Esc` | Close the overlay, the menu or the form that is open |

`Ctrl+C` in detail: the first press cancels the latest call in flight when there is one
and SOLAR offers `solar.cancel`; says why it cannot when SOLAR does not; clears the command
line when it has text; and in every case arms the second press, which quits. Any other
key disarms it.

**The command line**, on the Session tab:

| Key | Action |
| --- | ------ |
| `Enter` | Run the line |
| `Tab`, `Shift+Tab` | Complete, and move through the candidates; on an empty line, next and previous tab |
| `↑`, `↓` | The completion menu when it is open, the history otherwise |
| `←`, `→`, `Home`, `End`, `Ctrl+A`, `Ctrl+E` | Move |
| `Ctrl+←`, `Ctrl+→`, `Alt+B`, `Alt+F` | Move by a word |
| `Backspace`, `Delete`, `Ctrl+W`, `Ctrl+U`, `Ctrl+K` | Delete a character, a word, to the start, to the end |
| `PgUp`, `PgDn`, `Ctrl+B`, `Ctrl+F` | Scroll the transcript |
| `?` | Help, when the line is empty |

**Lists**, in the APIs, Log and History tabs, the viewer and the help:

| Key | Action |
| --- | ------ |
| `↑`, `↓`, `k`, `j` | Move by one |
| `PgUp`, `PgDn`, `Ctrl+B`, `Ctrl+F` | Move by a page |
| `Home`, `End`, `g`, `G` | Go to the top, to the bottom |

**APIs**: `1` to `9` run that example, `a` runs them all, `Enter` opens the form, `e`
edits on the command line, `f` filters, `R` reloads the manifest. **In the form**: `Tab`,
`Shift+Tab`, `↑` and `↓` move between fields, `Space` toggles a boolean, `←` and `→` cycle
fixed values, `Enter` runs, `Esc` closes.

**Log**: `e`, `w`, `i`, `d`, `t` choose the level, `←` and `→` lower and raise it, `Enter`
expands a line, `c` clears.

**History**: `Enter` views, `r` runs again, `e` edits, `x` exports.

**The envelope viewer**: `←` and `→` for the previous and next call, `Esc`, `q` or
`Ctrl+O` to close.

`Home`, `End`, `PgUp` and `PgDn` are kept by macOS Terminal for its own scrollback unless
Shift is held, which is why every one of them has a letter or a `Ctrl` key beside it.

## 12. The files ZENITH writes

ZENITH writes a file only when asked, with `/export`, `/report` or `x`, into the current
directory unless a path is given, under a name with the time in UTC:
`zenith-history-2026-09-27T15-47-00Z.ndjson`, `zenith-report-2026-09-27T15-47-00Z.ndjson`.
It never overwrites a file that exists.

Both files are NDJSON: one JSON object per line, each with a `kind`. The first line is
always the header.

| `kind` | Members |
| ------ | ------- |
| `header` | `format` (`zenith-history` or `zenith-report`), `format_version` (`1.0.0`), `zenith_version`, `written_at` |
| `call` | `id`, `method`, `origin` (`handshake`, `command`, `example`, `form`, `raw`, `cancel`, `report`), `sent_at`, `received_at`, `round_trip_us`, `request` (the line as sent, as JSON when it was JSON, as a string otherwise), `response` (the same, or `null` when none arrived), `closed` (why a call ended without a response, or `null`) |
| `dropped` | `calls`, `bytes`: what the History had already dropped when the file was written, so a reader knows the file is not the whole session |

A report adds, in this order, after the header:

| `kind` | Members |
| ------ | ------- |
| `zenith` | The version, the target, the system, the terminal as ZENITH sees it: size, colour depth, whether Unicode is drawn, the theme, and the values of `TERM`, `COLORTERM`, `TERM_PROGRAM`, `WT_SESSION` and `NO_COLOR` |
| `solar` | The binary that was found, the copy that runs on Windows, its SHA-256, the `solar.version` data of the handshake, and whether it is connected |
| `system` | The `data` of a `system.info` call made for the report, or why there is none |
| `log` | One per line of the Log tab, the most recent 1 000, with `time`, `level`, `request_id`, `method`, `duration_us` and `message` |

and then the `call` and `dropped` lines of the History. A report holds no environment
variable beyond the five named, and no file content. `docs/TESTING_BY_HAND.md` tells a
tester to send it.

## 13. Memory

Every collection that grows with use is a ring buffer with two declared limits. When
either would be passed, the oldest entries go, and the tab that shows the collection says
how many went.

| Collection | Entries | Bytes | Where the drops are shown |
| ---------- | ------- | ----- | ------------------------- |
| History | 10 000 calls | 32 MiB | The History header |
| Log | 20 000 lines | 16 MiB | The Log header |
| Session transcript | 1 000 entries | 16 MiB | A line at the top of the transcript |
| Command line history | 500 lines | 256 KiB | Not shown: it is a convenience, not a record |
| Calls in flight | 256 | | The command line refuses the next one |

The bytes of an entry are the bytes of the text it holds, the lines as they crossed the
pipe, plus a fixed amount for its bookkeeping. A call shared by the History and the
transcript is stored once and counted by both, so the sum of the limits bounds the
memory. Responses are kept as the text that arrived and parsed again when they are shown,
which costs well under a millisecond for the largest response SOLAR has today, the
manifest, and saves holding every response twice.

The threads that read SOLAR's output hand it to the interface through a channel of 1 024
messages. When the interface falls behind, the reading threads wait, then SOLAR waits on
its pipe, and nothing piles up in between.

The claim is measured, not asserted: section 15.

## 14. Themes and fallbacks

### 14.1 The palette

Every colour ZENITH draws is one of the five colours of SOLAR's `docs/brand/README.md`, or
a mix of two of them, written as the mix. Nothing outside the palette is introduced.

| Name | Hex | From the brand |
| ---- | --- | -------------- |
| Gold | `#FFC23D` | the symbol on dark; here, what matters |
| Copper | `#B35A00` | the symbol on light |
| Night | `#0B0C14` | dark background |
| Mist | `#F5F1E8` | light background, the name on dark |
| Ink | `#17161C` | the name on light, text |

### 14.2 The roles

A theme gives each role a colour. The code draws roles, never colours.

| Role | Night | Light | High contrast |
| ---- | ----- | ----- | ------------- |
| background | night | mist | night |
| text | mist | ink | mist |
| muted | mist 62% over night | ink 68% over mist | mist |
| faint, for borders and stars | mist 30% over night | ink 30% over mist | mist |
| accent | gold | copper 75% over ink | gold |
| error | copper 50% over gold | copper 75% over ink | gold |
| selection | mist 12% over night | ink 10% over mist | reverse video |

The contrast of every text role against its background is computed by a test with the
formula of WCAG 2.2, and must be at least 4.5:1 in the night and light themes and at
least 7:1 in the high-contrast one. Gold is never drawn on a light background, which the
brand forbids at 1.43:1.

In the high-contrast theme nothing is muted: all text is mist or gold on night, a
selection is reverse video, and emphasis is bold or underlined.

Errors and the accent are close in hue in the dark themes, because the palette is warm;
they are told apart by the words `error` and `ok`, by the reverse badge, and by bold, as
principle 4 requires anyway.

### 14.3 Colour depth

| Depth | How it is chosen | What is drawn |
| ----- | ---------------- | ------------- |
| True colour | `COLORTERM` is `truecolor` or `24bit`, or `WT_SESSION` is set (Windows Terminal), or `TERM_PROGRAM` is `iTerm.app` | The hex values |
| 256 colours | `TERM` contains `256color`, or the classic Windows console | The nearest entry of the xterm cube, except gold, which is `215` as the brand says |
| 16 colours | anything else | A table per theme; gold is yellow, `ESC[33m`, as the brand says |
| None | `NO_COLOR` is set and not empty (no-color.org) | No colour at all: the terminal's own foreground and background, and emphasis by bold, underline and reverse only |

`--color <depth>` or `ZENITH_COLOR` overrides the guess: `truecolor`, `256`, `16` or
`none`. `NO_COLOR` wins over everything except an explicit `--color`, which is the rule
no-color.org gives to a user who asks for colour on purpose.

### 14.4 Characters

By default ZENITH draws only characters that the fonts of the classic Windows console
have: the box drawing, the block elements and a few shapes from code page 437, such as
`─ │ ┌ ┐ └ ┘ █ ▀ ▄ · ∙ ● ○ › …`. Rounded corners, check marks and emoji are left out on
purpose, because a missing glyph is a box of garbage in somebody's terminal.

`--ascii` or `ZENITH_ASCII` draws in 7-bit ASCII only: `+ - |` for the boxes, `*` and `.`
for the stars, `banner-ascii.txt` for the mark. As in SOLAR, it is a switch and not a
guess, because no terminal can be asked reliably whether it has a glyph.

### 14.5 Choosing a theme

`/theme` in the session, `--theme <name>` or `ZENITH_THEME` at startup. The choice is not
written to disk in this version.

## 15. Performance

The loop is driven by events and never polls. The main thread waits on one channel,
which three threads feed, the keyboard, SOLAR's standard output and SOLAR's standard
error, and wakes early only for a deadline that something on screen needs:

| Deadline | Only while |
| -------- | ---------- |
| The next frame of the opening, at twelve a second | the opening is on screen |
| The timer of a call in flight, at four a second | a call is in flight |
| The next change of `in orbit` | always: once a second in the first minute, once a minute after |
| The next check that SOLAR has exited | SOLAR has closed its output and not yet exited |

An idle ZENITH therefore wakes once a minute. Redraws only happen after something
changed, and ratatui writes to the terminal only the cells that differ.

What is measured, how, and where the numbers are:

| What | How | Where |
| ---- | --- | ----- |
| Startup | From starting the process to the first frame drawn connected, and each step between, recorded by the process when `ZENITH_TRACE_TIMINGS` names a file | `cargo xtask perf`, `STATUS.md` |
| A key to its redraw | For each key, from the moment it is read to the moment the frame is flushed, over a scripted session of a thousand keys | the same |
| Idle | The processor time of the process over sixty seconds with nothing happening, and the number of times the loop woke | the same |
| Memory | The resident memory at idle, and after a scripted session of one hundred thousand calls, sampled every ten thousand | `cargo xtask soak`, `STATUS.md` |
| Drawing | Each screen at 80 × 24 and at 200 × 60, in-process | `cargo bench` |

## 16. Tests

| Kind | What it holds | Where |
| ---- | ------------- | ----- |
| Unit | The scanner, completion, validation, the ring buffers, the parsing of envelopes and of the manifest, the contrast of every theme | beside the code |
| Properties | Any text at all given to the scanner, the completer or the validator returns without panicking; the validator agrees with the `jsonschema` crate on every schema of the manifest for generated values | `tests/properties.rs` |
| Screens | Every screen and state, rendered with ratatui's `TestBackend`, at 80 × 24 and at 160 × 48, in each theme, snapshotted with `insta`, with every colour of every cell | `crates/zenith/tests/screens.rs` |
| The real SOLAR | The handshake, every command, every failure path that can be produced, cancellation and batches when the SOLAR under test has them, and a walk through every API of the manifest that runs every example through the path a person uses and checks each result against the example | `crates/zenith/tests/against_solar.rs` |
| The whole program | `zenith` started in a pseudo-terminal, keys typed into it, the screen read back | `cargo xtask perf`, `cargo xtask soak` |

The tests that need SOLAR find it the way ZENITH does, `ZENITH_SOLAR` first, and say
plainly that they were skipped when it is not there. CI always has it, because CI builds
it, and in CI a skip is a failure.

## 17. What this version does not do

No AI features, no program other than `solar` started, no file written unless asked, no
configuration file, no persistent history, no mouse. Each of these is either out of scope
by instruction or an open question, and `docs/OPEN_QUESTIONS.md` says which.
