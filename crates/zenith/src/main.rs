//! The `zenith` binary: the command line, the terminal, and the loop.

use std::io::{IsTerminal as _, Write as _};
use std::path::PathBuf;
use std::process::ExitCode;
use std::thread;
use std::time::Instant;

use clap::Parser;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use zenith::app::{Incoming, Options};
use zenith::command_history::{self, System};
use zenith::glyphs::Charset;
use zenith::keys::LevelKey;
use zenith::runtime::{Runtime, Timings};
use zenith::terminal;
use zenith::theme::{Depth, ThemeName};

/// The command line of `zenith`.
#[derive(Debug, Parser)]
#[command(
    name = "zenith",
    about = zenith::ABOUT,
    long_about = None,
    disable_version_flag = true,
    after_help = "Environment: ZENITH_SOLAR, ZENITH_THEME, ZENITH_COLOR, ZENITH_ASCII, NO_COLOR, \
                  ZENITH_NO_HISTORY, ZENITH_DATA_DIR. docs/DESIGN.md in nipscernlab/zenith says \
                  what each does."
)]
struct Args {
    /// The solar program to start. Without it, `ZENITH_SOLAR`, then the PATH.
    #[arg(long, value_name = "PATH")]
    solar: Option<PathBuf>,

    /// The theme: night, light or high-contrast. Without it, `ZENITH_THEME`, then night.
    #[arg(long, value_name = "NAME", value_parser = parse_theme)]
    theme: Option<ThemeName>,

    /// The colours the terminal draws: auto, truecolor, 256, 16 or none. It beats
    /// `NO_COLOR`, which beats `ZENITH_COLOR`.
    #[arg(long, value_name = "DEPTH", value_parser = parse_depth)]
    color: Option<ColourChoice>,

    /// Draw with 7-bit ASCII only, for terminals without Unicode. Also `ZENITH_ASCII`.
    #[arg(long)]
    ascii: bool,

    /// The level SOLAR is started at: error, warn, info, debug or trace. The Log tab
    /// filters below it.
    #[arg(long, value_name = "LEVEL", default_value = "trace", value_parser = parse_level)]
    solar_log: String,

    /// Keep the command line history for this session only: nothing is read from its file or
    /// written to it. Also `ZENITH_NO_HISTORY`.
    #[arg(long)]
    no_history: bool,

    /// Print the version and what this program is, and exit.
    #[arg(short = 'V', long)]
    version: bool,
}

#[derive(Debug, Clone, Copy)]
enum ColourChoice {
    Auto,
    Fixed(Depth),
}

fn parse_theme(value: &str) -> Result<ThemeName, String> {
    ThemeName::parse(value).ok_or_else(|| {
        format!(
            "there is no theme {value}; the themes are {}",
            ThemeName::ALL.map(ThemeName::name).join(", ")
        )
    })
}

fn parse_depth(value: &str) -> Result<ColourChoice, String> {
    if value.eq_ignore_ascii_case("auto") {
        return Ok(ColourChoice::Auto);
    }
    Depth::parse(value)
        .map(ColourChoice::Fixed)
        .ok_or_else(|| format!("{value} is not auto, truecolor, 256, 16 or none"))
}

fn parse_level(value: &str) -> Result<String, String> {
    if value.eq_ignore_ascii_case("off") {
        return Ok("off".to_owned());
    }
    LevelKey::parse(value)
        .map(|level| level.name().to_owned())
        .ok_or_else(|| format!("{value} is not off, error, warn, info, debug or trace"))
}

fn variable(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn options(args: &Args) -> Options {
    let depth = match args.color {
        Some(ColourChoice::Fixed(depth)) => depth,
        _ if variable("NO_COLOR").is_some() => Depth::None,
        _ => variable("ZENITH_COLOR")
            .and_then(|value| Depth::parse(&value))
            .unwrap_or_else(|| Depth::guess(|name| std::env::var(name).ok())),
    };
    let theme = args
        .theme
        .or_else(|| variable("ZENITH_THEME").and_then(|value| ThemeName::parse(&value)))
        .unwrap_or(ThemeName::Night);
    let charset = if args.ascii || variable("ZENITH_ASCII").is_some() {
        Charset::Ascii
    } else {
        Charset::Unicode
    };
    let off = args.no_history || variable("ZENITH_NO_HISTORY").is_some();
    let file = command_history::path(System::this(), variable);
    Options {
        theme,
        depth,
        charset,
        solar: args.solar.clone(),
        solar_log: args.solar_log.clone(),
        opening: true,
        environment: Vec::new(),
        history: command_history::kept(off, file),
    }
}

fn main() -> ExitCode {
    let process_start = Instant::now();
    let args = Args::parse();
    if args.version {
        let mut stdout = std::io::stdout().lock();
        return match writeln!(stdout, "{}", zenith::version_line()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        };
    }
    if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
        let _ = writeln!(
            std::io::stderr(),
            "zenith: ZENITH draws a full-screen interface and needs a terminal, and its input \
             or output is not one. `zenith --version` works anywhere."
        );
        return ExitCode::from(2);
    }
    let options = options(&args);
    terminal::install_panic_hook();
    let (backend_terminal, guard) = match terminal::enter() {
        Ok(entered) => entered,
        Err(error) => {
            terminal::restore();
            let _ = writeln!(
                std::io::stderr(),
                "zenith: the terminal refused raw mode: {error}."
            );
            return ExitCode::FAILURE;
        }
    };
    let timings = Timings::from_environment(process_start);
    let mut runtime = Runtime::new(backend_terminal, options, timings);
    let sender = runtime.sender();

    // Keys, pastes and resizes, from a thread of their own that blocks on the terminal.
    let input = sender.clone();
    let _ = thread::Builder::new()
        .name("zenith-input".to_owned())
        .spawn(move || {
            while let Ok(event) = event::read() {
                let incoming = match event {
                    Event::Key(key) if key.kind != KeyEventKind::Release => Incoming::Key(key),
                    Event::Paste(text) => Incoming::Paste(text),
                    Event::Resize(width, height) => Incoming::Resize { width, height },
                    _ => continue,
                };
                if input.send(incoming).is_err() {
                    return;
                }
            }
        });

    // A signal, or the console closing, ends ZENITH the same way /quit does.
    let terminate = sender;
    let _ = ctrlc::set_handler(move || {
        let _ = terminate.send(Incoming::Terminate);
    });

    let result = runtime.run();
    drop(runtime);
    drop(guard);
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(
                std::io::stderr(),
                "zenith: drawing to the terminal failed: {error}."
            );
            ExitCode::FAILURE
        }
    }
}
