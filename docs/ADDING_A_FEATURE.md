# Adding a feature

The one path for adding a command, a key, a view or a tab to ZENITH, whoever is writing
the code. Each kind of change touches the same few places every time, and the tests are
arranged so that forgetting one of them fails something.

Before any of it, two questions:

1. **Is it SOLAR's to do?** ZENITH is a client of SOLAR and only a client (ADR 0001). If
   the feature needs something SOLAR does not offer, that is a missing SOLAR API: write it
   in the first section of `docs/OPEN_QUESTIONS.md` and stop. Never read SOLAR's source,
   and never work around what SOLAR lacks.
2. **What does the design say?** A change in behaviour is a change to `docs/DESIGN.md`
   first: the section that describes the screen, the key or the command. Write that, then
   the code that does what it says.

## The places, and what each one holds

| Path | What it holds |
| ---- | ------------- |
| `crates/zenith/src/commands.rs` | The command table, `COMMANDS`, and the parser |
| `crates/zenith/src/keys.rs` | The key table, `BINDINGS`: every key, where it works, what it does |
| `crates/zenith/src/app/mod.rs` | The state machine: every event in, every effect out |
| `crates/zenith/src/app/*.rs` | The state of each tab, and the History, the Log and the connection |
| `crates/zenith/src/ui/*.rs` | The drawing of each tab, the cards of the transcript, the overlays |
| `crates/zenith/src/limits.rs` | Every limit on what ZENITH keeps |
| `crates/zenith/src/app/tests.rs` | The state machine driven by scripted events |
| `crates/zenith/tests/screens.rs` | Every screen, snapshotted |
| `crates/zenith/tests/against_solar.rs` | The real loop, driven by keys, against a real SOLAR |
| `crates/zenith-client/` | Everything that talks to SOLAR, with no terminal code |
| `xtask/src/walkthrough.rs` | The table of `docs/TESTING_BY_HAND.md`, done by a machine |

## A command

1. **`commands.rs`**: a `Spec` in `COMMANDS`, with its name, usage, one-line summary and
   what it takes; a variant of `Command`; its arm in `parse`. `/help` and the completion of
   command names read the table, so they need nothing more.
2. **`app/mod.rs`**: its arm in `App::run`. A command that calls SOLAR goes through
   `command_call` with a `Layout`, which puts the call in the History and the transcript;
   a local one changes the state and pushes an `Entry`.
3. **If its answer has a layout of its own**: a variant of `Layout` in `app/session.rs`,
   and its drawing in `ui/cards.rs`, in `single`. When the data does not have the shape
   the layout expects, fall back to `json::human`, never to nothing.
4. **If its argument can be completed**: an arm in `completion.rs`, in `arguments`.
5. **Tests**: its parsing in `commands.rs`; its behaviour in `app/tests.rs`, through the
   harness; a scene in `tests/screens.rs` if it draws something new; and, if it calls
   SOLAR, a line in `the_commands_of_the_brief_all_work_against_the_installed_solar` or a
   test of its own in `tests/against_solar.rs`.
6. **Documents**: the table of section 10 of `docs/DESIGN.md`, the commands of the README,
   `CHANGELOG.md`. When a person testing SOLAR should try it, a row in the table of
   `docs/TESTING_BY_HAND.md`, section 4, and its step in `xtask/src/walkthrough.rs`, which
   a test holds together.

## A key

1. **`keys.rs`**: a variant of `Action` if the action is new, and a row in `BINDINGS` with
   the place it works, its keys, and the words the help overlay shows. The help overlay is
   drawn from the table, so it needs nothing more.
2. **The rule of ADR 0005**: every action has a plain key or a `Ctrl` key. `Alt` and the
   function keys are only ever extra routes, because macOS Terminal does not send Alt and
   several terminals keep function keys for themselves. A test fails otherwise. Never bind
   `Ctrl` and `Alt` together: that is `AltGr`, which is how many keyboards type `/`, `?`,
   `{` and `}`, and ZENITH treats it as text.
3. **`app/mod.rs`**: the action in the `act_on_*` function of the place.
4. **`docs/DESIGN.md`**, section 11, must name the key; a test reads the document and fails
   when it does not.

## A view

A card in the transcript, a section of an API's entry, an overlay.

- **A card**: a variant of `Entry` in `app/session.rs`, with its size in `Measured`, and its
  lines in `ui/cards.rs`, in `entry_lines`. Text from SOLAR goes through `text::span` or
  `text::clean`, which turn control characters into visible escapes, so that nothing SOLAR
  sends can drive the terminal; a test sends escape sequences and checks every cell.
- **An overlay**: a variant of `Overlay` in `app/mod.rs`, the places its keys work in
  `places`, its keys in `act_on_overlay`, and its drawing in `ui/overlays.rs`, dispatched
  from `ui/mod.rs`.
- **A view that scrolls**: what the wheel does over it, a variant of `Scrolls` returned by
  `ui::scrolls_at` from the same layout the drawing uses, handled in `App::on_mouse`, and
  a limit that keeps its scroll from passing its end, from the lines the drawing lays out;
  the table of section 11.1 of `docs/DESIGN.md` says what a notch does there.
- **Colours**: roles of `theme.rs`, never colours. A new role gets its colour in every
  theme, from the palette or a mix of two of its colours, and its contrast is added to the
  tests of `theme.rs`. Meaning is never carried by colour alone: every state has a word or
  a shape too.
- **Characters**: from `glyphs.rs`, which has a Unicode set of WGL4 characters and a
  7-bit ASCII one. A test draws every screen in ASCII and fails on any other character.

## A tab

1. **The state**: a module in `app/`, a field of `App`, and the tab in `Tab::ALL` with its
   title. The header and `Tab` pick it up from there.
2. **Its keys**: a `Place` in `keys.rs`, mapped in `App::places`, with the rows of the
   table, and its actions in an `act_on_*` function.
3. **Its drawing**: a module in `ui/`, dispatched from `ui::draw`, with a line of key hints
   at the bottom like the other tabs.
4. **Its memory**: anything that grows with use is a `Ring` with a limit in entries and a
   limit in bytes, declared in `limits.rs` with the reason for its size, and the tab says
   how many entries it dropped (ADR 0006). The limit also goes in the table of section 13
   of `docs/DESIGN.md`, and of `docs/OPEN_QUESTIONS.md` while it is not settled.
5. **Its screens**: a scene in `SCENES` of `tests/screens.rs`, which then draws it at both
   sizes, in every theme, without colour and in ASCII.

## Then, every time

```bash
cargo nextest run --workspace
cargo insta review
cargo xtask screenshots
cargo xtask ci
```

Review every snapshot that changed before accepting it; if the README shows the screen,
redraw its pictures. Write the entry in `CHANGELOG.md` in the same commit as the code,
which CI checks, and update every other document the change affects, which the pull
request template asks. Then one commit, in Conventional Commits, with a body that says why.
