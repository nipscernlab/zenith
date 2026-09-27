//! Every limit on what ZENITH keeps, in one place, each with the reason for its size.
//!
//! A terminal stays open for hours, so nothing may grow without end (ADR 0006). Each
//! collection that grows with use is a ring buffer with a limit in entries and a limit in
//! bytes, and drops its oldest entries when either would be passed. `docs/DESIGN.md`,
//! section 13, and `docs/OPEN_QUESTIONS.md` give the same numbers; a test holds them
//! together.

/// One mebibyte.
pub const MIB: usize = 1024 * 1024;

/// The calls the History keeps. A day of testing by hand is a few thousand calls.
pub const HISTORY_ENTRIES: usize = 10_000;

/// The bytes the History keeps: ten thousand typical calls with room for large responses.
pub const HISTORY_BYTES: usize = 32 * MIB;

/// The lines the Log keeps. At `trace`, SOLAR writes two lines a call and a few more, so
/// this is the last several thousand calls.
pub const LOG_ENTRIES: usize = 20_000;

/// The bytes the Log keeps.
pub const LOG_BYTES: usize = 16 * MIB;

/// The entries the Session transcript keeps. Nobody scrolls back further than that on
/// screen, and the History keeps more.
pub const TRANSCRIPT_ENTRIES: usize = 1_000;

/// The bytes the Session transcript keeps.
pub const TRANSCRIPT_BYTES: usize = 16 * MIB;

/// The lines the command line history keeps. It is a convenience, not a record.
pub const COMMAND_HISTORY_ENTRIES: usize = 500;

/// The bytes the command line history keeps.
pub const COMMAND_HISTORY_BYTES: usize = 256 * 1024;

/// The calls that may wait for SOLAR at once. More would be a script, not a person.
pub const CALLS_IN_FLIGHT: usize = 256;

/// The lines of the Log a `/report` carries: the recent past, not the whole session.
pub const REPORT_LOG_LINES: usize = 1_000;

/// The bookkeeping an entry costs beyond the text it holds, counted in its size so that a
/// flood of empty entries still meets the byte limit.
pub const ENTRY_OVERHEAD: usize = 64;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::log::group;

    #[test]
    fn the_design_and_the_open_questions_give_the_limits_the_code_keeps() {
        let design = include_str!("../../../docs/DESIGN.md");
        let open = include_str!("../../../docs/OPEN_QUESTIONS.md");
        let mib = |bytes: usize| format!("{} MiB", bytes / MIB);
        let command_line = format!(
            "{} lines | {} KiB |",
            group(COMMAND_HISTORY_ENTRIES),
            COMMAND_HISTORY_BYTES / 1024
        );
        let rows = [
            format!(
                "| History | {} calls | {} |",
                group(HISTORY_ENTRIES),
                mib(HISTORY_BYTES)
            ),
            format!(
                "| Log | {} lines | {} |",
                group(LOG_ENTRIES),
                mib(LOG_BYTES)
            ),
            format!("| Command line history | {command_line}"),
            format!("| Calls in flight | {CALLS_IN_FLIGHT} |"),
        ];
        for row in &rows {
            assert!(
                design.contains(row.as_str()),
                "docs/DESIGN.md does not have {row}"
            );
            assert!(
                open.contains(row.as_str()),
                "docs/OPEN_QUESTIONS.md does not have {row}"
            );
        }
        let transcript = format!(
            "{} entries | {} |",
            group(TRANSCRIPT_ENTRIES),
            mib(TRANSCRIPT_BYTES)
        );
        assert!(design.contains(&format!("| Session transcript | {transcript}")));
        assert!(open.contains(&format!("| Transcript | {transcript}")));
    }
}
