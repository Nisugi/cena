//! A character's settings: **one file, a section per system**.
//!
//! ```text
//! <CENA_DATA_DIR>/<instance>_<character>.settings.json
//! ```
//!
//! > **AUTHOR, 2026-09-21:** *"We have a file that saves characters infomon
//! > data. We now need a file to save settings/preferences. They should
//! > probably go next to each other."* And: *"Can all these settings/
//! > preferences from different systems share a file?"*
//!
//! Beside the snapshot (`character_store`), under the same
//! `<instance>_<character>` name, and for every system at once.
//!
//! # Why it is not the snapshot, and not the travel file
//!
//! The snapshot is **what the game said**: parser output, rewritten whole,
//! thrown away when the parser changes. This is **what the player chose**, and
//! the game never re-teaches it -- so, like `travel_store`, a schema change
//! here **migrates and never discards**, and a file that cannot be read is
//! never overwritten.
//!
//! `travel_store` holds a travel profile too, beside its memories (author,
//! 2026-09-21, so that a snapshot rewrite could never clobber either). It
//! stays there until someone decides otherwise: moving a player's settings
//! between files is a migration, and this file does not need it to be useful.
//!
//! # Sections are kept as JSON until someone asks for one
//!
//! **This is what makes sharing safe.** Each system reads and writes its own
//! section by name ([`SettingsFile::section`], [`SettingsFile::set_section`])
//! and every other section rides along untouched as a `serde_json::Value`.
//! A typed struct of all sections would drop, on its next save, any section
//! written by a system this build has not heard of -- the travel file's
//! version-1-build hazard, once per system instead of once per file.
//!
//! The third store of this shape (`character_store`, `travel_store`), and the
//! rule of three was paid the next day: the filename rule and the atomic write
//! live in [`crate::store`]. **The `load`s did NOT move** -- each has its own
//! error enum with its own wording, and `travel_store` reads a file holding
//! every character, so a shared one would need a trait with three implementors
//! to say what each already says plainly.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// The version this build writes. Bump it **with a migration**.
///
/// The envelope's version, not any section's: a section that changes shape
/// versions itself, or migrates on read, without every other system's
/// settings being refused for it.
pub const SETTINGS_SCHEMA_VERSION: u32 = 1;

/// One character's settings, for every system that has any.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsFile {
    /// The envelope version it was written with; see `SETTINGS_SCHEMA_VERSION`.
    pub schema_version: u32,
    /// The game instance the character is on; checked on load, ignoring case.
    pub instance: String,
    /// The character's name; checked on load, ignoring case.
    pub character: String,
    /// By system name. `BTreeMap`, so the file is written in a stable order
    /// and a diff of it shows what changed.
    #[serde(default)]
    pub sections: BTreeMap<String, serde_json::Value>,
}

impl SettingsFile {
    /// An empty file for this character: nothing chosen, every default.
    #[must_use]
    pub fn new(instance: &str, character: &str) -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            instance: instance.to_owned(),
            character: character.to_owned(),
            sections: BTreeMap::new(),
        }
    }

    /// One system's settings, or its defaults if it has none here.
    ///
    /// # Errors
    ///
    /// The section exists and is not the shape `T` reads. **Not answered with
    /// the default**: a player who mistyped a setting should be told, not
    /// silently given the behaviour they were trying to change.
    pub fn section<T: DeserializeOwned + Default>(
        &self,
        name: &str,
    ) -> Result<T, serde_json::Error> {
        self.sections
            .get(name)
            .map_or_else(|| Ok(T::default()), |v| T::deserialize(v))
    }

    /// Replace one system's settings, leaving every other section as it was.
    ///
    /// # Errors
    ///
    /// `T` could not be represented as JSON.
    pub fn set_section<T: Serialize>(
        &mut self,
        name: &str,
        value: &T,
    ) -> Result<(), serde_json::Error> {
        self.sections
            .insert(name.to_owned(), serde_json::to_value(value)?);
        Ok(())
    }
}

/// The path a character's settings live at. `None` when either half sanitises
/// to nothing, as for the snapshot.
#[must_use]
pub fn settings_path(dir: &Path, instance: &str, character: &str) -> Option<PathBuf> {
    crate::store::character_path(dir, instance, character, ".settings.json")
}

/// Why a settings file could not be used.
///
/// **A missing file is not here**: it is the ordinary state of a character
/// who has changed nothing, and [`load`] answers it with an empty file.
#[derive(Debug)]
pub enum SettingsLoadError {
    /// Written by a newer build. Refused, and **not to be overwritten**.
    Newer {
        /// The file's `schema_version`, above what this build reads.
        found: u32,
    },
    /// The file names a different character or instance.
    WrongCharacter {
        /// Who the file does name, as `instance/character`.
        found: String,
    },
    /// Unreadable, or not valid JSON. Also not to be overwritten: a person can
    /// mend a file, and cannot mend settings that were replaced with none.
    Unreadable(String),
}

impl std::fmt::Display for SettingsLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Newer { found } => write!(
                f,
                "settings file schema {found}, this build reads up to {SETTINGS_SCHEMA_VERSION}"
            ),
            Self::WrongCharacter { found } => write!(f, "settings file describes {found}"),
            Self::Unreadable(why) => write!(f, "unreadable settings file: {why}"),
        }
    }
}

impl std::error::Error for SettingsLoadError {}

/// Load a character's settings; an empty file if there is none yet.
///
/// # Errors
///
/// [`SettingsLoadError`] when a file exists and cannot be trusted. **The
/// caller must then not save.**
pub fn load(
    dir: &Path,
    instance: &str,
    character: &str,
) -> Result<SettingsFile, SettingsLoadError> {
    let path = settings_path(dir, instance, character).ok_or_else(|| {
        SettingsLoadError::Unreadable("character or instance has no usable filename".to_owned())
    })?;
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Ok(SettingsFile::new(instance, character));
        }
        Err(err) => return Err(SettingsLoadError::Unreadable(err.to_string())),
    };
    let file: SettingsFile = serde_json::from_str(&text)
        .map_err(|err| SettingsLoadError::Unreadable(err.to_string()))?;
    if file.schema_version > SETTINGS_SCHEMA_VERSION {
        return Err(SettingsLoadError::Newer {
            found: file.schema_version,
        });
    }
    let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    if !same(&file.instance, instance) || !same(&file.character, character) {
        return Err(SettingsLoadError::WrongCharacter {
            found: format!("{}/{}", file.instance, file.character),
        });
    }
    Ok(file)
}

/// Write a character's settings, atomically: a temp file in the same
/// directory, then a rename (`character_store::save` has the why).
///
/// # Errors
///
/// The underlying [`io::Error`].
pub fn save(dir: &Path, file: &SettingsFile) -> io::Result<PathBuf> {
    let path = settings_path(dir, &file.instance, &file.character)
        .ok_or_else(crate::store::unusable_name)?;
    crate::store::save_json(dir, &path, file)?;
    Ok(path)
}
