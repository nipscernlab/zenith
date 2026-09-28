//! Finding the `solar` binary, and on Windows making the copy that runs.
//!
//! The order is the one `docs/DESIGN.md` gives in section 9.1: `--solar`, then
//! `ZENITH_SOLAR`, then the `PATH`. The `PATH` is searched here rather than left to the
//! system, so that a failure can say how many directories were searched, and so that what
//! runs is an absolute path that the report can name.
//!
//! On Windows, the binary that was found is never the one that runs (ADR 0003): it is
//! copied to a directory named by its SHA-256 under the temporary directory, because
//! SOLAR's build replaces `target\release\solar.exe` and Windows cannot replace a running
//! executable.

use std::ffi::{OsStr, OsString};
use std::fmt::{self, Write as _};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Where the path to `solar` came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// `--solar` on the command line.
    Flag,
    /// The `ZENITH_SOLAR` environment variable.
    Environment,
    /// A search of the `PATH`.
    Path,
}

impl fmt::Display for Origin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Flag => "--solar",
            Self::Environment => "ZENITH_SOLAR",
            Self::Path => "the PATH",
        })
    }
}

/// What to look for, and where. The environment is passed in rather than read here, so
/// that the tests can give each case its own.
#[derive(Debug, Clone, Default)]
pub struct Search {
    /// The value of `--solar`.
    pub flag: Option<PathBuf>,
    /// The value of `ZENITH_SOLAR`.
    pub environment: Option<OsString>,
    /// The value of `PATH`.
    pub path: Option<OsString>,
}

impl Search {
    /// The search this process would make, with `flag` from the command line.
    #[must_use]
    pub fn from_environment(flag: Option<PathBuf>) -> Self {
        Self {
            flag,
            environment: std::env::var_os("ZENITH_SOLAR").filter(|value| !value.is_empty()),
            path: std::env::var_os("PATH"),
        }
    }
}

/// The binary that was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// Its path.
    pub path: PathBuf,
    /// Where the path came from.
    pub origin: Origin,
}

/// Why `solar` was not found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocateError {
    /// A path was given, by the flag or the variable, and nothing is there.
    Missing {
        /// The path given.
        path: PathBuf,
        /// Who gave it.
        origin: Origin,
    },
    /// A path was given and it is a directory, not a program.
    Directory {
        /// The path given.
        path: PathBuf,
        /// Who gave it.
        origin: Origin,
    },
    /// No directory of the `PATH` has `solar`.
    NotOnPath {
        /// How many directories were searched.
        directories: usize,
    },
}

impl fmt::Display for LocateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { path, origin } => write!(
                formatter,
                "The path given by {origin} does not exist: {}.",
                path.display()
            ),
            Self::Directory { path, origin } => write!(
                formatter,
                "The path given by {origin} is a directory, not the solar program: {}.",
                path.display()
            ),
            Self::NotOnPath { directories } => write!(
                formatter,
                "ZENITH could not find solar on the PATH, which has {directories} {}.",
                if *directories == 1 {
                    "directory"
                } else {
                    "directories"
                }
            ),
        }
    }
}

impl std::error::Error for LocateError {}

/// The file name of the program on this system.
#[must_use]
pub fn program_name() -> &'static str {
    if cfg!(windows) { "solar.exe" } else { "solar" }
}

/// Finds `solar`.
///
/// # Errors
///
/// [`LocateError`] when a given path does not exist or is a directory, or when no
/// directory of the `PATH` has the program.
pub fn locate(search: &Search) -> Result<Found, LocateError> {
    let given = search
        .flag
        .clone()
        .map(|path| (path, Origin::Flag))
        .or_else(|| {
            search
                .environment
                .clone()
                .map(|value| (PathBuf::from(value), Origin::Environment))
        });
    if let Some((path, origin)) = given {
        return given_path(path, origin);
    }
    let directories: Vec<PathBuf> = search
        .path
        .as_deref()
        .map(|value| std::env::split_paths(value).collect())
        .unwrap_or_default();
    for directory in directories
        .iter()
        .filter(|directory| !directory.as_os_str().is_empty())
    {
        let candidate = directory.join(program_name());
        if is_program(&candidate) {
            return Ok(Found {
                path: candidate,
                origin: Origin::Path,
            });
        }
    }
    Err(LocateError::NotOnPath {
        directories: directories.len(),
    })
}

fn given_path(path: PathBuf, origin: Origin) -> Result<Found, LocateError> {
    if path.is_dir() {
        return Err(LocateError::Directory { path, origin });
    }
    if path.is_file() {
        return Ok(Found { path, origin });
    }
    // On Windows a person often writes the program without its extension.
    if cfg!(windows) && path.extension().is_none() {
        let with_extension = path.with_extension("exe");
        if with_extension.is_file() {
            return Ok(Found {
                path: with_extension,
                origin,
            });
        }
    }
    Err(LocateError::Missing { path, origin })
}

#[cfg(unix)]
fn is_program(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_program(path: &Path) -> bool {
    path.is_file()
}

/// Where the program that runs comes from: in place, or from a copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    /// Run the binary where it was found.
    InPlace,
    /// Copy it under this directory first, in a directory named after the program and
    /// then by its hash: `<root>/solar/<hash>/solar.exe`.
    CopyUnder(PathBuf),
}

impl Placement {
    /// What this system needs: a copy under the temporary directory on Windows, the
    /// binary in place elsewhere.
    #[must_use]
    pub fn for_this_system() -> Self {
        if cfg!(windows) {
            Self::CopyUnder(std::env::temp_dir().join("zenith"))
        } else {
            Self::InPlace
        }
    }
}

/// The binary that will run, and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepared {
    /// What was found.
    pub found: Found,
    /// What runs: the same path, or the copy.
    pub runs: PathBuf,
    /// The SHA-256 of the binary, in hex, when a copy was made or reused.
    pub sha256: Option<String>,
}

/// Why the copy could not be made.
#[derive(Debug)]
pub struct CopyError {
    /// The file or directory the operation was on.
    pub path: PathBuf,
    /// What the system said.
    pub error: io::Error,
    /// What ZENITH was doing.
    pub doing: &'static str,
}

impl fmt::Display for CopyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "ZENITH could not {} {}: {}.",
            self.doing,
            self.path.display(),
            self.error
        )
    }
}

impl std::error::Error for CopyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// The number of hex digits of the hash that name the directory of a copy.
const NAME_DIGITS: usize = 16;

/// Makes what was found ready to run, copying it when the placement asks for it.
///
/// # Errors
///
/// [`CopyError`] when the binary cannot be read, or the copy cannot be written. There is
/// no fallback to running the original on Windows: that is what ADR 0003 exists to
/// prevent.
pub fn prepare(found: Found, placement: &Placement) -> Result<Prepared, CopyError> {
    let Placement::CopyUnder(root) = placement else {
        return Ok(Prepared {
            runs: found.path.clone(),
            found,
            sha256: None,
        });
    };
    let hash = sha256_of(&found.path).map_err(|error| CopyError {
        path: found.path.clone(),
        error,
        doing: "read",
    })?;
    let name = found
        .path
        .file_name()
        .map_or_else(|| OsStr::new(program_name()).to_owned(), OsStr::to_owned);
    // One directory per program, so that removing the copies of other builds never
    // touches another program's copy, such as the test double's.
    let program = root.join(
        found
            .path
            .file_stem()
            .unwrap_or_else(|| OsStr::new("solar")),
    );
    let directory = program.join(&hash[..NAME_DIGITS]);
    let target = directory.join(&name);
    if !copy_is_whole(&found.path, &target) {
        fs::create_dir_all(&directory).map_err(|error| CopyError {
            path: directory.clone(),
            error,
            doing: "create the directory",
        })?;
        copy_into_place(&found.path, &target, &hash)?;
    }
    remove_other_copies(&program, &directory);
    Ok(Prepared {
        found,
        runs: target,
        sha256: Some(hash),
    })
}

/// The SHA-256 of a file, in lower case hex.
///
/// # Errors
///
/// What the system says when the file cannot be read.
pub fn sha256_of(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let mut hex = String::with_capacity(64);
    for byte in hasher.finalize() {
        let _ = write!(hex, "{byte:02x}");
    }
    Ok(hex)
}

fn copy_is_whole(source: &Path, target: &Path) -> bool {
    match (fs::metadata(source), fs::metadata(target)) {
        (Ok(source), Ok(target)) => target.is_file() && source.len() == target.len(),
        _ => false,
    }
}

/// Copies under a temporary name, checks that the copy has the hash its directory is
/// named after, and renames it into place. If the source changed between the hash and the
/// copy, which happens when SOLAR's build is writing it at that moment, the copy is thrown
/// away and the error says so, rather than leaving a file whose name lies about it.
fn copy_into_place(source: &Path, target: &Path, hash: &str) -> Result<(), CopyError> {
    let temporary = target.with_extension(format!("{}.partial", std::process::id()));
    let failed = |path: &Path, doing: &'static str| {
        let path = path.to_owned();
        move |error| CopyError { path, error, doing }
    };
    fs::copy(source, &temporary).map_err(failed(&temporary, "copy solar to"))?;
    let copied = sha256_of(&temporary).map_err(failed(&temporary, "read back the copy"))?;
    if copied != hash {
        let _ = fs::remove_file(&temporary);
        return Err(CopyError {
            path: source.to_owned(),
            error: io::Error::other("the file changed while it was being copied; try again"),
            doing: "copy",
        });
    }
    match fs::rename(&temporary, target) {
        Ok(()) => Ok(()),
        // Another ZENITH finished the same copy first, which is as good.
        Err(_) if copy_is_whole(source, target) => {
            let _ = fs::remove_file(&temporary);
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            Err(CopyError {
                path: target.to_owned(),
                error,
                doing: "put the copy in place at",
            })
        }
    }
}

/// Removes the copies of other builds. A copy that is running cannot be removed on
/// Windows, and that failure is what keeps a second ZENITH safe, so failures are ignored.
fn remove_other_copies(root: &Path, keep: &Path) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let named_by_hash = path
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| {
                name.len() == NAME_DIGITS && name.bytes().all(|byte| byte.is_ascii_hexdigit())
            });
        if named_by_hash && path != keep && path.is_dir() {
            let _ = fs::remove_dir_all(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "zenith-test-{name}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn fake_program(directory: &Path, content: &[u8]) -> PathBuf {
        let path = directory.join(program_name());
        fs::write(&path, content).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    #[test]
    fn the_flag_wins_over_the_variable_and_the_variable_over_the_path() {
        let scratch = Scratch::new("order");
        let program = fake_program(&scratch.0, b"x");
        let search = Search {
            flag: Some(program.clone()),
            environment: Some(OsString::from("elsewhere")),
            path: Some(scratch.0.clone().into_os_string()),
        };
        assert_eq!(locate(&search).unwrap().origin, Origin::Flag);
        let search = Search {
            flag: None,
            environment: Some(program.clone().into_os_string()),
            ..search
        };
        assert_eq!(locate(&search).unwrap().origin, Origin::Environment);
        let search = Search {
            environment: None,
            ..search
        };
        let found = locate(&search).unwrap();
        assert_eq!((found.path, found.origin), (program, Origin::Path));
    }

    #[test]
    fn a_given_path_that_does_not_exist_is_reported_with_who_gave_it() {
        let search = Search {
            flag: Some(PathBuf::from("no/such/solar")),
            ..Search::default()
        };
        let error = locate(&search).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "The path given by --solar does not exist: {}.",
                Path::new("no/such/solar").display()
            )
        );
    }

    #[test]
    fn a_directory_is_not_a_program() {
        let scratch = Scratch::new("directory");
        let search = Search {
            environment: Some(scratch.0.clone().into_os_string()),
            ..Search::default()
        };
        assert!(matches!(
            locate(&search),
            Err(LocateError::Directory {
                origin: Origin::Environment,
                ..
            })
        ));
    }

    #[test]
    fn a_search_that_finds_nothing_says_how_many_directories_it_searched() {
        let first = Scratch::new("empty-a");
        let second = Scratch::new("empty-b");
        let path = std::env::join_paths([&first.0, &second.0]).unwrap();
        let search = Search {
            path: Some(path),
            ..Search::default()
        };
        let error = locate(&search).unwrap_err();
        assert_eq!(error, LocateError::NotOnPath { directories: 2 });
        assert!(error.to_string().ends_with("which has 2 directories."));
    }

    #[test]
    fn in_place_runs_what_was_found() {
        let scratch = Scratch::new("in-place");
        let program = fake_program(&scratch.0, b"x");
        let found = Found {
            path: program.clone(),
            origin: Origin::Flag,
        };
        let prepared = prepare(found, &Placement::InPlace).unwrap();
        assert_eq!(prepared.runs, program);
        assert_eq!(prepared.sha256, None);
    }

    #[test]
    fn a_copy_is_named_by_the_hash_and_reused_while_the_binary_is_the_same() {
        let scratch = Scratch::new("copy");
        let program = fake_program(&scratch.0, b"one build");
        let root = scratch.0.join("cache");
        let placement = Placement::CopyUnder(root.clone());
        let found = Found {
            path: program.clone(),
            origin: Origin::Path,
        };
        let first = prepare(found.clone(), &placement).unwrap();
        let hash = first.sha256.clone().unwrap();
        assert_eq!(hash, sha256_of(&program).unwrap());
        assert_eq!(
            first.runs,
            root.join("solar").join(&hash[..16]).join(program_name())
        );
        assert_eq!(fs::read(&first.runs).unwrap(), b"one build");
        let second = prepare(found, &placement).unwrap();
        assert_eq!(second.runs, first.runs);
    }

    #[test]
    fn a_new_build_gets_a_new_copy_and_the_old_one_is_removed() {
        let scratch = Scratch::new("rebuild");
        let program = fake_program(&scratch.0, b"old build");
        let root = scratch.0.join("cache");
        let placement = Placement::CopyUnder(root.clone());
        let found = Found {
            path: program.clone(),
            origin: Origin::Path,
        };
        let old = prepare(found.clone(), &placement).unwrap();
        fs::write(&program, b"new build").unwrap();
        let new = prepare(found, &placement).unwrap();
        assert_ne!(old.runs, new.runs);
        assert_eq!(fs::read(&new.runs).unwrap(), b"new build");
        assert!(!old.runs.exists());
    }

    #[test]
    fn a_copy_of_another_program_is_never_removed() {
        let scratch = Scratch::new("programs");
        let solar = fake_program(&scratch.0, b"solar");
        let other_directory = scratch.0.join("other");
        fs::create_dir_all(&other_directory).unwrap();
        let other = other_directory.join("double.bin");
        fs::write(&other, b"double").unwrap();
        let placement = Placement::CopyUnder(scratch.0.join("cache"));
        let copy_of_solar = prepare(
            Found {
                path: solar,
                origin: Origin::Path,
            },
            &placement,
        )
        .unwrap();
        prepare(
            Found {
                path: other,
                origin: Origin::Flag,
            },
            &placement,
        )
        .unwrap();
        assert!(copy_of_solar.runs.is_file());
    }

    #[test]
    fn a_directory_in_the_cache_not_named_by_a_hash_is_left_alone() {
        let scratch = Scratch::new("foreign");
        let program = fake_program(&scratch.0, b"x");
        let root = scratch.0.join("cache");
        fs::create_dir_all(root.join("solar").join("notes")).unwrap();
        let found = Found {
            path: program,
            origin: Origin::Path,
        };
        prepare(found, &Placement::CopyUnder(root.clone())).unwrap();
        assert!(root.join("solar").join("notes").is_dir());
    }

    #[test]
    fn the_hash_is_the_sha256_of_the_content() {
        let scratch = Scratch::new("hash");
        let path = scratch.0.join("abc");
        fs::write(&path, b"abc").unwrap();
        assert_eq!(
            sha256_of(&path).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn a_program_without_its_extension_is_completed_with_exe_on_windows_only() {
        let scratch = Scratch::new("extension");
        fs::write(scratch.0.join("prog.exe"), b"x").unwrap();
        let without = given_path(scratch.0.join("prog"), Origin::Flag);
        if cfg!(windows) {
            assert_eq!(without.unwrap().path, scratch.0.join("prog.exe"));
        } else {
            assert!(matches!(without, Err(LocateError::Missing { .. })));
        }
        // A name that has an extension already is never given another.
        let other = given_path(scratch.0.join("prog.bin"), Origin::Flag);
        assert!(matches!(other, Err(LocateError::Missing { .. })));
    }

    #[cfg(unix)]
    #[test]
    fn on_unix_a_file_on_the_path_is_a_program_only_when_it_may_be_run() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = Scratch::new("mode");
        let program = fake_program(&scratch.0, b"x");
        let search = Search {
            flag: None,
            environment: None,
            path: Some(scratch.0.clone().into_os_string()),
        };
        assert!(locate(&search).is_ok());
        fs::set_permissions(&program, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(
            locate(&search),
            Err(LocateError::NotOnPath { directories: 1 })
        ));
        for mode in [0o744, 0o654, 0o645] {
            fs::set_permissions(&program, fs::Permissions::from_mode(mode)).unwrap();
            assert!(locate(&search).is_ok(), "{mode:o}");
        }
        // A directory that may be entered is not a program either.
        fs::remove_file(&program).unwrap();
        fs::create_dir_all(&program).unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(
            locate(&search),
            Err(LocateError::NotOnPath { .. })
        ));
    }

    #[test]
    fn a_copy_that_failed_says_what_zenith_was_doing_and_why() {
        let error = CopyError {
            path: PathBuf::from("cache/solar"),
            error: io::Error::other("the disk is full"),
            doing: "create the directory",
        };
        assert_eq!(
            error.to_string(),
            "ZENITH could not create the directory cache/solar: the disk is full."
        );
        assert_eq!(
            std::error::Error::source(&error)
                .map(ToString::to_string)
                .as_deref(),
            Some("the disk is full")
        );
    }

    #[test]
    fn a_whole_copy_is_used_as_it_is_and_a_cut_one_is_copied_again() {
        let scratch = Scratch::new("whole");
        let program = fake_program(&scratch.0, b"the build");
        let placement = Placement::CopyUnder(scratch.0.join("cache"));
        let found = Found {
            path: program,
            origin: Origin::Path,
        };
        let first = prepare(found.clone(), &placement).unwrap();
        // The same length: the copy is taken to be whole and is not written again.
        fs::write(&first.runs, b"THE BUILD").unwrap();
        let again = prepare(found.clone(), &placement).unwrap();
        assert_eq!(fs::read(&again.runs).unwrap(), b"THE BUILD");
        // Another length: the copy was cut, and is made again.
        fs::write(&first.runs, b"the bu").unwrap();
        let mended = prepare(found, &placement).unwrap();
        assert_eq!(fs::read(&mended.runs).unwrap(), b"the build");
    }

    #[test]
    fn a_copy_that_cannot_be_put_in_place_is_an_error_and_not_a_program() {
        let scratch = Scratch::new("blocked");
        let program = fake_program(&scratch.0, b"a build");
        let root = scratch.0.join("cache");
        let hash = sha256_of(&program).unwrap();
        // A directory where the copy should go: the copy cannot be renamed onto it.
        let target = root.join("solar").join(&hash[..16]).join(program_name());
        fs::create_dir_all(target.join("inside")).unwrap();
        let prepared = prepare(
            Found {
                path: program,
                origin: Origin::Path,
            },
            &Placement::CopyUnder(root),
        );
        let error = prepared.unwrap_err();
        assert_eq!(error.doing, "put the copy in place at");
    }

    #[test]
    fn only_a_directory_named_by_sixteen_hex_digits_is_a_copy_to_remove() {
        let scratch = Scratch::new("names");
        let program = fake_program(&scratch.0, b"x");
        let root = scratch.0.join("cache");
        let solar = root.join("solar");
        for name in ["cafe", "not-hex-sixteen!", "0123456789abcdef0"] {
            fs::create_dir_all(solar.join(name)).unwrap();
        }
        fs::create_dir_all(solar.join("0123456789abcdef")).unwrap();
        let prepared = prepare(
            Found {
                path: program,
                origin: Origin::Path,
            },
            &Placement::CopyUnder(root),
        )
        .unwrap();
        assert!(prepared.runs.is_file());
        for name in ["cafe", "not-hex-sixteen!", "0123456789abcdef0"] {
            assert!(solar.join(name).is_dir(), "{name} was removed");
        }
        assert!(!solar.join("0123456789abcdef").exists());
    }
}
