//! Reading and writing the travel file (`plan/24` §5).
//!
//! ```text
//! <CENA_DATA_DIR>/travel.json
//! ```
//!
//! **One file, with a spot in it for each character** (author, 2026-09-21:
//! *"travel file should be a global file with character spots within it"*):
//!
//! ```text
//! { "schema_version": 3,
//!   "targets":    { "GS3": { "my shop": [7120] } },
//!   "characters": { "GS3_Nerten": { "settings": {…}, "memories": {…}, "last_room": 228 } } }
//! ```
//!
//! - **`targets` are shared by every character of an instance**, as go2's are
//!   (`GameSettings['custom targets']`): a name the player chose means the
//!   same room whoever is walking. Per instance, because a room's id is a
//!   fact about one game's map.
//! - **A character's spot** is what is that character's alone: the travel
//!   profile (`settings`), what earlier crossings wrote down (`memories`),
//!   and where it was last known to be (`last_room`).
//!
//! # Why it is not in the character's snapshot
//!
//! The snapshot (`character_store`) is **what the game said**: parser output,
//! rewritten whole after every sync, thrown away when the parser changes.
//! This file is **what Hydra did and what the player chose**, and the game
//! never re-teaches either. Losing a memory strands the character at an
//! event, since the way back is priced on it. So **a schema change here must
//! migrate, never discard**, and a file that cannot be read is never
//! overwritten -- the opposite of the snapshot's rule, for the opposite
//! reason.
//!
//! # Many characters, one file
//!
//! Hydra runs 3-25 characters in one process, and every one of them may
//! finish a trip at once. So **nothing here writes the file it read**. A save
//! changes one thing -- one character's spot ([`save`]), or one target
//! ([`save_target`]) -- by reading the file afresh, changing that, and
//! writing it back, all under [`WRITING`]. A character that holds a stale
//! copy of someone else's spot cannot write it back, because it never writes
//! any spot but its own; and a [`TravelFile`]'s `targets` are a copy to read,
//! which [`save`] does not write.
//!
//! The lock is this process's. Two Hydras sharing one data directory can
//! still lose an update between them; the rename keeps the file whole
//! either way.
//!
//! # Versions
//!
//! **3** is this shape. **1 and 2** were a file per character,
//! `<instance>_<character>.travel.json`: a character with no spot yet and
//! such a file beside the snapshot is **read from it** -- its settings,
//! memories and last room into the spot, its targets into the instance's
//! (a name already shared is kept as it is). The old file is left where it
//! is: it is never read again once the spot exists, and deleting what a
//! player may have edited by hand is not this module's to do.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};

use crate::character_store::safe_component;

/// The version this build writes. Bump it **with a migration**: see the
/// module docs for why this file is never simply refused.
pub const TRAVEL_SCHEMA_VERSION: u32 = 3;

/// Held across every read-change-write of the file. See the module docs.
static WRITING: Mutex<()> = Mutex::new(());

/// A character's targets by name: the room or rooms each means.
pub type Targets = BTreeMap<String, Vec<u32>>;

/// What one character sees of the travel file: its own spot, and its
/// instance's targets.
///
/// `BTreeMap`, so the file is written in a stable order: a person opens this
/// to see why a route was refused, and a diff of it should show what changed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TravelFile {
    pub instance: String,
    pub character: String,
    /// The travel profile: `ice_mode`, `use_urchins`, the name of the sack a
    /// house key is kept in.
    pub settings: BTreeMap<String, String>,
    /// What an earlier crossing wrote down -- *entered Duskruin from the
    /// Landing*. They outlive the session and the login.
    pub memories: BTreeMap<String, String>,
    /// go2's **custom targets** (`go2.lic:1149-1160`), as they stood when
    /// this was loaded. Several rooms mean *the nearest*. Map ids, as go2
    /// keeps them. **A copy to read**: [`save`] does not write it, since
    /// another character may have named a room since. [`save_target`] does.
    pub targets: Targets,
    /// The map room the character was last known to be in, to tell apart
    /// rooms that read alike when a login gives too little to. **A hint,
    /// never a fact**: the character may have been moved by another client
    /// since, so it only ever breaks a tie between rooms that already fit
    /// what the game shows (`cena_map::locate`'s `Origin::Still`).
    pub last_room: Option<u32>,
}

impl TravelFile {
    /// What a character who has never travelled has: nothing chosen, nothing
    /// remembered.
    #[must_use]
    pub fn new(instance: &str, character: &str) -> TravelFile {
        TravelFile {
            instance: instance.to_owned(),
            character: character.to_owned(),
            ..TravelFile::default()
        }
    }
}

/// The file, as it is written.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Shared {
    schema_version: u32,
    /// By instance, then by name.
    #[serde(default)]
    targets: BTreeMap<String, Targets>,
    /// By `<instance>_<character>`, as the snapshot's file is named.
    #[serde(default)]
    characters: BTreeMap<String, Spot>,
}

/// One character's spot.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Spot {
    #[serde(default)]
    settings: BTreeMap<String, String>,
    #[serde(default)]
    memories: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_room: Option<u32>,
}

/// A file per character, as versions 1 and 2 had it.
#[derive(Debug, Default, Deserialize)]
struct Legacy {
    schema_version: u32,
    instance: String,
    character: String,
    #[serde(default)]
    settings: BTreeMap<String, String>,
    #[serde(default)]
    memories: BTreeMap<String, String>,
    #[serde(default)]
    targets: Targets,
    #[serde(default)]
    last_room: Option<u32>,
}

/// Where the travel file lives.
#[must_use]
pub fn travel_path(dir: &Path) -> PathBuf {
    dir.join("travel.json")
}

/// Where a character's own file lived, in versions 1 and 2. `None` when
/// either half sanitises to nothing, as for the snapshot.
#[must_use]
pub fn legacy_path(dir: &Path, instance: &str, character: &str) -> Option<PathBuf> {
    Some(dir.join(format!("{}.travel.json", spot_name(instance, character)?)))
}

/// `<instance>_<character>`, as the snapshot's file is named.
fn spot_name(instance: &str, character: &str) -> Option<String> {
    let (instance, character) = (safe_component(instance), safe_component(character));
    (!instance.is_empty() && !character.is_empty()).then(|| format!("{instance}_{character}"))
}

/// Why the travel file could not be used.
///
/// **A missing file is not here**: it is the ordinary state of a Hydra
/// nobody has travelled with, and [`load`] answers it with an empty spot.
#[derive(Debug)]
pub enum TravelLoadError {
    /// Written by a newer build than this one. Refused rather than guessed
    /// at -- and **not overwritten**: nothing is saved over it.
    Newer { found: u32 },
    /// A character's own file, from before the shared one, names a different
    /// character or instance.
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

impl From<TravelLoadError> for io::Error {
    fn from(refused: TravelLoadError) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, refused.to_string())
    }
}

/// The file as it stands; an empty one if there is none yet.
fn read(dir: &Path) -> Result<Shared, TravelLoadError> {
    let text = match fs::read_to_string(travel_path(dir)) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Ok(Shared {
                schema_version: TRAVEL_SCHEMA_VERSION,
                ..Shared::default()
            });
        }
        Err(err) => return Err(TravelLoadError::Unreadable(err.to_string())),
    };
    let shared: Shared =
        serde_json::from_str(&text).map_err(|err| TravelLoadError::Unreadable(err.to_string()))?;
    if shared.schema_version > TRAVEL_SCHEMA_VERSION {
        return Err(TravelLoadError::Newer {
            found: shared.schema_version,
        });
    }
    Ok(shared)
}

/// Atomically: a temp file in the same directory, then a rename
/// (`character_store::save` has the why).
fn write(dir: &Path, shared: &Shared) -> io::Result<PathBuf> {
    let path = travel_path(dir);
    fs::create_dir_all(dir)?;
    let text = serde_json::to_string_pretty(shared)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, text)?;
    fs::rename(&temp, &path)?;
    Ok(path)
}

/// A character's own file from versions 1 and 2, if it has one.
fn read_legacy(
    dir: &Path,
    instance: &str,
    character: &str,
) -> Result<Option<Legacy>, TravelLoadError> {
    let Some(path) = legacy_path(dir, instance, character) else {
        return Ok(None);
    };
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(TravelLoadError::Unreadable(err.to_string())),
    };
    let legacy: Legacy =
        serde_json::from_str(&text).map_err(|err| TravelLoadError::Unreadable(err.to_string()))?;
    if legacy.schema_version >= TRAVEL_SCHEMA_VERSION {
        return Err(TravelLoadError::Newer {
            found: legacy.schema_version,
        });
    }
    let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    if !same(&legacy.instance, instance) || !same(&legacy.character, character) {
        return Err(TravelLoadError::WrongCharacter {
            found: format!("{}/{}", legacy.instance, legacy.character),
        });
    }
    Ok(Some(legacy))
}

/// The key a character's spot is under, however the file spells it: the game
/// does not change a name's case, but a person editing the file may.
fn key_in<V>(map: &BTreeMap<String, V>, wanted: &str) -> Option<String> {
    map.keys()
        .find(|key| key.eq_ignore_ascii_case(wanted))
        .cloned()
}

/// What this character sees of the travel file; an empty spot if it has none.
/// **Writes nothing**, even when it reads a character's old file: the first
/// [`save`] is what moves it in.
///
/// # Errors
///
/// [`TravelLoadError`] when a file exists and cannot be trusted. **The caller
/// must then not save**, or it would replace memories it could not read with
/// none at all -- and [`save`] refuses to, for the same reason.
pub fn load(dir: &Path, instance: &str, character: &str) -> Result<TravelFile, TravelLoadError> {
    let name = spot_name(instance, character).ok_or_else(|| {
        TravelLoadError::Unreadable("character or instance has no usable name".to_owned())
    })?;
    let shared = read(dir)?;
    let mut targets = key_in(&shared.targets, &safe_component(instance))
        .and_then(|key| shared.targets.get(&key).cloned())
        .unwrap_or_default();
    let spot = match key_in(&shared.characters, &name) {
        Some(key) => shared.characters.get(&key).cloned().unwrap_or_default(),
        None => match read_legacy(dir, instance, character)? {
            Some(legacy) => {
                for (named, rooms) in legacy.targets {
                    targets.entry(named).or_insert(rooms);
                }
                Spot {
                    settings: legacy.settings,
                    memories: legacy.memories,
                    last_room: legacy.last_room,
                }
            }
            None => Spot::default(),
        },
    };
    Ok(TravelFile {
        instance: instance.to_owned(),
        character: character.to_owned(),
        settings: spot.settings,
        memories: spot.memories,
        targets,
        last_room: spot.last_room,
    })
}

/// Read the file afresh, change it, and write it back, with nobody else in
/// this process doing the same in between.
fn change(
    dir: &Path,
    with: impl FnOnce(&mut Shared) -> io::Result<()>,
) -> io::Result<PathBuf> {
    // A panic elsewhere while holding it left the file whole: the rename is
    // the only write, and it is atomic.
    let _held = WRITING.lock().unwrap_or_else(PoisonError::into_inner);
    let mut shared = read(dir)?;
    shared.schema_version = TRAVEL_SCHEMA_VERSION;
    with(&mut shared)?;
    write(dir, &shared)
}

/// Write this character's spot -- **and no one else's, and not the targets**
/// (module docs). A character whose spot was read from its old file brings
/// that file's targets with it, the first time only, and never over a name
/// the instance already has.
///
/// # Errors
///
/// An [`io::Error`] if the file cannot be written, or **if it cannot be
/// read**: a file this build cannot trust is left exactly as it is.
pub fn save(dir: &Path, file: &TravelFile) -> io::Result<PathBuf> {
    let name = spot_name(&file.instance, &file.character).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "character or instance has no usable name",
        )
    })?;
    change(dir, |shared| {
        let key = key_in(&shared.characters, &name);
        // The old file is asked only while there is no spot: once moved in,
        // whatever becomes of it is no reason to refuse a save.
        if key.is_none()
            && let Some(legacy) = read_legacy(dir, &file.instance, &file.character)?
        {
            let instance = safe_component(&file.instance);
            let at = key_in(&shared.targets, &instance).unwrap_or(instance);
            let targets = shared.targets.entry(at).or_default();
            for (named, rooms) in legacy.targets {
                targets.entry(named).or_insert(rooms);
            }
        }
        shared.characters.insert(
            key.unwrap_or(name),
            Spot {
                settings: file.settings.clone(),
                memories: file.memories.clone(),
                last_room: file.last_room,
            },
        );
        Ok(())
    })
}

/// go2's `;go2 save`: from now on `name` means these rooms, for every
/// character of the instance. No rooms at all forgets the name.
///
/// # Errors
///
/// As [`save`].
pub fn save_target(dir: &Path, instance: &str, name: &str, rooms: &[u32]) -> io::Result<PathBuf> {
    let instance = safe_component(instance);
    if instance.is_empty() || name.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a target needs an instance and a name",
        ));
    }
    change(dir, |shared| {
        let at = key_in(&shared.targets, &instance).unwrap_or(instance);
        let targets = shared.targets.entry(at).or_default();
        if rooms.is_empty() {
            targets.remove(name.trim());
        } else {
            targets.insert(name.trim().to_owned(), rooms.to_vec());
        }
        Ok(())
    })
}
