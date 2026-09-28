//! The terminal: taken over at the start, and always given back, even on a panic.
//!
//! Raw mode, the alternate screen and bracketed paste are turned on together and off
//! together. Giving the terminal back is done in three places, and doing it twice is
//! harmless: when the guard is dropped, in the panic hook before the panic is printed,
//! and when a signal or the console closing ends the process.
//!
//! While ZENITH runs, the terminal's own background is the theme's, with OSC 11, so that
//! the margin a terminal keeps around its cells, where no program can draw, is the colour
//! of the screen; giving the terminal back puts its own background back, with OSC 111.
//!
//! ZENITH takes the mouse for the wheel unless it was told not to, and gives it back with
//! the rest, and whenever `/mouse` says so (ADR 0015).

use std::io::{self, Stdout, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{DisableBracketedPaste, EnableBracketedPaste};
#[cfg(windows)]
use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

use crate::theme::Rgb;

/// Whether ZENITH set the terminal's background, and so has to put the terminal's own back.
static PAINTED: AtomicBool = AtomicBool::new(false);

/// OSC 111: the terminal's own background again.
pub const OWN_BACKGROUND: &str = "\x1b]111\x1b\\";

/// Whether ZENITH has the mouse, and so has to give it back.
static CAPTURED: AtomicBool = AtomicBool::new(false);

/// The mouse, asked of the terminal: a press and a release of each button, the wheel's
/// notches among them, and no movement, which ZENITH has no use for and would only wake it;
/// in SGR's encoding, which has no limit on the column. Windows' console is asked through
/// its own interface instead, and reports movement whatever it is asked.
pub const MOUSE_ON: &str = "\x1b[?1000h\x1b[?1006h";

/// The mouse, given back to the terminal.
pub const MOUSE_OFF: &str = "\x1b[?1006l\x1b[?1000l";

/// Takes the mouse from the terminal, for the wheel, or gives it back, for selecting text.
pub fn set_mouse(on: bool) {
    let mut stdout = io::stdout();
    #[cfg(windows)]
    {
        let _ = if on {
            execute!(stdout, EnableMouseCapture)
        } else {
            execute!(stdout, DisableMouseCapture)
        };
    }
    #[cfg(not(windows))]
    {
        let _ = stdout.write_all(if on { MOUSE_ON } else { MOUSE_OFF }.as_bytes());
        let _ = stdout.flush();
    }
    CAPTURED.store(on, Ordering::SeqCst);
}

/// OSC 11, which sets the terminal's background, and with it the colour of its margin.
#[must_use]
pub fn background_sequence(rgb: Rgb) -> String {
    format!(
        "\x1b]11;rgb:{:02x}/{:02x}/{:02x}\x1b\\",
        rgb.0, rgb.1, rgb.2
    )
}

/// Sets the terminal's background to `rgb`, and remembers to put the terminal's own back.
pub fn paint_background(rgb: Rgb) {
    let mut stdout = io::stdout();
    let _ = stdout.write_all(background_sequence(rgb).as_bytes());
    let _ = stdout.flush();
    PAINTED.store(true, Ordering::SeqCst);
}

/// Gives the terminal back when dropped.
#[derive(Debug)]
pub struct Guard;

impl Drop for Guard {
    fn drop(&mut self) {
        restore();
    }
}

/// Takes the terminal over and returns it with the guard that gives it back.
///
/// # Errors
///
/// What the terminal returns when it cannot be put in raw mode or switched to the
/// alternate screen; in that case whatever was done is undone first.
pub fn enter() -> io::Result<(Terminal<CrosstermBackend<Stdout>>, Guard)> {
    enable_raw_mode()?;
    let guard = Guard;
    execute!(io::stdout(), EnterAlternateScreen, EnableBracketedPaste)?;
    let terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    Ok((terminal, guard))
}

/// Gives the terminal back: its mouse, its own background, the main screen, a visible
/// cursor, no raw mode. The mouse goes back first: on Windows, giving it back restores the
/// console's mode of before, which leaving raw mode then builds on.
pub fn restore() {
    if CAPTURED.load(Ordering::SeqCst) {
        set_mouse(false);
    }
    let mut stdout = io::stdout();
    if PAINTED.swap(false, Ordering::SeqCst) {
        let _ = stdout.write_all(OWN_BACKGROUND.as_bytes());
    }
    let _ = execute!(
        stdout,
        DisableBracketedPaste,
        LeaveAlternateScreen,
        ratatui::crossterm::cursor::Show
    );
    let _ = disable_raw_mode();
    let _ = stdout.flush();
}

/// Installs a panic hook that gives the terminal back before the panic is printed, so
/// that the message is readable and the shell works afterwards.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        let _ = writeln!(
            io::stderr(),
            "ZENITH {} stopped because of a bug. Please send this message to \
             chrysthofer.afonso@cern.ch, with what you were doing.",
            crate::VERSION
        );
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_background_is_set_with_osc_11_and_given_back_with_osc_111() {
        assert_eq!(
            background_sequence(Rgb(0x0B, 0x0C, 0x14)),
            "\u{1b}]11;rgb:0b/0c/14\u{1b}\\"
        );
        assert_eq!(
            background_sequence(Rgb(255, 0, 16)),
            "\u{1b}]11;rgb:ff/00/10\u{1b}\\"
        );
        assert_eq!(OWN_BACKGROUND, "\u{1b}]111\u{1b}\\");
    }

    #[test]
    fn the_mouse_is_asked_for_its_buttons_in_sgr_and_given_back_in_the_reverse_order() {
        assert_eq!(MOUSE_ON, "\u{1b}[?1000h\u{1b}[?1006h");
        assert_eq!(MOUSE_OFF, "\u{1b}[?1006l\u{1b}[?1000l");
    }
}
