//! The terminal: taken over at the start, and always given back, even on a panic.
//!
//! Raw mode, the alternate screen and bracketed paste are turned on together and off
//! together. Giving the terminal back is done in three places, and doing it twice is
//! harmless: when the guard is dropped, in the panic hook before the panic is printed,
//! and when a signal or the console closing ends the process.

use std::io::{self, Stdout, Write};

use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{DisableBracketedPaste, EnableBracketedPaste};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

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

/// Gives the terminal back: the main screen, a visible cursor, no raw mode.
pub fn restore() {
    let mut stdout = io::stdout();
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
