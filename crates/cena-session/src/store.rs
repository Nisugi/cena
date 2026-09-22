//! What the three per-character stores share: a filename, and an atomic write.
//!
//! `character_store`, `travel_store` and `settings_store` each grew the same
//! path-building and the same temp-then-rename save. **Rule of three**
//! (`plan/05` section -1): the third one is when the shared part moves down,
//! and `settings_store.rs` was committed with a note saying so rather than
//! doing it.
//!
//! # What is here, and what deliberately is not
//!
//! Here: the filename component rule, and [`save_json`]. Those are identical
//! in all three, to the byte.
//!
//! **Not here: `load`.** All three read a file, parse JSON, check a schema
//! version and check the character matches -- but each has its own error enum
//! with its own variants and its own wording, and `travel_store` reads a file
//! that holds **every** character rather than one. A generic `load` would need
//! a trait with three implementors to say what each already says plainly.
//! That is the abstraction Rule -1 refuses, and the duplication that remains
//! is three similar shapes rather than three copies of one.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Strip everything that cannot appear in a filename, and **lowercase it**.
///
/// Character and instance names are the game's and ought to be alphanumeric,
/// but a filename is not the place to find out otherwise.
///
/// # Lowercasing is a bug fix, and the bug was platform-dependent
///
/// `CharacterSnapshot::describes` compares both halves with
/// `eq_ignore_ascii_case` (`snapshot.rs:358`), so the store's contract has
/// always been that the wire's capitalisation does not matter. The filename did
/// not honour it: `save` wrote `GameInstance_Ashryn.json` and
/// `load(dir, "gameinstance", "ASHRYN")` opened `gameinstance_ASHRYN.json`.
///
/// On Windows those are one file and the round-trip worked. On Linux they are
/// two, so `load` returned `Missing` and the character silently got a second,
/// empty store -- which per this module's own note means "a character who has
/// to re-run fifteen commands".
///
/// That is what failed `character_store::the_name_matches_regardless_of_case`
/// in CI while it passed locally. It was reported as a shared-temp-directory
/// race; it is neither shared nor a race. MEASURED: the test owns a directory
/// named after itself, and it fails single-threaded.
///
/// Lowercasing here rather than at each call site because all three stores and
/// the combat recorder build filenames from this, and a fix in one would have
/// left the others platform-dependent.
#[must_use]
pub fn safe_component(name: &str) -> String {
    name.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// `<dir>/<instance>_<character><suffix>`, or `None` if either half sanitises
/// to nothing.
///
/// `None` rather than a fallback name: a name of only punctuation has never
/// come off the wire, and inventing a filename for one would mean two such
/// characters sharing a store.
#[must_use]
pub fn character_path(
    dir: &Path,
    instance: &str,
    character: &str,
    suffix: &str,
) -> Option<PathBuf> {
    let instance = safe_component(instance);
    let character = safe_component(character);
    if instance.is_empty() || character.is_empty() {
        return None;
    }
    Some(dir.join(format!("{instance}_{character}{suffix}")))
}

/// Write a value as pretty JSON, atomically.
///
/// Pretty-printed, deliberately. These are files a person opens when a
/// character's skills look wrong, and the size difference is irrelevant at one
/// file per character.
///
/// Temp file in the SAME directory, so the rename is within one filesystem and
/// therefore atomic. The OS temp dir would not be: a cross-device rename
/// degrades to copy-then-delete, which is the non-atomic write this exists to
/// avoid.
///
/// # NOT UNIT-TESTED, and this is the honest statement of that
///
/// Carried verbatim from `character_store::save`, where it was established.
/// Mutation found that replacing the last three lines with a direct
/// `fs::write(&path, text)` leaves the whole suite green, and THREE attempts
/// to close that failed:
///
///  1. "identical bytes after an identical save" -- true of a direct write
///     too.
///  2. "no leftover .tmp, right parent directory" -- likewise.
///  3. "make the target read-only, assert the original survives" -- MEASURED
///     on Windows: `fs::write` and `fs::rename` BOTH fail with
///     `PermissionDenied`, and both leave the original intact. The test passes
///     under either implementation, for a reason that has nothing to do with
///     atomicity.
///
/// The property is "a crash between the truncate and the write leaves a valid
/// file", and a unit test cannot crash the process at a chosen instant.
/// Writing a fourth test that merely looked like coverage would repeat the
/// mistake the first three made, so it is stated here instead: **this is
/// enforced by review, not by a test.** If these lines become a direct write,
/// nothing will fail.
///
/// **Moving it here made that statement stronger, not weaker**: one place to
/// review instead of three, and three stores that cannot drift apart on it.
///
/// # Errors
///
/// The directory cannot be created, the value cannot be serialised, or the
/// file cannot be written or renamed.
pub fn save_json<T: serde::Serialize>(dir: &Path, path: &Path, value: &T) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let text = serde_json::to_string_pretty(value)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, text)?;
    fs::rename(&temp, path)?;
    Ok(())
}

/// The error a store returns when a name cannot be a filename.
///
/// One wording, because all three said the same thing in three places.
#[must_use]
pub fn unusable_name() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "character or instance has no usable filename",
    )
}
