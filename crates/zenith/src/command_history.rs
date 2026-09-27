//! The command line history, kept between sessions in a file of its own.
//!
//! Only the lines typed on the command line are kept, never a response. The file is
//! NDJSON: a first line that names the format and its version, then one JSON string per
//! line, the oldest first, at most as many as the command line history holds
//! ([`crate::limits::COMMAND_HISTORY_ENTRIES`] and [`crate::limits::COMMAND_HISTORY_BYTES`]).
//! `docs/DESIGN.md`, section 12, specifies it.
//!
//! It is written whole every time, to a temporary file beside it that is flushed to the
//! disk and then renamed over it, so a crash leaves either the old file or the new one and
//! never half of either. It lives in the directory each system keeps per-user data in,
//! unless `ZENITH_DATA_DIR` names another.

use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::app::KeptHistory;

/// The name of the file.
pub const FILE_NAME: &str = "command-history.ndjson";

/// The directory ZENITH makes in the per-user data directory of each system.
pub const DIRECTORY_NAME: &str = "nipscern-zenith";

/// What the first line of the file says the file is.
pub const FORMAT: &str = "zenith-command-history";

/// The version of the format this ZENITH writes and reads.
pub const VERSION: u64 = 1;

/// The systems whose conventions for per-user data differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum System {
    /// `%APPDATA%`.
    Windows,
    /// `~/Library/Application Support`.
    MacOs,
    /// `$XDG_DATA_HOME`, or `~/.local/share` when it is not set.
    Unix,
}

impl System {
    /// The system this was built for.
    #[must_use]
    pub fn this() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Unix
        }
    }
}

/// Where the file is: in the directory `ZENITH_DATA_DIR` names when it is set, else in
/// [`DIRECTORY_NAME`] under the per-user data directory of `system`. `None` when the
/// variables that say where that is are not set.
#[must_use]
pub fn path(system: System, variable: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let set = |name: &str| variable(name).filter(|value| !value.is_empty());
    if let Some(directory) = set("ZENITH_DATA_DIR") {
        return Some(PathBuf::from(directory).join(FILE_NAME));
    }
    let base = match system {
        System::Windows => set("APPDATA").map(PathBuf::from),
        System::MacOs => set("HOME").map(|home| {
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
        }),
        // The XDG specification says a relative path in the variable is to be ignored, and
        // on the systems it is for an absolute path starts with a slash.
        System::Unix => set("XDG_DATA_HOME")
            .filter(|value| value.starts_with('/'))
            .map(PathBuf::from)
            .or_else(|| set("HOME").map(|home| PathBuf::from(home).join(".local").join("share"))),
    }?;
    Some(base.join(DIRECTORY_NAME).join(FILE_NAME))
}

/// What a session starts with: the history in `file` when there is one and it can be
/// read, with a note when something about it is worth saying; nothing at all when the
/// history is `off`.
#[must_use]
pub fn kept(off: bool, file: Option<PathBuf>) -> KeptHistory {
    if off {
        return KeptHistory::default();
    }
    let Some(file) = file else {
        return KeptHistory {
            note: Some(
                "ZENITH cannot tell where this system keeps per-user data, so the command line \
                history is kept for this session only. ZENITH_DATA_DIR names a directory for it."
                    .to_owned(),
            ),
            ..KeptHistory::default()
        };
    };
    let session_only = |why: String| KeptHistory {
        note: Some(format!(
            "The command line history in {} is left as it is, because {why}. This session \
            keeps its own for as long as it runs.",
            file.display()
        )),
        ..KeptHistory::default()
    };
    match read(&file) {
        Loaded::Lines { lines, skipped } => {
            let note = (skipped > 0).then(|| {
                format!(
                    "{skipped} {} of {} {} not a line of the command line history, and {} left \
                    out.",
                    if skipped == 1 { "line" } else { "lines" },
                    file.display(),
                    if skipped == 1 { "was" } else { "were" },
                    if skipped == 1 { "was" } else { "were" },
                )
            });
            KeptHistory {
                file: Some(file),
                lines,
                note,
            }
        }
        Loaded::Missing => KeptHistory {
            file: Some(file),
            ..KeptHistory::default()
        },
        Loaded::Foreign(why) => session_only(why),
        Loaded::Unreadable(error) => session_only(format!("it could not be read: {error}")),
    }
}

/// What reading the file found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Loaded {
    /// The lines, oldest first, and how many lines of the file were not a line of the
    /// history and were skipped.
    Lines {
        /// The lines.
        lines: Vec<String>,
        /// The lines of the file that were skipped.
        skipped: usize,
    },
    /// There is no file yet.
    Missing,
    /// The file is not a command line history this ZENITH can read, for the reason given:
    /// another format, or a newer version. It is neither read nor written, so that the
    /// program that wrote it keeps its lines.
    Foreign(String),
    /// The file could not be read, for the reason given.
    Unreadable(String),
}

/// Reads the file.
#[must_use]
pub fn read(path: &Path) -> Loaded {
    match fs::read_to_string(path) {
        Ok(text) => parse(&text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Loaded::Missing,
        Err(error) => Loaded::Unreadable(error.to_string()),
    }
}

/// Reads the text of the file.
#[must_use]
pub fn parse(text: &str) -> Loaded {
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let Some(first) = lines.next() else {
        return Loaded::Lines {
            lines: Vec::new(),
            skipped: 0,
        };
    };
    let header: Value = serde_json::from_str(first).unwrap_or(Value::Null);
    if header.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Loaded::Foreign(format!("its first line does not say {FORMAT}"));
    }
    match header.get("version").and_then(Value::as_u64) {
        Some(VERSION) => {}
        Some(version) => {
            return Loaded::Foreign(format!(
                "it is version {version} of the format, and this ZENITH reads version {VERSION}"
            ));
        }
        None => return Loaded::Foreign("its first line has no version".to_owned()),
    }
    let mut kept = Vec::new();
    let mut skipped = 0;
    for line in lines {
        match serde_json::from_str::<Value>(line) {
            Ok(Value::String(text)) if !text.trim().is_empty() => kept.push(text),
            _ => skipped += 1,
        }
    }
    Loaded::Lines {
        lines: kept,
        skipped,
    }
}

/// The text of a file that holds `lines`.
#[must_use]
pub fn render(lines: &[String]) -> String {
    let mut text = serde_json::json!({"format": FORMAT, "version": VERSION}).to_string();
    text.push('\n');
    for line in lines {
        text.push_str(&Value::String(line.clone()).to_string());
        text.push('\n');
    }
    text
}

/// Writes `lines` as the whole file, making its directory when it is missing: to a
/// temporary file beside it, flushed to the disk, then renamed over it.
///
/// # Errors
///
/// What the system said, about the directory, the temporary file or the rename. The file
/// that was there before is untouched when any of them fails.
pub fn write(path: &Path, lines: &[String]) -> io::Result<()> {
    let directory = path
        .parent()
        .ok_or_else(|| io::Error::other("the path has no directory"))?;
    fs::create_dir_all(directory)?;
    let name = path
        .file_name()
        .map_or_else(|| FILE_NAME.into(), std::ffi::OsStr::to_os_string);
    let mut temporary_name = name;
    temporary_name.push(format!(".{}.tmp", std::process::id()));
    let temporary = directory.join(temporary_name);
    let written = (|| {
        let mut file = private_file(&temporary)?;
        file.write_all(render(lines).as_bytes())?;
        file.sync_all()
    })();
    let renamed = written.and_then(|()| fs::rename(&temporary, path));
    if renamed.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    renamed
}

/// A new file that only its owner can read, where the system has such a thing: the lines
/// are what one person typed.
fn private_file(path: &Path) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    options.open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variables<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    #[test]
    fn each_system_keeps_it_where_its_convention_says() {
        let windows = path(
            System::Windows,
            variables(&[("APPDATA", r"C:\Users\lab\AppData\Roaming")]),
        )
        .unwrap();
        assert!(windows.starts_with(r"C:\Users\lab\AppData\Roaming"));
        assert!(windows.ends_with(Path::new(DIRECTORY_NAME).join(FILE_NAME)));
        assert_eq!(
            path(System::MacOs, variables(&[("HOME", "/Users/lab")])).unwrap(),
            Path::new(
                "/Users/lab/Library/Application Support/nipscern-zenith/command-history.ndjson"
            )
        );
        assert_eq!(
            path(System::Unix, variables(&[("HOME", "/home/lab")])).unwrap(),
            Path::new("/home/lab/.local/share/nipscern-zenith/command-history.ndjson")
        );
        assert_eq!(
            path(
                System::Unix,
                variables(&[("HOME", "/home/lab"), ("XDG_DATA_HOME", "/data/lab")])
            )
            .unwrap(),
            Path::new("/data/lab/nipscern-zenith/command-history.ndjson")
        );
    }

    #[test]
    fn a_relative_xdg_data_home_is_ignored_and_nothing_known_means_no_file() {
        assert_eq!(
            path(
                System::Unix,
                variables(&[("HOME", "/home/lab"), ("XDG_DATA_HOME", "relative")])
            )
            .unwrap(),
            Path::new("/home/lab/.local/share/nipscern-zenith/command-history.ndjson")
        );
        assert_eq!(path(System::Unix, variables(&[])), None);
        assert_eq!(path(System::Windows, variables(&[("APPDATA", "")])), None);
    }

    #[test]
    fn zenith_data_dir_is_used_as_it_is_on_every_system() {
        for system in [System::Windows, System::MacOs, System::Unix] {
            assert_eq!(
                path(
                    system,
                    variables(&[("ZENITH_DATA_DIR", "/tmp/z"), ("HOME", "/home/lab")])
                )
                .unwrap(),
                Path::new("/tmp/z").join(FILE_NAME)
            );
        }
    }

    #[test]
    fn the_lines_come_back_as_they_were_written() {
        let lines = vec![
            "/list".to_owned(),
            r#"/call solar.ping {"message": "a \"quote\", a tab\t and é"}"#.to_owned(),
        ];
        let text = render(&lines);
        assert!(text.starts_with(r#"{"format":"zenith-command-history","version":1}"#));
        assert_eq!(text.lines().count(), 3);
        assert_eq!(parse(&text), Loaded::Lines { lines, skipped: 0 });
    }

    #[test]
    fn a_line_that_is_not_a_line_of_the_history_is_skipped_and_counted() {
        let text = format!(
            "{}\n\"/ping\"\nnot json\n42\n\"\"\n\"/version\"\n",
            render(&[]).trim()
        );
        assert_eq!(
            parse(&text),
            Loaded::Lines {
                lines: vec!["/ping".to_owned(), "/version".to_owned()],
                skipped: 3,
            }
        );
    }

    #[test]
    fn a_file_of_another_format_or_version_is_left_alone() {
        assert!(matches!(parse("\"/ping\"\n"), Loaded::Foreign(_)));
        assert!(matches!(
            parse("{\"format\":\"something else\",\"version\":1}\n"),
            Loaded::Foreign(_)
        ));
        let newer = parse("{\"format\":\"zenith-command-history\",\"version\":2}\n\"/ping\"\n");
        assert!(
            matches!(&newer, Loaded::Foreign(why) if why.contains("version 2")),
            "{newer:?}"
        );
        assert_eq!(
            parse(""),
            Loaded::Lines {
                lines: Vec::new(),
                skipped: 0
            }
        );
    }

    #[test]
    fn writing_replaces_the_whole_file_and_leaves_nothing_beside_it() {
        let directory = std::env::temp_dir().join(format!("zenith-history-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        let file = directory.join("deeper").join(FILE_NAME);
        assert_eq!(read(&file), Loaded::Missing);
        write(&file, &["/list".to_owned(), "/ping".to_owned()]).unwrap();
        write(&file, &["/version".to_owned()]).unwrap();
        assert_eq!(
            read(&file),
            Loaded::Lines {
                lines: vec!["/version".to_owned()],
                skipped: 0
            }
        );
        let beside: Vec<_> = fs::read_dir(file.parent().unwrap()).unwrap().collect();
        assert_eq!(beside.len(), 1, "a temporary file was left behind");
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_write_that_fails_leaves_the_file_as_it_was() {
        let directory =
            std::env::temp_dir().join(format!("zenith-history-fail-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        let file = directory.join(FILE_NAME);
        write(&file, &["/list".to_owned()]).unwrap();
        // A directory where the temporary file would go makes the write fail before the
        // rename, as a crash halfway through writing would.
        let temporary = format!("{FILE_NAME}.{}.tmp", std::process::id());
        fs::create_dir_all(directory.join(&temporary)).unwrap();
        assert!(write(&file, &["/ping".to_owned()]).is_err());
        assert_eq!(
            read(&file),
            Loaded::Lines {
                lines: vec!["/list".to_owned()],
                skipped: 0
            }
        );
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_session_starts_with_the_lines_of_the_last_or_says_why_not() {
        let directory = std::env::temp_dir().join(format!("zenith-kept-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        let file = directory.join(FILE_NAME);

        assert_eq!(kept(true, Some(file.clone())), KeptHistory::default());
        let nowhere = kept(false, None);
        assert!(nowhere.file.is_none() && nowhere.note.unwrap().contains("ZENITH_DATA_DIR"));

        let first = kept(false, Some(file.clone()));
        assert_eq!(first.file.as_deref(), Some(file.as_path()));
        assert!(first.lines.is_empty() && first.note.is_none());

        fs::create_dir_all(&directory).unwrap();
        fs::write(&file, format!("{}\"/list\"\n7\n", render(&[]))).unwrap();
        let later = kept(false, Some(file.clone()));
        assert_eq!(later.lines, vec!["/list".to_owned()]);
        assert!(later.note.unwrap().starts_with("1 line of "));

        fs::write(
            &file,
            "{\"format\":\"zenith-command-history\",\"version\":9}\n",
        )
        .unwrap();
        let newer = kept(false, Some(file.clone()));
        assert!(
            newer.file.is_none(),
            "a newer file must not be written over"
        );
        assert!(newer.note.unwrap().contains("version 9"));
        let _ = fs::remove_dir_all(&directory);
    }

    #[cfg(unix)]
    #[test]
    fn only_its_owner_can_read_it() {
        use std::os::unix::fs::PermissionsExt as _;
        let directory =
            std::env::temp_dir().join(format!("zenith-history-mode-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        let file = directory.join(FILE_NAME);
        write(&file, &["/list".to_owned()]).unwrap();
        let mode = fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        let _ = fs::remove_dir_all(&directory);
    }
}
