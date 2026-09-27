# How this repository is written

Code is read far more often than it is written, and this code will be read by people who
did not attend the conversation that produced it, in laboratories that are not this one,
and by agents that cannot ask what something meant. These rules exist for them. They are
the rules of SOLAR, so that a person who has read one repository has read both, with the
additions a terminal interface needs.

They are rules, not preferences. Where a tool can enforce one, it does, and the rule says
which tool.

## The language

**English, and British English**, everywhere: colour, catalogue, behaviour, licence,
serialise, artefact. `typos` runs with `locale = "en-gb"` and refuses the American
spellings.

The exceptions are never prose: an identifier the ecosystem fixes, such as serde's
`Serialize` or the `NO_COLOR` variable, and a flag every command line tool spells the
same way, such as `--color`. Those are listed in `typos.toml` with the reason beside them.

The one file that is not English only is `LICENSE`, which is the laboratory's base
licence copied byte for byte, in Portuguese with an English translation.

Write plainly. Short sentences. No emoji, no exclamation marks, no dashes used as
punctuation, no filler: "it is important to note that", "simply", "just", "obviously".
If a sentence would survive being deleted, delete it.

## Comments

A comment says **why**, because the code already says what. A comment that restates the
line above it is noise that will one day be wrong.

```rust
// Wrong: says what the line says.
// Clear the flag.
armed = false;

// Right: says what the reader cannot see.
// Any key between two presses of Ctrl+C means the person moved on, so the second press
// must not quit.
armed = false;
```

Three kinds of comment are always worth writing:

1. **The reason for a decision** that a reader would otherwise want to undo.
2. **The measurement behind a choice**: `jsonschema took the binary from 775 KB to 4.6 MB`.
3. **The contract section** a piece of code implements, when the contract is the reason:
   `section 9.5 of SOLAR's contract: an id is never reused while its call is alive`.

Every `#[allow]` carries `reason = "..."`, which `clippy::allow_attributes_without_reason`
requires. A lint is never silenced without a written argument, and never at workspace
level when it can be silenced at the one place it fires. `unsafe` is forbidden in the
whole workspace, and nothing here needs it.

## Documentation comments

Every public item has one; `missing_docs` is a warning and CI turns warnings into errors.

- The first line is a sentence that stands alone, because it is what `cargo doc` lists.
- A function that returns `Result` has an `# Errors` section saying what it returns and
  when.
- An example that would help is a doctest, so that it cannot rot.

## What ZENITH says to a person

Every sentence on screen is read by somebody in the middle of something, often somebody
testing SOLAR who is already looking at a failure. The rules for SOLAR's error messages
hold for ZENITH's:

- **Say what happened, then what to do.** `ZENITH could not find solar on the PATH.` is
  followed by the three ways to point it at one.
- **Name the thing.** The path, the version, the exit code, the parameter. Never
  "something went wrong".
- **Quote SOLAR exactly.** When a message comes from SOLAR, it is shown as SOLAR wrote it,
  never paraphrased, because the person may be testing that very message.
- **Never rely on colour.** A state has a word or a shape as well.
- **Sentences start with a capital and end with a full stop.** Labels, badges and column
  headers are lower case and have no full stop.

## Naming

- A test is named as a sentence that says what must be true:
  `a_response_with_an_unknown_id_is_shown_as_unexpected`. When it fails, the name is the
  bug report.
- Avoid abbreviations. `request` rather than `req`, `response` rather than `resp`,
  `connection` rather than `conn`.
- A snapshot is named after the screen and the state it shows, and the size and theme
  are part of the name: `session_with_an_error__80x24__night`.

## Commit messages

[Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/). The subject is
lower case, imperative, under seventy-two characters:

```text
feat(client): start solar from a copy keyed by its hash on Windows
fix(ui): the completion menu covered the last line of the transcript
docs: the design, before the code
```

The types in use are `feat`, `fix`, `docs`, `test`, `build`, `ci`, `style`, `refactor`,
`perf` and `chore`. The scope is the crate or the area: `client`, `app`, `ui`, `theme`,
`xtask`, `ci`.

The body says **why**, in prose, wrapped at seventy-six characters. It is the only place
where the reasoning behind a change survives, so it carries the measurement that prompted
it, the alternative that was rejected, and anything the diff cannot show. A commit that
fixes something says how it was found.

One commit per finished, tested step. `cargo xtask ci` green before pushing.

## What the tools decide, so nobody argues about it

| Thing | Tool |
| ----- | ---- |
| Formatting of Rust | `cargo fmt`, with `rustfmt.toml` |
| Formatting of TOML | `taplo fmt`, with `taplo.toml` |
| Line endings, final newlines, trailing spaces | `.editorconfig` |
| Spelling | `typos`, with `typos.toml` |
| Lints, including pedantic | `clippy`, configured in `Cargo.toml` and `clippy.toml` |
| What the screen looks like | the snapshots in `crates/zenith/tests/snapshots/` |

All of them run in `cargo xtask ci` and in CI.
