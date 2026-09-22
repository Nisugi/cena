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
//!   "targets":    { "bank alley": [7120] },
//!   "characters": { "GS3_Nerten": { "settings": {…}, "memories": {…},
//!                                   "targets": { "my shop": [228] }, "last_room": 228 } } }
//! ```
//!
//! - **A character's spot** is what is that character's alone: the travel
//!   profile (`settings`), what earlier crossings wrote down (`memories`),
//!   where it was last known to be (`last_room`), and **its own `targets`**
//!   -- where a saved name goes unless told otherwise.
//! - **The top-level `targets` are everyone's, on every instance**: `--global`
//!   (author, 2026-09-21: *"A person running one session or twenty five
//!   sessions are going to want the same travel shortcuts for all their
//!   guys"*). go2 keeps all of its per game (`GameSettings`); the instances
//!   share one map, so a name means the same room on each. **A character's
//!   own name is looked for first**, so one character may mean its own shop
//!   by a name everyone uses.
//!
//! Places the map already names -- `bank`, `gemshop`, `town` -- are not kept
//! here at all: they are the map's tags, and `go2 bank` finds the nearest
//! (`cena_behavior::travel::destination`). These are for what the map does
//! not name.
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
//! writing it back, all under the private `WRITING` lock. A character that holds a stale
//! copy of someone else's spot cannot write it back, because it never writes
//! any spot but its own; and a [`TravelFile`]'s `targets` are a copy to read
//! -- everyone's and its own together -- which [`save`] does not write.
//!
//! The lock is this process's. Two Hydras sharing one data directory can
//! still lose an update between them; the rename keeps the file whole
//! either way.
//!
//! # Versions
//!
//! **3** is this shape. **1 and 2** were a file per character,
//! `<instance>_<character>.travel.json`: a character with no spot yet and
//! such a file beside the snapshot is **read from it**, whole, into the
//! spot: its targets were that character's, and stay so. The old file is left where it
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
    /// go2's **custom targets** (`go2.lic:1149-1160`): everyone's, and this
    /// character's own over them, as they stood when this was loaded. Several rooms mean *the nearest*. Map ids, as go2
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
    /// Everyone's, by name.
    #[serde(default)]
    targets: Targets,
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
    /// The character's own targets, looked for before everyone's.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    targets: Targets,
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

/// Atomically, via [`crate::store::save_json`], which records why.
fn write(dir: &Path, shared: &Shared) -> io::Result<PathBuf> {
    let path = travel_path(dir);
    crate::store::save_json(dir, &path, shared)?;
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
    let spot = match key_in(&shared.characters, &name) {
        Some(key) => shared.characters.get(&key).cloned().unwrap_or_default(),
        None => read_legacy(dir, instance, character)?.map_or_else(Spot::default, Spot::from),
    };
    // Everyone's, and then the character's own over them.
    let mut targets = shared.targets;
    targets.extend(spot.targets);
    Ok(TravelFile {
        instance: instance.to_owned(),
        character: character.to_owned(),
        settings: spot.settings,
        memories: spot.memories,
        targets,
        last_room: spot.last_room,
    })
}

impl From<Legacy> for Spot {
    fn from(legacy: Legacy) -> Spot {
        Spot {
            settings: legacy.settings,
            memories: legacy.memories,
            targets: legacy.targets,
            last_room: legacy.last_room,
        }
    }
}

/// Read the file afresh, change it, and write it back, with nobody else in
/// this process doing the same in between.
fn change(dir: &Path, with: impl FnOnce(&mut Shared) -> io::Result<()>) -> io::Result<PathBuf> {
    // A panic elsewhere while holding it left the file whole: the rename is
    // the only write, and it is atomic.
    let _held = WRITING.lock().unwrap_or_else(PoisonError::into_inner);
    let mut shared = read(dir)?;
    shared.schema_version = TRAVEL_SCHEMA_VERSION;
    with(&mut shared)?;
    write(dir, &shared)
}

/// This character's spot, to change. **One that does not exist yet is moved
/// in from the character's old file first**, whoever is asking: a target
/// saved before any trip would otherwise make an empty spot, and the old
/// file's memories -- never read again once a spot exists -- would be lost.
/// The old file is asked only while there is no spot: once moved in, whatever
/// becomes of it is no reason to refuse a save.
fn spot_in<'a>(
    shared: &'a mut Shared,
    dir: &Path,
    instance: &str,
    character: &str,
) -> io::Result<&'a mut Spot> {
    let name = spot_name(instance, character).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "character or instance has no usable name",
        )
    })?;
    let key = if let Some(key) = key_in(&shared.characters, &name) {
        key
    } else {
        let moved_in =
            read_legacy(dir, instance, character)?.map_or_else(Spot::default, Spot::from);
        shared.characters.insert(name.clone(), moved_in);
        name
    };
    Ok(shared.characters.entry(key).or_default())
}

/// Write this character's settings, memories and last room -- **and no one
/// else's, and no targets** (module docs): [`TravelFile::targets`] is
/// everyone's and the character's own read together, so writing it back
/// would make everyone's the character's. [`save_target`] writes those.
///
/// # Errors
///
/// An [`io::Error`] if the file cannot be written, or **if it cannot be
/// read**: a file this build cannot trust is left exactly as it is.
pub fn save(dir: &Path, file: &TravelFile) -> io::Result<PathBuf> {
    change(dir, |shared| {
        let spot = spot_in(shared, dir, &file.instance, &file.character)?;
        spot.settings.clone_from(&file.settings);
        spot.memories.clone_from(&file.memories);
        spot.last_room = file.last_room;
        Ok(())
    })
}

/// Whose a target is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Whose<'a> {
    /// This character's alone. What `--save-target` means unless told
    /// otherwise.
    Character {
        instance: &'a str,
        character: &'a str,
    },
    /// Every character's, on every instance. `--global`.
    Everyone,
}

/// go2's `;go2 save`: from now on `name` means these rooms -- or these as
/// well, if it already means several. No rooms at all forgets the name. **A character's own name is looked for before
/// everyone's** ([`load`]), so forgetting one's own uncovers the shared one.
///
/// # Errors
///
/// As [`save`].
pub fn save_target(dir: &Path, whose: Whose<'_>, name: &str, rooms: &[u32]) -> io::Result<PathBuf> {
    let name = name.trim();
    if name.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a target needs a name",
        ));
    }
    change(dir, |shared| {
        let targets = match whose {
            Whose::Character {
                instance,
                character,
            } => &mut spot_in(shared, dir, instance, character)?.targets,
            Whose::Everyone => &mut shared.targets,
        };
        match targets.get_mut(name) {
            _ if rooms.is_empty() => {
                targets.remove(name);
            }
            // go2's rule (`go2.lic:1149-1157`): a name that means several
            // rooms gains these; one that means a single room is replaced.
            Some(several) if several.len() > 1 => {
                for room in rooms {
                    if !several.contains(room) {
                        several.push(*room);
                    }
                }
            }
            _ => {
                targets.insert(name.to_owned(), rooms.to_vec());
            }
        }
        Ok(())
    })
}
