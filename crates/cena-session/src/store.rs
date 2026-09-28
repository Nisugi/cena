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
//!
//! # A change is a read, a change and a write, whole
//!
//! [`save_text`] keeps a file whole; it does not keep two changes. Two
//! changes to one file made at once -- a login's roster entry and the
//! launcher's star, two characters' `;trigger` edits -- each read it, each
//! change their copy, and the second write loses the first change. So every
//! read-change-write of a file a player or a character changes is made
//! inside [`changing`], the whole of it (the crate review of 2026-09-28,
//! R5).

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

/// Each file a change has been made to, by path: the right to read it,
/// change it and write it back ([`changing`]). It holds nothing else.
static CHANGING: Mutex<BTreeMap<PathBuf, Arc<Mutex<()>>>> = Mutex::new(BTreeMap::new());

/// Run `change` -- a read of the file at `path`, a change, and its write --
/// with no other change to that file in this process made meanwhile. What
/// `change` checks is what it writes: validation goes inside, not after.
///
/// A lock per file, so a change to one never waits on a change to another.
/// The key is the path as given, and every caller builds its file's path
/// from the one data directory the same way. **Not reentrant**: `change`
/// must not change its own file again through here, which would wait on
/// itself.
///
/// The lock is this process's. Two Hydras sharing one data directory can
/// still lose a change between them; each write keeps the file whole either
/// way (`travel_store`'s module docs weigh a lock file, and why not).
pub fn changing<T>(path: &Path, change: impl FnOnce() -> T) -> T {
    let file = {
        let mut files = CHANGING.lock().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(files.entry(path.to_owned()).or_default())
    };
    // A panic while it was held left the file whole: the rename is the only
    // write, and it is atomic.
    let _held = file.lock().unwrap_or_else(PoisonError::into_inner);
    change()
}

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
    let text = serde_json::to_string_pretty(value)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    save_text(dir, path, &text)
}

/// Write `text` to `path` in `dir`, atomically: [`save_json`]'s write, for
/// a file kept as text a player may also edit by hand (a behavior's TOML,
/// the keybinds). The temp file is `path` with a name of this write's own
/// and `.tmp` after its extension: two writes of one file at once, in this
/// process or another, never rename each other's half-written temp into
/// place. That keeps the file whole; keeping both changes is [`changing`]'s.
///
/// # Errors
///
/// The directory cannot be created, or the file cannot be written or
/// renamed.
pub fn save_text(dir: &Path, path: &Path, text: &str) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let extension = path
        .extension()
        .map_or_else(String::new, |ext| ext.to_string_lossy().into_owned());
    // One thread writes one file at a time, so the process and the thread
    // name this write among any made at once.
    let thread = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        std::thread::current().id().hash(&mut hasher);
        hasher.finish()
    };
    let temp = path.with_extension(format!("{extension}.{}-{thread:x}.tmp", std::process::id()));
    // **Synced before the rename, and the directory after it.** The rename
    // alone survives a process crash, not a power loss: the rename's
    // directory entry can reach disk before the file's data does, leaving a
    // valid name over an empty file -- the torn write this function exists
    // to prevent, by another route. Same review-only status as the rest.
    let mut file = fs::File::create(&temp)?;
    io::Write::write_all(&mut file, text.as_bytes())?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temp, path).inspect_err(|_| {
        let _ = fs::remove_file(&temp);
    })?;
    // std cannot open a directory for syncing on Windows, where NTFS
    // journals the rename itself.
    #[cfg(unix)]
    fs::File::open(dir)?.sync_all()?;
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

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;

    /// A change to a file waits for one being made to it; a change to
    /// another file does not wait.
    #[test]
    fn a_change_waits_for_one_to_the_same_file_only() {
        let (same, other) = (
            Path::new("store-test-waits/same.toml"),
            Path::new("store-test-waits/other.toml"),
        );
        let (tell, told) = mpsc::channel();
        let (waiting, elsewhere) = changing(same, || {
            let waiting = {
                let tell = tell.clone();
                std::thread::spawn(move || changing(same, || tell.send("same")))
            };
            let elsewhere = std::thread::spawn(move || changing(other, || tell.send("other")));
            assert_eq!(told.recv_timeout(Duration::from_secs(10)), Ok("other"));
            assert!(
                told.recv_timeout(Duration::from_millis(200)).is_err(),
                "the same file's change waits"
            );
            (waiting, elsewhere)
        });
        assert_eq!(told.recv_timeout(Duration::from_secs(10)), Ok("same"));
        assert!(waiting.join().is_ok_and(|sent| sent.is_ok()));
        assert!(elsewhere.join().is_ok_and(|sent| sent.is_ok()));
    }

    /// The write leaves no temp file beside the one it wrote.
    #[test]
    fn a_write_leaves_only_its_file() -> io::Result<()> {
        let dir = std::env::temp_dir().join(format!("cena-store-temp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("keys.toml");
        save_text(&dir, &path, "a = 1\n")?;
        save_text(&dir, &path, "a = 2\n")?;
        let names: Vec<String> = fs::read_dir(&dir)?
            .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
            .collect::<io::Result<_>>()?;
        assert_eq!(names, ["keys.toml"]);
        assert_eq!(fs::read_to_string(&path)?, "a = 2\n");
        fs::remove_dir_all(&dir)
    }
}
