//! `/report`: one file with everything a bug report needs, laid out by `docs/DESIGN.md`,
//! section 12, and written line by line as it goes.

use std::io::{self, Write};
use std::time::SystemTime;

use serde_json::{Map, Value, json};

use super::App;
use super::link::Phase;
use super::log::Level;
use crate::clock::Utc;
use crate::glyphs::Charset;
use crate::limits;

/// The variables of the environment a report names, and no others: the ones that decide
/// how the terminal draws. A report never carries anything else from the environment.
pub const VARIABLES: [&str; 5] = [
    "TERM",
    "COLORTERM",
    "TERM_PROGRAM",
    "WT_SESSION",
    "NO_COLOR",
];

/// Writes a report. `variable` reads the environment, and is passed in so that the tests
/// can give their own. Returns how many calls the report holds.
///
/// # Errors
///
/// What writing returns.
pub fn write(
    app: &App,
    out: &mut impl Write,
    system: Option<&Value>,
    why: Option<&str>,
    now: SystemTime,
    variable: impl Fn(&str) -> Option<String>,
) -> io::Result<usize> {
    writeln!(
        out,
        "{}",
        json!({
            "kind": "header",
            "format": "zenith-report",
            "format_version": "1.0.0",
            "zenith_version": crate::VERSION,
            "written_at": Utc::of(now).rfc3339(),
        })
    )?;
    let environment: Map<String, Value> = VARIABLES
        .iter()
        .map(|name| {
            (
                (*name).to_owned(),
                variable(name).map_or(Value::Null, Value::String),
            )
        })
        .collect();
    writeln!(
        out,
        "{}",
        json!({
            "kind": "zenith",
            "version": crate::VERSION,
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "terminal": {
                "columns": app.size.0,
                "rows": app.size.1,
                "depth": app.theme.depth.name(),
                "unicode": app.glyphs.charset == Charset::Unicode,
                "theme": app.theme.name.name(),
            },
            "environment": environment,
        })
    )?;
    let link = &app.link;
    let binary = link.binary.as_ref();
    writeln!(
        out,
        "{}",
        json!({
            "kind": "solar",
            "found": binary.map(|prepared| prepared.found.path.display().to_string()),
            "found_by": binary.map(|prepared| prepared.found.origin.to_string()),
            "runs": binary.map(|prepared| prepared.runs.display().to_string()),
            "sha256": binary.and_then(|prepared| prepared.sha256.clone()),
            "pid": link.pid,
            "connected": link.phase == Phase::Connected,
            "version": link.info.as_ref().map(|info| json!({
                "solar_version": info.solar_version,
                "protocol": info.protocol,
                "manifest_schema_version": info.manifest_schema_version,
                "build": info.build,
            })),
            "failure": link.failure.as_ref().map(super::link::Failure::what),
        })
    )?;
    writeln!(
        out,
        "{}",
        json!({"kind": "system", "data": system, "why": why})
    )?;
    let skip = app
        .log
        .entries
        .len()
        .saturating_sub(limits::REPORT_LOG_LINES);
    for (_, entry) in app.log.entries.iter().skip(skip) {
        let level = match entry.level {
            Level::Known(level) => level.name(),
            Level::Raw => "raw",
        };
        writeln!(
            out,
            "{}",
            json!({
                "kind": "log",
                "connection": entry.connection,
                "time": entry.time,
                "level": level,
                "request_id": entry.request_id,
                "method": entry.method,
                "duration_us": entry.duration_us,
                "message": entry.message,
            })
        )?;
    }
    app.history.write_calls(out)
}
