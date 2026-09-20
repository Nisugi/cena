//! Reading and writing a character's snapshot.
//!
//! M3 step 8, the I/O half. The shape on disk is `cena-model`'s
//! `CharacterSnapshot`; this file is the filesystem and nothing else.
//!
//! # Why the split, and why this crate
//!
//! `cena-model` is types and stateless classifiers. Putting `File::create` in
//! it would make the crate that holds the game's *meaning* also hold a
//! dependency on the machine it runs on -- and the crate graph is the
//! architecture (`CLAUDE.md`). `model_does_no_file_io` in `cena-arch-tests` is
//! what keeps that true rather than this comment.
//!
//! **`cena-platform` was the obvious home and is not a legal one.** It already
//! owns the log sink, `CENA_LOG_DIR` and the filename sanitiser this reuses --
//! but `layering.rs:74` gives it **no** intra-workspace dependencies at all: it
//! is the bottom of the graph. A store there would have to depend on
//! `cena-model` for the snapshot type, inverting the arrow.
//!
//! `cena-session` is the only crate that may hold both (`layering.rs:98-101`:
//! `cena-model`, `cena-platform`, `cena-protocol`), and it is also where the
//! lifecycle lives -- so the crate that knows a login just happened is the one
//! that can load, and the one that knows a sync finished is the one that can
//! save.
//!
//! # One file per character, named by instance and name
//!
//! ```text
//! <CENA_DATA_DIR>/<instance>_<character>.json
//! ```
//!
//! Both halves, because Lich keys its table the same way (`infomon.rb:86`) and
//! for the same reason: two characters of the same name on different instances
//! are different characters, and merging them would look like one who had
//! silently lost training.
//!
//! # Writes are atomic
//!
//! Write to a temporary file in the same directory, then rename over the
//! target. A crash or a kill mid-write leaves the previous snapshot intact
//! rather than a half-written one, and `rename` within a directory is atomic on
//! both platforms this targets.
//!
//! This matters more here than for the log sink: the store is the **primary
//! record** (author, 2026-09-19), so a truncated file is not a lost log line,
//! it is a character who has to re-run fifteen commands.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use cena_model::state::character::snapshot::CharacterSnapshot;

/// Where character stores live when [`DATA_DIR_ENV`] is unset.
///
/// Relative deliberately, and for the reason `sink/config.rs:57-61` gives about
/// the log directory: *"An absolute default is right for exactly one machine
/// and silently wrong everywhere else."*
pub const DEFAULT_DATA_DIR: &str = "data";

/// The environment variable that moves the store directory.
pub const DATA_DIR_ENV: &str = "CENA_DATA_DIR";

/// The configured data directory, or the default.
///
/// Set with, in PowerShell:
///
/// ```text
/// $env:CENA_DATA_DIR = "E:\Gemstone\data\cena_data"
/// ```
#[must_use]
pub fn data_dir() -> PathBuf {
    std::env::var_os(DATA_DIR_ENV).map_or_else(|| PathBuf::from(DEFAULT_DATA_DIR), PathBuf::from)
}

/// Make one path component out of a name from the wire.
///
/// **Filter, never substitute**, copied from `sink/writer.rs:279-287`.
///
/// That site's comment says filtering is used "so two names cannot collide
/// through substitution". Half right: it rules out substitution collisions and
/// **not** collisions generally, because filtering is itself lossy -- `A-B`
/// and `A_B` both reduce to `AB`.
///
/// It is safe here for a reason about the game rather than the code: MEASURED
/// against a live capture, character names are alphanumeric only, so two names
/// differing only in punctuation cannot both exist.
/// `filtering_collides_and_the_game_is_why_that_is_safe` records that, so a
/// widened name vocabulary finds the assumption written down.
///
/// An empty result is refused by the caller rather than defaulted, because a
/// store named after nobody is worse than no store.
fn safe_component(name: &str) -> String {
    name.chars().filter(char::is_ascii_alphanumeric).collect()
}

/// The path a character's snapshot lives at.
///
/// `None` when either half sanitises to nothing -- a name of only punctuation,
/// or an empty instance. The wire has never sent one, and inventing a fallback
/// filename would mean two such characters sharing a store.
#[must_use]
pub fn store_path(dir: &Path, instance: &str, character: &str) -> Option<PathBuf> {
    let instance = safe_component(instance);
    let character = safe_component(character);
    if instance.is_empty() || character.is_empty() {
        return None;
    }
    Some(dir.join(format!("{instance}_{character}.json")))
}

/// Why a snapshot could not be loaded.
///
/// **A missing file and a stale one are different answers**, and both are
/// ordinary rather than exceptional: the first is a character who has never
/// been synced, the second is one whose file predates a parser change. A caller
/// that treated either as an error would refuse to start.
#[derive(Debug)]
pub enum LoadError {
    /// No file at that path. The character has never been stored.
    Missing,
    /// The file exists but was written by a different [`SCHEMA_VERSION`].
    ///
    /// [`SCHEMA_VERSION`]: cena_model::state::character::snapshot::SCHEMA_VERSION
    ///
    /// **Refused, not migrated.** A snapshot is the output of a parser, so one
    /// written by a different parser may have read a column differently. The
    /// character re-syncs, which is the recovery Lich's own version check
    /// arranges (`cli.rb:73-80`).
    WrongVersion {
        /// What the file says it is.
        found: u32,
        /// What this build writes.
        expected: u32,
    },
    /// The file names a different character or instance.
    ///
    /// Reached only if a file was renamed by hand or copied between machines;
    /// the path encodes both halves, so it should be unreachable. Checked
    /// anyway, because the failure it prevents -- loading one character's
    /// skills into another -- is silent.
    WrongCharacter {
        /// Who the file describes.
        found: String,
    },
    /// The file could not be read or is not valid JSON.
    Unreadable(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => f.write_str("no stored snapshot"),
            Self::WrongVersion { found, expected } => {
                write!(f, "snapshot schema {found}, this build writes {expected}")
            }
            Self::WrongCharacter { found } => write!(f, "snapshot describes {found}"),
            Self::Unreadable(why) => write!(f, "unreadable snapshot: {why}"),
        }
    }
}

impl std::error::Error for LoadError {}

/// Load a character's snapshot, or say why not.
///
/// Every failure is recoverable by syncing, which is why they are one enum
/// rather than an `io::Error`: the caller's response to all four is the same
/// (start from an empty snapshot and re-run the commands), and only the
/// message differs.
///
/// # Errors
///
/// Returns [`LoadError`] when the file is missing, stale, describes another
/// character, or cannot be parsed.
pub fn load(dir: &Path, instance: &str, character: &str) -> Result<CharacterSnapshot, LoadError> {
    let path = store_path(dir, instance, character).ok_or_else(|| {
        LoadError::Unreadable("character or instance has no usable filename".to_owned())
    })?;
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Err(LoadError::Missing),
        Err(err) => return Err(LoadError::Unreadable(err.to_string())),
    };
    let snapshot: CharacterSnapshot =
        serde_json::from_str(&text).map_err(|err| LoadError::Unreadable(err.to_string()))?;

    if !snapshot.is_current() {
        return Err(LoadError::WrongVersion {
            found: snapshot.schema_version,
            expected: cena_model::state::character::snapshot::SCHEMA_VERSION,
        });
    }
    if !snapshot.describes(instance, character) {
        return Err(LoadError::WrongCharacter {
            found: format!("{}/{}", snapshot.instance, snapshot.character),
        });
    }
    Ok(snapshot)
}

/// Write a character's snapshot, atomically.
///
/// The directory is created if absent, so a first run on a new machine works
/// without setup.
///
/// # Errors
///
/// Returns the underlying [`io::Error`] if the directory cannot be created or
/// the file cannot be written or renamed.
pub fn save(dir: &Path, snapshot: &CharacterSnapshot) -> io::Result<PathBuf> {
    let path = store_path(dir, &snapshot.instance, &snapshot.character).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "character or instance has no usable filename",
        )
    })?;
    fs::create_dir_all(dir)?;

    // Pretty-printed, deliberately. This is a file a person opens when a
    // character's skills look wrong -- the author's "way to reset and refresh
    // it" starts with looking at what is stored. The size difference is
    // irrelevant at one file per character.
    let text = serde_json::to_string_pretty(snapshot)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;

    // Temp file in the SAME directory, so the rename is within one filesystem
    // and therefore atomic. The OS temp dir would not be: a cross-device
    // rename degrades to copy-then-delete, which is the non-atomic write this
    // exists to avoid.
    //
    // # NOT UNIT-TESTED, and this is the honest statement of that
    //
    // Mutation found that replacing these three lines with a direct
    // `fs::write(&path, text)` leaves the whole suite green, and THREE
    // attempts to close that failed:
    //
    //  1. "identical bytes after an identical save" -- true of a direct write
    //     too.
    //  2. "no leftover .tmp, right parent directory" -- likewise.
    //  3. "make the target read-only, assert the original survives" --
    //     MEASURED on Windows: `fs::write` and `fs::rename` BOTH fail with
    //     `PermissionDenied`, and both leave the original intact. The test
    //     passes under either implementation, for a reason that has nothing
    //     to do with atomicity.
    //
    // The property is "a crash between the truncate and the write leaves a
    // valid file", and a unit test cannot crash the process at a chosen
    // instant. Writing a fourth test that merely looked like coverage would
    // repeat the mistake the first three made, so it is stated here instead:
    // **this is enforced by review, not by a test.** If these lines become a
    // direct write, nothing will fail.
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, text)?;
    fs::rename(&temp, &path)?;
    Ok(path)
}
