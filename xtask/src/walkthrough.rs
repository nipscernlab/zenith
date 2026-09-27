//! `cargo xtask walkthrough`: the table of section 4 of `docs/TESTING_BY_HAND.md`, done
//! by a machine. Each row is typed into the real `zenith` in a pseudo-terminal, against
//! the installed SOLAR, and what the row says should happen is waited for on the screen.
//! The run stops at the first row that does not do what the guide says, and shows the
//! screen as it was.
//!
//! A test holds the steps and the table together: every row of the table has its step,
//! in the same order, so the guide can neither promise what ZENITH no longer does nor
//! lose a row without this failing.

use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use crate::pty::{Session, solar};

/// The guide whose table this walks.
const GUIDE: &str = include_str!("../../docs/TESTING_BY_HAND.md");

/// The size of the terminal, larger than the smallest ZENITH needs so that a whole card
/// fits on the screen.
const COLUMNS: u16 = 120;
const ROWS: u16 = 40;

/// How long a row may take to do what it says.
const PATIENCE: Duration = Duration::from_secs(20);

/// Keys, as a terminal sends them.
const ENTER: &[u8] = b"\r";
const TAB: &[u8] = b"\t";
const ESC: &[u8] = b"\x1b";
const DOWN: &[u8] = b"\x1b[B";
const UP: &[u8] = b"\x1b[A";
const CTRL_C: &[u8] = b"\x03";
const CTRL_O: &[u8] = b"\x0f";
const CTRL_R: &[u8] = b"\x12";
const CTRL_U: &[u8] = b"\x15";

/// One row of the guide's table, and how a machine does it.
struct Step {
    /// The first cell of the row, exactly as the guide writes it.
    row: &'static str,
    /// Does what the row says, and checks what it says should happen.
    run: fn(&mut Walk) -> Result<(), String>,
}

/// The rows of the table, in its order.
const STEPS: &[Step] = &[
    Step {
        row: "`/list`",
        run: |walk| {
            walk.fresh()?;
            walk.line("/list")?;
            walk.see(" APIs \u{b7} SOLAR ")?;
            walk.see("solar.ping")
        },
    },
    Step {
        row: "`/ping hello`",
        run: |walk| {
            walk.fresh()?;
            walk.line("/ping hello")?;
            walk.see("pong in ")?;
            walk.see(" in SOLAR")?;
            walk.see("echo hello")
        },
    },
    Step {
        row: "`/version`",
        run: |walk| {
            walk.fresh()?;
            walk.line("/version")?;
            walk.see("solar_version")?;
            walk.see("solar/1")?;
            walk.see("manifest_schema_version")?;
            walk.see("rustc_version")
        },
    },
    Step {
        row: "`/describe solar.ping`",
        run: |walk| {
            walk.fresh()?;
            walk.line("/describe solar.ping")?;
            for heading in ["parameters", "errors", "examples", "description"] {
                walk.see(heading)?;
            }
            Ok(())
        },
    },
    Step {
        row: "`/call system.info`",
        run: |walk| {
            walk.fresh()?;
            walk.line("/call system.info")?;
            walk.see("os_family")?;
            walk.see("cpu_count")
        },
    },
    Step {
        row: "`/call solar.ping {\"mesage\": \"hi\"}`",
        run: |walk| {
            walk.fresh()?;
            let line = r#"/call solar.ping {"mesage": "hi"}"#;
            walk.line(line)?;
            walk.see("There is no parameter mesage. Did you mean message?")?;
            walk.underlined("\"mesage\"")?;
            walk.press(CTRL_U)?;
            walk.line(r#"/call solar.ping {"message": "hi"}"#)?;
            walk.see("pong")
        },
    },
    Step {
        row: "`/call solar.pnig`",
        run: |walk| {
            walk.fresh()?;
            walk.line("/call solar.pnig")?;
            walk.see("NOT_FOUND \u{b7} METHOD_NOT_FOUND")?;
            walk.see("solar.ping")
        },
    },
    Step {
        row: "`/de` then `Tab`",
        run: |walk| {
            walk.fresh()?;
            walk.type_text("/de")?;
            walk.press(TAB)?;
            walk.see("\u{203a} /describe ")?;
            walk.type_text("solar.p")?;
            walk.press(TAB)?;
            walk.see("\u{203a} /describe solar.ping")?;
            walk.press(CTRL_U)
        },
    },
    Step {
        row: "`Ctrl+O`",
        run: |walk| {
            walk.fresh()?;
            walk.press(CTRL_O)?;
            walk.see("request")?;
            walk.see("\"jsonrpc\"")?;
            walk.press(ESC)?;
            walk.gone("\"jsonrpc\"")
        },
    },
    Step {
        row: "`Tab`",
        run: |walk| {
            walk.fresh()?;
            walk.press(TAB)?;
            walk.see("examples")?;
            walk.press(DOWN)?;
            walk.press(UP)?;
            walk.select("solar.ping")?;
            walk.press(b"1")?;
            walk.see("matches the example")?;
            walk.press(b"2")?;
            walk.see_times("matches the example", 2)
        },
    },
    Step {
        row: "`Enter` on an API",
        run: |walk| {
            walk.press(ENTER)?;
            walk.see("solar.ping parameters")?;
            walk.press(ESC)?;
            walk.gone("solar.ping parameters")
        },
    },
    Step {
        row: "`Tab` again",
        run: |walk| {
            walk.press(TAB)?;
            walk.see("level ")?;
            walk.press(b"t")?;
            walk.see("[trace]")?;
            walk.press(b"i")?;
            walk.see("[info]")
        },
    },
    Step {
        row: "`Tab` again",
        run: |walk| {
            walk.press(TAB)?;
            walk.see("sent, UTC")?;
            walk.press(ENTER)?;
            walk.see("request  sent")?;
            walk.see("response")?;
            walk.press(ESC)?;
            walk.gone("request  sent")
        },
    },
    Step {
        row: "`/raw [{\"jsonrpc\":\"2.0\",\"id\":\"a\",\"method\":\"solar.ping\"}]`",
        run: |walk| {
            walk.fresh()?;
            walk.line(r#"/raw [{"jsonrpc":"2.0","id":"a","method":"solar.ping"}]"#)?;
            let answered = walk.see_either("element 0, id \"a\"", "UNIMPLEMENTED")?;
            walk.said(if answered == 0 {
                "SOLAR answered the batch with an array"
            } else {
                "SOLAR answered the batch with UNIMPLEMENTED"
            });
            Ok(())
        },
    },
    Step {
        row: "`/call solar.cancel {\"id\": 1}`",
        run: |walk| {
            walk.fresh()?;
            walk.line("/list")?;
            walk.see(" APIs \u{b7} SOLAR ")?;
            if !walk.shows("solar.cancel") {
                walk.said("this SOLAR has no solar.cancel, so the row does not apply");
                return Ok(());
            }
            walk.fresh()?;
            walk.line(r#"/call solar.cancel {"id": 1}"#)?;
            walk.see("already_finished")
        },
    },
    Step {
        row: "`Ctrl+R`",
        run: |walk| {
            walk.fresh()?;
            walk.press(CTRL_R)?;
            walk.see("Restarting SOLAR.")?;
            walk.see("Connected to SOLAR")
        },
    },
    Step {
        row: "`/theme light`, `/theme high-contrast`, `/theme night`",
        run: |walk| {
            walk.fresh()?;
            let night = walk.background()?;
            walk.line("/theme light")?;
            walk.see("The theme is now light.")?;
            let light = walk.background()?;
            walk.line("/theme high-contrast")?;
            walk.see("The theme is now high-contrast.")?;
            let contrast = walk.background()?;
            walk.line("/theme night")?;
            walk.see("The theme is now night.")?;
            if night == light || light == contrast || walk.background()? != night {
                return Err(format!(
                    "the background did not change with the theme: night {night:?}, light \
                     {light:?}, high-contrast {contrast:?}"
                ));
            }
            Ok(())
        },
    },
    Step {
        row: "`?` on an empty line",
        run: |walk| {
            walk.fresh()?;
            walk.press(b"?")?;
            walk.see("Everywhere")?;
            walk.see("The command line")?;
            walk.press(ESC)?;
            walk.gone("Everywhere")
        },
    },
    Step {
        row: "Make the window smaller than 80 × 24",
        run: |walk| {
            walk.session.resize(70, 20)?;
            walk.see("ZENITH needs 80 \u{d7} 24, and this terminal is 70 \u{d7} 20.")?;
            walk.session.resize(COLUMNS, ROWS)?;
            walk.gone("ZENITH needs")?;
            walk.see("in orbit")
        },
    },
    Step {
        row: "`Ctrl+C`, then `Ctrl+C` again",
        run: |walk| {
            walk.fresh()?;
            walk.press(CTRL_C)?;
            walk.see("Press Ctrl+C again to quit")?;
            walk.press(CTRL_C)?;
            walk.session.exited(PATIENCE)?;
            if walk.session.restored() {
                Ok(())
            } else {
                Err(
                    "zenith exited without giving the terminal back: the alternate screen or \
                     the hidden cursor is still set"
                        .to_owned(),
                )
            }
        },
    },
];

/// A session being walked through.
struct Walk {
    session: Session,
    notes: Vec<String>,
}

impl Walk {
    /// Bytes, as keys pressed, and a moment for the screen to follow.
    fn press(&mut self, keys: &[u8]) -> Result<(), String> {
        self.session.write(keys)?;
        thread::sleep(Duration::from_millis(60));
        Ok(())
    }

    /// Text typed a character at a time, as a person types it.
    fn type_text(&mut self, text: &str) -> Result<(), String> {
        for character in text.chars() {
            let mut buffer = [0_u8; 4];
            self.session
                .write(character.encode_utf8(&mut buffer).as_bytes())?;
            thread::sleep(Duration::from_millis(4));
        }
        thread::sleep(Duration::from_millis(60));
        Ok(())
    }

    /// A line typed and run.
    fn line(&mut self, text: &str) -> Result<(), String> {
        self.type_text(text)?;
        self.press(ENTER)
    }

    /// An empty transcript on the Session tab, so that what a row waits for cannot be
    /// something an earlier row left on the screen.
    fn fresh(&mut self) -> Result<(), String> {
        self.line("/clear")?;
        self.see("Type / for the commands")
    }

    fn see(&self, text: &str) -> Result<(), String> {
        self.session.wait_for(text, PATIENCE).map(drop)
    }

    fn gone(&self, text: &str) -> Result<(), String> {
        self.session.wait_until_gone(text, PATIENCE)
    }

    fn shows(&self, text: &str) -> bool {
        self.session.screen().contains(text)
    }

    /// Waits for `text` to be on the screen `times` times.
    fn see_times(&self, text: &str, times: usize) -> Result<(), String> {
        let from = std::time::Instant::now();
        while self.session.screen().matches(text).count() < times {
            if from.elapsed() > PATIENCE {
                return Err(format!(
                    "{text:?} was not on the screen {times} times; the screen was:\n{}",
                    self.session.dump()
                ));
            }
            thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }

    /// Waits for one of two texts, and says which came.
    fn see_either(&self, first: &str, second: &str) -> Result<usize, String> {
        let from = std::time::Instant::now();
        loop {
            let screen = self.session.screen();
            if screen.contains(first) {
                return Ok(0);
            }
            if screen.contains(second) {
                return Ok(1);
            }
            if from.elapsed() > PATIENCE {
                return Err(format!(
                    "neither {first:?} nor {second:?} appeared; the screen was:\n{}",
                    self.session.dump()
                ));
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    /// Moves the selection of a list to the entry that shows `name`.
    fn select(&mut self, name: &str) -> Result<(), String> {
        self.press(b"g")?;
        for _ in 0..64 {
            if self.shows(&format!("\u{203a} {name} ")) {
                return Ok(());
            }
            self.press(DOWN)?;
        }
        Err(format!(
            "{name} could not be selected; the screen was:\n{}",
            self.session.dump()
        ))
    }

    /// Checks that `text` is underlined where it stands on the command line, the last row
    /// of the screen that shows it.
    fn underlined(&self, text: &str) -> Result<(), String> {
        let rows = self.session.rows();
        let Some((row, line)) = rows
            .iter()
            .enumerate()
            .rev()
            .find(|(_, line)| line.contains(text))
        else {
            return Err(format!("{text:?} is not on the screen"));
        };
        let Some(at) = line.find(text) else {
            return Err(format!("{text:?} is not on the screen"));
        };
        let first = u16::try_from(line[..at].chars().count()).unwrap_or(u16::MAX);
        let length = u16::try_from(text.chars().count()).unwrap_or(u16::MAX);
        let row = u16::try_from(row).unwrap_or(u16::MAX);
        let underlined = (first..first + length).all(|column| {
            self.session
                .cell(row, column)
                .is_some_and(|(_, under)| under)
        });
        if underlined {
            Ok(())
        } else {
            Err(format!("{text:?} is on the screen but not underlined"))
        }
    }

    /// The background of the transcript, where nothing is written.
    fn background(&self) -> Result<vt100::Color, String> {
        self.session
            .cell(ROWS / 2, COLUMNS - 2)
            .map(|(background, _)| background)
            .ok_or_else(|| "the screen has no cell there".to_owned())
    }

    /// Something worth reporting about a row that passed.
    fn said(&mut self, note: &str) {
        self.notes.push(note.to_owned());
    }
}

/// Walks the table. Without a SOLAR it says so, and fails only when
/// `ZENITH_REQUIRE_SOLAR` is set, as the tests do.
///
/// # Errors
///
/// The first row that did not do what the guide says.
pub(crate) fn run(root: &Path) -> Result<(), String> {
    let rows = rows_of_the_guide(GUIDE);
    if !rows
        .iter()
        .map(String::as_str)
        .eq(STEPS.iter().map(|step| step.row))
    {
        return Err(format!(
            "the steps of the walkthrough are not the rows of the guide, which are: {}",
            rows.join(", ")
        ));
    }
    let solar = match solar() {
        Ok(solar) => solar,
        Err(why) if std::env::var_os("ZENITH_REQUIRE_SOLAR").is_none() => {
            println!("walkthrough: skipped, because there is no SOLAR to walk against: {why}");
            return Ok(());
        }
        Err(why) => return Err(why),
    };
    let zenith = debug_binary(root)?;
    let solar_text = solar.display().to_string();
    let session = Session::start(&zenith, &["--solar", &solar_text], &[], COLUMNS, ROWS)?;
    let mut walk = Walk {
        session,
        notes: Vec::new(),
    };
    walk.see("in orbit")?;
    for (index, step) in STEPS.iter().enumerate() {
        let notes = walk.notes.len();
        (step.run)(&mut walk)
            .map_err(|why| format!("row {} of the table, {}: {why}", index + 1, step.row))?;
        let said = walk.notes[notes..].join("; ");
        if said.is_empty() {
            println!("walkthrough: {:>2} {}", index + 1, step.row);
        } else {
            println!("walkthrough: {:>2} {}: {said}", index + 1, step.row);
        }
    }
    println!(
        "walkthrough: the {} rows of section 4 of docs/TESTING_BY_HAND.md do what they say",
        STEPS.len()
    );
    Ok(())
}

/// The `zenith` of the current code, built now. The walkthrough checks behaviour, not
/// speed, so it is the development build, which the tests have already compiled.
fn debug_binary(root: &Path) -> Result<PathBuf, String> {
    let status = std::process::Command::new("cargo")
        .args(["build", "--locked", "-p", "zenith", "--bin", "zenith"])
        .current_dir(root)
        .status()
        .map_err(|error| format!("cargo could not be started: {error}"))?;
    if !status.success() {
        return Err("the build of zenith failed".to_owned());
    }
    Ok(root
        .join("target")
        .join("debug")
        .join(format!("zenith{}", std::env::consts::EXE_SUFFIX)))
}

/// The first cell of every row of the table in section 4 of the guide.
fn rows_of_the_guide(guide: &str) -> Vec<String> {
    let section = guide
        .split("\n## ")
        .find(|section| section.starts_with("4. "))
        .unwrap_or_default();
    section
        .lines()
        .filter(|line| line.starts_with("| "))
        .skip(2)
        .filter_map(|line| line.strip_prefix("| "))
        .filter_map(|line| line.split(" | ").next())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_of_the_guide_has_its_step_in_the_same_order() {
        let rows = rows_of_the_guide(GUIDE);
        let steps: Vec<&str> = STEPS.iter().map(|step| step.row).collect();
        assert_eq!(rows, steps);
    }
}
