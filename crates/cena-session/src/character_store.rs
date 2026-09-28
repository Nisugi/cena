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

/// Where character stores lived before Hydra had a folder of its own
/// (`plan/50` §6 item 10), relative to wherever it was started; still where
/// they live on a platform that names no application-data folder, and where
/// [`move_in`] looks for them to copy in once.
pub const DEFAULT_DATA_DIR: &str = "data";

/// The note [`move_in`] leaves in the old folder it copied.
pub const MOVED_NOTE: &str = "MOVED.txt";

/// The environment variable that moves the store directory.
pub const DATA_DIR_ENV: &str = "CENA_DATA_DIR";

/// How old a stored group may be before a login reports it stale.
///
/// **Thirty days, a judgement rather than a measurement.** Lich uses a
/// hardcoded cutoff DATE instead (`infomon/cli.rb:76`), bumped by hand when
/// its parser changes -- a different mechanism for a different problem: a date
/// invalidates everyone's store after a code change. Cena does that with
/// `SCHEMA_VERSION`, which `restore_into` refuses on, so this only has to
/// answer "how long before a character has probably trained".
// `from_days` would read better and is not yet stable as a `const fn`
// (rust-lang#120301), so the arithmetic is spelled out instead.
#[allow(
    clippy::duration_suboptimal_units,
    reason = "from_days is not const-stable"
)]
pub const MAX_STALE: std::time::Duration = std::time::Duration::from_secs(30 * 24 * 60 * 60);

/// The configured data directory, or Hydra's own: `data` in its folder in
/// the player's application data ([`app_dir`]), fixed, not wherever Hydra
/// was started. The author, 2026-09-27: *"yeah the data folder should be
/// fixed"* (`plan/50` §6 item 10).
///
/// Set elsewhere with, in PowerShell:
///
/// ```text
/// $env:CENA_DATA_DIR = "E:\Gemstone\data\cena_data"
/// ```
#[must_use]
pub fn data_dir() -> PathBuf {
    std::env::var_os(DATA_DIR_ENV).map_or_else(own_data_dir, PathBuf::from)
}

/// `data` in Hydra's own folder, or [`DEFAULT_DATA_DIR`] where the platform
/// names none.
fn own_data_dir() -> PathBuf {
    which(
        app_dir().map(|app| app.join("data")),
        Path::new(DEFAULT_DATA_DIR),
    )
}

/// Hydra's own data folder `own`, unless the old folder `old` has data not
/// yet copied in ([`move_in`] leaves a note when it has been): until then
/// the old one is still read, so a copy that failed never starts a player
/// with nothing.
fn which(own: Option<PathBuf>, old: &Path) -> PathBuf {
    match own {
        Some(own) if own.exists() || !old.is_dir() || old.join(MOVED_NOTE).exists() => own,
        _ => old.to_owned(),
    }
}

/// Hydra's own folder in the player's application data: `%APPDATA%\Hydra`
/// on Windows, `~/Library/Application Support/Hydra` on macOS, and
/// `$XDG_DATA_HOME/hydra` or `~/.local/share/hydra` elsewhere. Not beside
/// the program, which an update replaces. `None` when the platform's
/// variables do not say.
#[must_use]
pub fn app_dir() -> Option<PathBuf> {
    app_dir_from(|name| std::env::var_os(name))
}

/// [`app_dir`], over any set of variables, so it can be tested.
fn app_dir_from(var: impl Fn(&str) -> Option<std::ffi::OsString>) -> Option<PathBuf> {
    let set = |name: &str| {
        var(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    if cfg!(windows) {
        set("APPDATA").map(|dir| dir.join("Hydra"))
    } else if cfg!(target_os = "macos") {
        set("HOME").map(|home| {
            home.join("Library")
                .join("Application Support")
                .join("Hydra")
        })
    } else {
        set("XDG_DATA_HOME")
            .or_else(|| set("HOME").map(|home| home.join(".local").join("share")))
            .map(|dir| dir.join("hydra"))
    }
}

/// Copy the data Hydra kept where it was started into its own folder,
/// once: when [`DATA_DIR_ENV`] does not name another, its own folder has no
/// data yet, and the old [`DEFAULT_DATA_DIR`] has some. The old folder is
/// left as it was, with a note ([`MOVED_NOTE`]) saying where its data went.
/// What was done, for the caller to say; `None` when nothing was.
///
/// # Errors
///
/// The copy failed; nothing is in Hydra's folder then, and the next start
/// tries again.
pub fn settle() -> io::Result<Option<String>> {
    if std::env::var_os(DATA_DIR_ENV).is_some() {
        return Ok(None);
    }
    let Some(app) = app_dir() else {
        return Ok(None);
    };
    move_in(Path::new(DEFAULT_DATA_DIR), &app.join("data"))
}

/// [`settle`], from `old` into `new`: copied whole under a name of its own,
/// then renamed into place, so a copy cut short is never taken for the data.
///
/// # Errors
///
/// A file could not be read, written or renamed.
pub fn move_in(old: &Path, new: &Path) -> io::Result<Option<String>> {
    if new.exists() || !old.is_dir() || old.join(MOVED_NOTE).exists() {
        return Ok(None);
    }
    let copying = new.with_extension("copying");
    if copying.exists() {
        std::fs::remove_dir_all(&copying)?;
    }
    copy_dir(old, &copying)?;
    std::fs::rename(&copying, new)?;
    let old_shown = std::fs::canonicalize(old).unwrap_or_else(|_| old.to_owned());
    std::fs::write(
        old.join(MOVED_NOTE),
        format!(
            "Hydra keeps its data in {} now. This folder was copied there on {} and is no longer read; delete it once the copy is checked.\n",
            new.display(),
            cena_platform::date_dir()
        ),
    )?;
    Ok(Some(format!(
        "[data] {} copied to {}, once; the old folder is left with a note in it",
        old_shown.display(),
        new.display()
    )))
}

/// `from`, every file and folder in it, copied into `to`.
fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
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
pub(crate) use crate::store::safe_component;

/// The path a character's snapshot lives at.
///
/// `None` when either half sanitises to nothing -- a name of only punctuation,
/// or an empty instance. The wire has never sent one, and inventing a fallback
/// filename would mean two such characters sharing a store.
#[must_use]
pub fn store_path(dir: &Path, instance: &str, character: &str) -> Option<PathBuf> {
    crate::store::character_path(dir, instance, character, ".json")
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
    let path = store_path(dir, &snapshot.instance, &snapshot.character)
        .ok_or_else(crate::store::unusable_name)?;
    crate::store::save_json(dir, &path, snapshot)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hydra's own folder is in the platform's application data, and there
    /// is none without the variables that say where that is.
    #[test]
    fn the_own_folder_is_the_platforms() {
        assert_eq!(app_dir_from(|_| None), None);
        assert_eq!(
            app_dir_from(|_| Some("".into())),
            None,
            "empty says nothing"
        );
        let root = PathBuf::from("root");
        let expected = if cfg!(windows) {
            root.join("Hydra")
        } else if cfg!(target_os = "macos") {
            root.join("Library")
                .join("Application Support")
                .join("Hydra")
        } else {
            root.join("hydra")
        };
        assert_eq!(
            app_dir_from(|_| Some(root.clone().into_os_string())),
            Some(expected)
        );
    }

    /// The old folder is copied in once, whole, and left as it was with a
    /// note; until then it is the one read, and after it never again.
    #[test]
    fn the_old_folder_is_copied_in_once() -> io::Result<()> {
        let base = std::env::temp_dir().join(format!("cena-settle-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let (old, new) = (base.join("data"), base.join("app").join("data"));
        fs::create_dir_all(old.join("hunt"))?;
        fs::write(old.join("roster.json"), "[]")?;
        fs::write(old.join("hunt").join("global.toml"), "x")?;
        assert_eq!(which(Some(new.clone()), &old), old, "not copied yet");
        assert_eq!(which(None, &old), old, "no folder of its own");
        let nothing = base.join("nothing");
        assert_eq!(which(Some(new.clone()), &nothing), new, "no old folder");
        fs::create_dir_all(&new)?;
        assert_eq!(which(Some(new.clone()), &old), new, "its own, once there");
        fs::remove_dir_all(&new)?;

        assert!(move_in(&old, &new)?.is_some());
        assert_eq!(fs::read_to_string(new.join("roster.json"))?, "[]");
        assert_eq!(
            fs::read_to_string(new.join("hunt").join("global.toml"))?,
            "x"
        );
        assert!(old.join("roster.json").exists(), "left as it was");
        assert!(old.join(MOVED_NOTE).exists());
        assert!(!new.with_extension("copying").exists());
        assert_eq!(which(Some(new.clone()), &old), new);

        assert_eq!(move_in(&old, &new)?, None, "once");
        fs::remove_dir_all(&new)?;
        assert_eq!(move_in(&old, &new)?, None, "not again once noted");
        assert_eq!(which(Some(new.clone()), &old), new, "and not read again");
        let _ = fs::remove_dir_all(&base);
        Ok(())
    }
}
