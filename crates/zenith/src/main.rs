//! The `zenith` binary: the command line, the terminal, and the loop that drives the
//! application.

use std::io::Write as _;
use std::process::ExitCode;

use clap::Parser;

/// The command line of `zenith`.
#[derive(Debug, Parser)]
#[command(
    name = "zenith",
    about = zenith::ABOUT,
    long_about = None,
    disable_version_flag = true
)]
struct Args {
    /// Print the version and what this program is, and exit.
    #[arg(short = 'V', long)]
    version: bool,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let mut stdout = std::io::stdout().lock();
    if args.version {
        return match writeln!(stdout, "{}", zenith::version_line()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        };
    }
    let _ = writeln!(
        std::io::stderr(),
        "zenith: the interface is not in this build yet. `zenith --version` works."
    );
    ExitCode::FAILURE
}
