//! Reading and writing a character's travel file (`plan/24` §5).
//!
//! ```text
//! <CENA_DATA_DIR>/<instance>_<character>.travel.json
//! ```
//!
//! # Why it is not in the character's snapshot
//!
//! The snapshot beside it (`character_store`) is **what the game said**: it is
//! the output of a parser, rewritten whole after every sync, and thrown away
//! and rebuilt when the parser changes. This file is **what Hydra did and
//! what the player chose**:
//!
//! - `settings`: the travel profile -- `ice_mode`, `use_urchins`, the name of
//!   the sack a house key is kept in.
//! - `memories`: what an earlier crossing wrote down -- *entered Duskruin
//!   from the Landing*. A character enters an event on Friday and leaves on
//!   Sunday, so these outlive the session and the login.
//!
//! The game never re-teaches either. Losing a memory strands the character at
//! an event, since the way back is priced on it. So it has a file of its own,
//! which a snapshot rewrite cannot touch (author, 2026-09-21), and **a schema
//! change here must migrate, never discard** -- the opposite of the
//! snapshot's rule, for the opposite reason.
//!
//! Same crate as the snapshot's store, for the same reason
//! (`character_store`'s module docs): this is the one crate that may both
//! know a character and touch the filesystem.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::character_store::safe_component;

/// The version this build writes. Bump it **with a migration**: see the
/// module docs for why this file is never simply refused.
///
/// **2** (2026-09-21) added `targets` and `last_room`. A version-1 file has
/// neither and loads as one with none: that *is* the migration, and [`load`]
/// stamps it 2 so the next save says what it holds. The number still had to
/// move, because a version-1 **build** would read a version-2 file, not know
/// those fields, and drop them on its next save -- losing the player's
/// targets silently. Now it refuses the file as [`TravelLoadError::Newer`].
pub const TRAVEL_SCHEMA_VERSION: u32 = 2;

/// One character's travel profile and memories.
///
/// `BTreeMap`, so the file is written in a stable order: a person opens this
/// to see why a route was refused, and a diff of it should show what changed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TravelFile {
    pub schema_version: u32,
    pub instance: String,
    pub character: String,
    #[serde(default)]
    pub settings: BTreeMap<String, String>,
    #[serde(default)]
    pub memories: BTreeMap<String, String>,
    /// go2's **custom targets**: a name the player chose, and the room or
    /// rooms it means (`go2.lic:1149-1160`). Several rooms mean *the nearest*.
    /// Map ids, as go2 keeps them.
    #[serde(default)]
    pub targets: BTreeMap<String, Vec<u32>>,
    /// The map room the character was last known to be in, to tell apart
    /// rooms that read alike when a login gives too little to (author,
    /// 2026-09-21). **A hint, never a fact**: the character may have been
    /// moved by another client since, so it only ever breaks a tie between
    /// rooms that already fit what the game shows (`cena_map::locate`'s
    /// `Origin::Still`).
    #[serde(default)]
    pub last_room: Option<u32>,
}

impl TravelFile {
    /// An empty file for this character: no settings chosen, nothing
    /// remembered. What a character who has never travelled has.
    #[must_use]
    pub fn new(instance: &str, character: &str) -> TravelFile {
        TravelFile {
            schema_version: TRAVEL_SCHEMA_VERSION,
            instance: instance.to_owned(),
            character: character.to_owned(),
            settings: BTreeMap::new(),
            memories: BTreeMap::new(),
            targets: BTreeMap::new(),
            last_room: None,
        }
    }
}

/// The path a character's travel file lives at. `None` when either half
/// sanitises to nothing, as for the snapshot.
#[must_use]
pub fn travel_path(dir: &Path, instance: &str, character: &str) -> Option<PathBuf> {
    let instance = safe_component(instance);
    let character = safe_component(character);
    if instance.is_empty() || character.is_empty() {
        return None;
    }
    Some(dir.join(format!("{instance}_{character}.travel.json")))
}

/// Why a travel file could not be used.
///
/// **A missing file is not here**: it is the ordinary state of a character
/// who has never travelled, and [`load`] answers it with an empty file.
#[derive(Debug)]
pub enum TravelLoadError {
    /// Written by a newer build than this one. Refused rather than guessed
    /// at -- and **not overwritten**: the caller must not save over it.
    Newer { found: u32 },
    /// The file names a different character or instance.
    WrongCharacter { found: String },
    /// The file could not be read, or is not valid JSON. Also not to be
    /// overwritten: a person can mend a file, and cannot mend a lost memory.
    Unreadable(String),
}

impl std::fmt::Display for TravelLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Newer { found } => write!(
                f,
                "travel file schema {found}, this build reads up to {TRAVEL_SCHEMA_VERSION}"
            ),
            Self::WrongCharacter { found } => write!(f, "travel file describes {found}"),
            Self::Unreadable(why) => write!(f, "unreadable travel file: {why}"),
        }
    }
}

impl std::error::Error for TravelLoadError {}

/// Load a character's travel file; an empty one if there is none yet.
///
/// # Errors
///
/// [`TravelLoadError`] when a file exists and cannot be trusted. **The caller
/// must then not save**, or it would replace memories it could not read with
/// none at all.
pub fn load(dir: &Path, instance: &str, character: &str) -> Result<TravelFile, TravelLoadError> {
    let path = travel_path(dir, instance, character).ok_or_else(|| {
        TravelLoadError::Unreadable("character or instance has no usable filename".to_owned())
    })?;
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Ok(TravelFile::new(instance, character));
        }
        Err(err) => return Err(TravelLoadError::Unreadable(err.to_string())),
    };
    let mut file: TravelFile =
        serde_json::from_str(&text).map_err(|err| TravelLoadError::Unreadable(err.to_string()))?;
    if file.schema_version > TRAVEL_SCHEMA_VERSION {
        return Err(TravelLoadError::Newer {
            found: file.schema_version,
        });
    }
    let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    if !same(&file.instance, instance) || !same(&file.character, character) {
        return Err(TravelLoadError::WrongCharacter {
            found: format!("{}/{}", file.instance, file.character),
        });
    }
    // Migrated by having been read: see [`TRAVEL_SCHEMA_VERSION`].
    file.schema_version = TRAVEL_SCHEMA_VERSION;
    Ok(file)
}

/// Write a character's travel file, atomically: a temp file in the same
/// directory, then a rename (`character_store::save` has the why).
///
/// # Errors
///
/// The underlying [`io::Error`] if the directory cannot be created or the
/// file cannot be written or renamed.
pub fn save(dir: &Path, file: &TravelFile) -> io::Result<PathBuf> {
    let path = travel_path(dir, &file.instance, &file.character).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "character or instance has no usable filename",
        )
    })?;
    fs::create_dir_all(dir)?;
    let text = serde_json::to_string_pretty(file)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, text)?;
    fs::rename(&temp, &path)?;
    Ok(path)
}
