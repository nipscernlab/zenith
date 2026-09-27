//! ZENITH, the terminal of Constellation: a full-screen client of SOLAR.
//!
//! The binary in `main.rs` owns the terminal and the loop. Everything it draws and every
//! decision it makes lives in this library, so that the tests can drive the same code
//! with ratatui's test backend and a real SOLAR. `docs/DESIGN.md` is the specification.

pub mod brand;
pub mod clock;
pub mod commands;
pub mod completion;
pub mod editor;
pub mod glyphs;
pub mod keys;
pub mod limits;
pub mod ring;
pub mod theme;

/// The version of this build, from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// One line saying what this program is, for `--help`.
pub const ABOUT: &str = "ZENITH, the terminal of Constellation, from NIPS-CERN. \
    A full-screen client of SOLAR, which it starts as `solar serve --stdio`.";

/// The line `zenith --version` prints, which is also how a person with the system monitor
/// of the same name tells the two apart.
#[must_use]
pub fn version_line() -> String {
    format!("ZENITH {VERSION}, the terminal of Constellation, NIPS-CERN")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_line_names_the_program_the_version_and_the_laboratory() {
        let line = version_line();
        assert!(line.starts_with("ZENITH "));
        assert!(line.contains(VERSION));
        assert!(line.contains("NIPS-CERN"));
    }
}
