# Testing SOLAR by hand, through ZENITH

This guide is for someone who has never used Rust. It installs what is needed, builds
SOLAR and ZENITH, goes through what to try, and says exactly what to send back when
something is not right. It takes about half an hour the first time, most of it waiting
for the first build.

SOLAR's automated checks are in SOLAR's own guide, `docs/TESTING_BY_HAND.md` in
[nipscernlab/solar](https://github.com/nipscernlab/solar); this one is the interactive
part, which is how SOLAR is tested by hand on Windows, on Linux and on the laboratory Mac
(ADR 0004).

The steps marked **Mac only** are for the laboratory Mac; everything else is the same on
the three systems. CI does every row of the table in section 4 on Windows, Linux and
macOS, typing it into ZENITH in a pseudo-terminal with `cargo xtask walkthrough`. If a
step does not do what it says on your machine, that is worth reporting too.

## 1. Install the tools

### On a Mac

1. **Mac only.** Install Apple's command line tools, which Rust needs to link programs.
   Open Terminal and type:

   ```bash
   xcode-select --install
   ```

   A window asks to install them; accept, and wait until it finishes. If it says they are
   already installed, go on.

2. Install Rust, with rustup, the official installer:

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

   Press `Enter` to accept the default installation. Then close Terminal and open it again,
   so that it finds the new commands, and check:

   ```bash
   cargo --version
   ```

   It prints a line such as `cargo 1.97.1`. Any version is fine: the repositories choose
   their own, and `cargo` fetches it the first time it builds.

3. Git comes with the command line tools. Check with `git --version`.

### On Linux

The same, without step 1: install Rust with the `curl` line above. A C compiler is needed
to link, and most distributions have one; if the build says `linker cc not found`, install
`gcc` with the system's package manager, for example `sudo dnf install gcc` on AlmaLinux.

### On Windows

Install Rust from [rustup.rs](https://rustup.rs), which also offers to install the
Microsoft C++ build tools it needs; accept. Install Git from
[git-scm.com](https://git-scm.com). Use Windows Terminal, which comes with Windows 11.

## 2. Build SOLAR and ZENITH

Put both in one folder:

```bash
mkdir -p ~/constellation && cd ~/constellation
git clone https://github.com/nipscernlab/solar
git clone https://github.com/nipscernlab/zenith
```

To test a particular version, check it out in each repository; for example, for the
version of ZENITH this guide came with:

```bash
cd ~/constellation/zenith && git checkout v0.2.0 && cd ..
```

Build SOLAR, then ZENITH. The first build of each downloads the compiler and the
libraries and takes several minutes; later builds take seconds.

```bash
cd ~/constellation/solar && cargo build --release --locked -p solar-cli
cd ~/constellation/zenith && cargo build --release --locked -p zenith
```

Each ends with a line starting `Finished`. If either ends with `error`, stop here and send
what it printed (section 5).

## 3. Start ZENITH

Make the terminal window at least 80 columns wide and 24 rows tall, which is the size a
new Terminal window already has, and start ZENITH, telling it where SOLAR is:

```bash
cd ~/constellation/zenith
./target/release/zenith --solar ../solar/target/release/solar
```

On Windows, in PowerShell:

```powershell
cd ~\constellation\zenith
.\target\release\zenith.exe --solar ..\solar\target\release\solar.exe
```

A starfield with ZENITH's mark, the dome of an observatory, shows while SOLAR starts,
which on a fast machine is too quick to see. Then the Session tab shows SOLAR's mark, a
disc cut by two slots, with SOLAR's version beside it, under the opening's stars, and says
`Connected to SOLAR <version> in <time>: protocol solar/1, manifest 2.0.0, <n> APIs.`

## 4. What to try

Type each line and press `Enter`. The right column is what should happen.

| Type or press | What should happen |
| ------------- | ------------------ |
| `/list` | A table of every API SOLAR has, with its version and summary |
| `/ping hello` | A line starting `ok pong in`, the round trip, SOLAR's own time, and `echo hello` |
| `/version` | SOLAR's mark with `The central API of the Constellation` beside it, then SOLAR's version, `solar/1`, `2.0.0`, and how SOLAR was built |
| `/describe solar.ping` | Everything about `solar.ping`: parameters, errors, examples, description |
| `/call system.info` | The operating system, the processor and the process, as SOLAR sees them |
| `/call solar.ping {"mesage": "hi"}` | Nothing is sent. A red band says `There is no parameter mesage. Did you mean message?`, and `"mesage"` is underlined on the line. Fix it and press `Enter` again |
| `/call solar.pnig` | SOLAR's own answer: an error `NOT_FOUND · METHOD_NOT_FOUND`, suggesting `solar.ping` |
| `/de` then `Tab` | The line becomes `/describe `; type `solar.p`, press `Tab` again, and it becomes `/describe solar.ping` |
| `Ctrl+O` | The whole envelope of the last call, exactly as it crossed the pipe. `Esc` closes it |
| `Tab` | The APIs tab. `↓` and `↑` move through the APIs; `1` runs the first example of the one selected, `2` the second, and each says `matches the example` |
| `Enter` on an API | A form with a field for each parameter. `Esc` closes it |
| `Tab` again | The Log tab. `t` shows everything SOLAR writes, `i` less. A SOLAR that has `solar.set_log_level` also logs at the level chosen, and the header says `SOLAR logs at info` |
| `Tab` again | The History tab: every call with its timing. `Enter` shows one whole |
| `/raw [{"jsonrpc":"2.0","id":"a","method":"solar.ping"}]` | A batch. A SOLAR that has batches answers with one card per element; one that has not answers `UNIMPLEMENTED` |
| `/call solar.cancel {"id": 1}` | Only on a SOLAR that has `solar.cancel`: an answer saying what happened to call 1, `already_finished` |
| `Ctrl+R` | `Restarting SOLAR.`, then `Connected to SOLAR` again |
| `/theme light`, `/theme high-contrast`, `/theme night` | The colours change |
| `?` on an empty line | Every key, by where it works. `Esc` closes it |
| The mouse wheel, over the transcript, the APIs and the help | It scrolls what is under the pointer: the transcript, the list of APIs one API a notch, the entry beside it, the help. It never brings back an earlier command line; `↑` and `↓` do that |
| `Shift` and drag, or `Option` and drag in iTerm2 | The terminal selects the text dragged over, to copy, while ZENITH keeps the wheel. In macOS Terminal, `Cmd+R` turns its mouse reporting off and on instead |
| `/mouse off`, then `/mouse on` | `The terminal has the mouse`: dragging selects text with no key held, and the wheel is the terminal's. `/mouse on` gives it back to ZENITH |
| Make the window smaller than 80 × 24 | One sentence saying how large ZENITH needs the window to be. Make it larger and the screen comes back |
| `Ctrl+C`, then `Ctrl+C` again | The first says `Press Ctrl+C again to quit`; the second quits, and the terminal works normally afterwards |
| Start ZENITH again, and press `↑` | The last line you ran before quitting is back on the command line: ZENITH keeps what you type there between sessions, in the file section 7 names |
| `/export` | `Wrote the recording of this connection`, and the file it names, in SOLAR's recording format. In another terminal, `solar replay` on that file sends its requests again and ends `0 answers differ`; a SOLAR older than the format refuses the file's first line |
| `/forget` | `Forgot the` number of lines `of the command line history, here and in` that file; `↑` then brings nothing back |

**Mac only.** In macOS Terminal, Option does not act as Alt, and nothing in ZENITH needs
it. `Home`, `End`, `PgUp` and `PgDn` scroll Terminal itself; `g`, `G`, `Ctrl+B` and
`Ctrl+F` do the same inside ZENITH. Please try both Terminal and iTerm2 if iTerm2 is
installed, and say in the report which you used.

## 5. When something is not right

1. **If ZENITH is still running**, type `/report` and press `Enter`. It writes one file in
   the folder ZENITH was started from, named `zenith-report-` and the time, with the
   versions of ZENITH and SOLAR, the system as SOLAR reports it, the recent log and every
   call of the session. That file is the most useful thing you can send.
2. **If ZENITH stopped with a message**, copy the whole message, everything printed after
   it closed.
3. **If a build failed**, copy everything the `cargo build` command printed.
4. Write down **what you did, what you expected, and what happened instead**, in that
   order, and a screenshot if the screen looked wrong.
5. Add the output of these commands:

   ```bash
   ~/constellation/zenith/target/release/zenith --version
   ~/constellation/solar/target/release/solar version
   rustc --version
   ```

   and, on a Mac, `sw_vers`; on Linux, `cat /etc/os-release`; on Windows, `winver`.

Send it all to **chrysthofer.afonso@cern.ch**, with the name of the terminal you used.

## 6. Running ZENITH from anywhere

To type `zenith` and `solar` in any folder, put both `target/release` folders on the
`PATH`. On a Mac or Linux, add this line to `~/.zshrc` (Mac) or `~/.bashrc` (Linux), then
open a new terminal:

```bash
export PATH="$HOME/constellation/zenith/target/release:$HOME/constellation/solar/target/release:$PATH"
```

ZENITH then finds SOLAR by itself, and `zenith` alone is enough. On Windows, ZENITH runs a
copy of `solar.exe` from the temporary folder rather than the one that was built, so that
building SOLAR again never fails because ZENITH is open; `Ctrl+R` picks up the new build.

Another program called `zenith`, a system monitor, exists. If `zenith --version` does not
start with `ZENITH`, it is the other one, earlier on the `PATH`.

## 7. What ZENITH keeps between sessions

ZENITH keeps the lines you run on the command line, and nothing else: never a response,
never a setting. They are in one file, which it writes again after every line:

| System | The file |
| ------ | -------- |
| Windows | `%APPDATA%\nipscern-zenith\command-history.ndjson` |
| Mac | `~/Library/Application Support/nipscern-zenith/command-history.ndjson` |
| Linux | `~/.local/share/nipscern-zenith/command-history.ndjson`, or under `$XDG_DATA_HOME` when it is set |

`/forget` empties it. To keep nothing on disk for one session, start ZENITH with
`--no-history`, or set `ZENITH_NO_HISTORY=1`; to keep it somewhere else, set
`ZENITH_DATA_DIR` to a directory. If you send a report about the history, say which of
these you used.
