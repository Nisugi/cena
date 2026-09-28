//! Where profiles live, and how a key resolves (`plan/12` §6a.2, binding:
//! every user-facing setting resolves through a three-level chain with
//! per-level override and explicit precedence).
//!
//! ```text
//! <CENA_DATA_DIR>/hunt/global.toml                              everyone's, on every instance
//! <CENA_DATA_DIR>/hunt/profiles/<name>.toml                     one hunt, shareable as one file
//! <CENA_DATA_DIR>/hunt/characters/<instance>_<character>.toml   one character's own overrides
//! ```
//!
//! A key resolves **character, then profile, then global, then the built-in
//! default**, and the first level that sets it wins. The levels are overlaid
//! as TOML tables before anything is typed, so the rule is one function
//! ([`overlay`]) and holds for every key alike: a table on both sides merges
//! key by key, and anything else -- a number, a string, a list -- is
//! replaced whole. A list replaces rather than appends, so a character can
//! take a routine step away as well as add one.
//!
//! The typed [`Profile`] is read once, from the merged table, so a mistyped
//! key at any level is refused by name ([`LoadError::Malformed`]).
//!
//! # Beside the other stores, in its own format
//!
//! Under the data directory, as the author put the settings beside the
//! snapshot (`cena_session::settings_store`). TOML rather than a section of
//! the JSON settings file because a profile is what a player writes and
//! shares (`plan/30` §5, Q4: *"Stores that Hydra writes stay JSON"*), and a
//! file per profile because a profile is the unit that is shared
//! (`plan/12` §6a.1: presets are one-file artifacts).
//!
//! **Presets** (`plan/30` §5) are not here yet: none is written, and a
//! profile is the preset until a second profile wants what the first has.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::profile::Profile;

/// The directory under the data directory.
pub const DIR: &str = "hunt";

/// Everyone's overrides: `<dir>/hunt/global.toml`.
#[must_use]
pub fn global_path(dir: &Path) -> PathBuf {
    dir.join(DIR).join("global.toml")
}

/// Where the profiles are: `<dir>/hunt/profiles`.
#[must_use]
pub fn profiles_dir(dir: &Path) -> PathBuf {
    dir.join(DIR).join("profiles")
}

/// One profile's file, or `None` when the name has nothing a filename can
/// hold ([`file_name`]).
#[must_use]
pub fn profile_path(dir: &Path, name: &str) -> Option<PathBuf> {
    Some(profiles_dir(dir).join(format!("{}.toml", file_name(name)?)))
}

/// One character's overrides, named as the other per-character stores are
/// (`cena_session::store::character_path`), or `None` when either half
/// sanitises to nothing.
#[must_use]
pub fn character_path(dir: &Path, instance: &str, character: &str) -> Option<PathBuf> {
    let instance = cena_session::store::safe_component(instance);
    let character = cena_session::store::safe_component(character);
    if instance.is_empty() || character.is_empty() {
        return None;
    }
    Some(
        dir.join(DIR)
            .join("characters")
            .join(format!("{instance}_{character}.toml")),
    )
}

/// A profile's name as a filename: lowercase, letters, digits, `-` and `_`
/// kept, everything else dropped. `None` when nothing is left.
#[must_use]
pub fn file_name(name: &str) -> Option<String> {
    let cleaned: String = name
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .map(|c| c.to_ascii_lowercase())
        .collect();
    (!cleaned.is_empty()).then_some(cleaned)
}

/// Lay `over` on `base`: a table on both sides merges key by key, and
/// anything else is replaced whole.
pub fn overlay(base: &mut toml::Table, over: toml::Table) {
    for (key, value) in over {
        if let toml::Value::Table(above) = value {
            if let Some(toml::Value::Table(under)) = base.get_mut(&key) {
                overlay(under, above);
                continue;
            }
            base.insert(key, toml::Value::Table(above));
        } else {
            base.insert(key, value);
        }
    }
}

/// The built-in default with each level laid over it in turn, lowest first,
/// then read as a [`Profile`].
///
/// # Errors
///
/// The merged table is not a profile: a key no table has, or a value of the
/// wrong shape.
pub fn resolve(levels: Vec<toml::Table>) -> Result<Profile, String> {
    merge(levels)?
        .try_into()
        .map_err(|e: toml::de::Error| e.to_string())
}

/// The built-in default with each level laid over it, not yet read as a
/// [`Profile`]: every setting, as `;hunt show` lists them.
///
/// # Errors
///
/// The default cannot be written as a table, which no field of it is.
pub fn merge(levels: Vec<toml::Table>) -> Result<toml::Table, String> {
    let mut merged = toml::Table::try_from(Profile::default()).map_err(|e| e.to_string())?;
    for level in levels {
        overlay(&mut merged, level);
    }
    Ok(merged)
}

/// A profile as one character will run it, and where it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    /// The resolved profile.
    pub profile: Profile,
    /// The files read, in the order they were laid: global, profile,
    /// character. A level with no file is absent.
    pub sources: Vec<PathBuf>,
}

/// Why a profile could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// The name has nothing a filename can hold.
    BadName(String),
    /// There is no such profile.
    Missing(PathBuf),
    /// A level's file exists and cannot be read.
    Unreadable {
        /// Which file.
        path: PathBuf,
        /// The I/O error.
        why: String,
    },
    /// A level's file is not TOML, or the merged result is not a profile.
    Malformed {
        /// Which file, when one file is at fault.
        path: Option<PathBuf>,
        /// What the reader said.
        why: String,
    },
    /// The profile read cleanly and is wrong ([`Profile::problems`]).
    Invalid(Vec<String>),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadName(name) => write!(f, "{name:?} is not a name a profile can have"),
            Self::Missing(path) => write!(f, "there is no profile at {}", path.display()),
            Self::Unreadable { path, why } => write!(f, "cannot read {}: {why}", path.display()),
            Self::Malformed {
                path: Some(path),
                why,
            } => {
                write!(f, "{} is not a profile: {why}", path.display())
            }
            Self::Malformed { path: None, why } => write!(f, "not a profile: {why}"),
            Self::Invalid(problems) => write!(f, "the profile is wrong: {}", problems.join("; ")),
        }
    }
}

impl std::error::Error for LoadError {}

/// Load `name` as `character` on `instance` would run it: global, then the
/// profile, then the character's own file. The profile must exist; the
/// other two need not. Without an instance or a character, the character
/// level is skipped.
///
/// # Errors
///
/// [`LoadError`]: no such profile, a file that cannot be read or is not
/// TOML, a merged result that is not a profile, or a profile that is wrong.
pub fn load(
    dir: &Path,
    instance: Option<&str>,
    character: Option<&str>,
    name: &str,
) -> Result<Loaded, LoadError> {
    let (levels, sources) = levels(dir, instance, character, name)?;
    resolved(levels, sources)
}

/// [`load`], with the profile's own file read as `text` would make it: an
/// edit, checked as the hunt will read it before it is saved, so a change
/// that would not read is never written (the crate review of 2026-09-28,
/// R5: one that was written and then put back put back over whatever
/// another change saved meanwhile).
///
/// # Errors
///
/// As [`load`]; `text` that is not TOML is [`LoadError::Malformed`], naming
/// the profile's file.
pub fn load_edited(
    dir: &Path,
    instance: Option<&str>,
    character: Option<&str>,
    name: &str,
    text: &str,
) -> Result<Loaded, LoadError> {
    let profile = profile_path(dir, name).ok_or_else(|| LoadError::BadName(name.to_owned()))?;
    let edited = text
        .parse::<toml::Table>()
        .map_err(|e| LoadError::Malformed {
            path: Some(profile.clone()),
            why: e.to_string(),
        })?;
    let (mut levels, sources) = levels(dir, instance, character, name)?;
    for (level, source) in levels.iter_mut().zip(&sources) {
        if *source == profile {
            level.clone_from(&edited);
        }
    }
    resolved(levels, sources)
}

/// The profile `levels` make, lowest first, or why they make none.
fn resolved(levels: Vec<toml::Table>, sources: Vec<PathBuf>) -> Result<Loaded, LoadError> {
    let profile = resolve(levels).map_err(|why| LoadError::Malformed { path: None, why })?;
    let problems = profile.problems();
    if !problems.is_empty() {
        return Err(LoadError::Invalid(problems));
    }
    Ok(Loaded { profile, sources })
}

/// Each level's table, lowest first, and the files they came from: what
/// [`load`] resolves, for a caller that wants the settings before they are
/// read as a profile (`;hunt show`, [`merge`]).
///
/// # Errors
///
/// No such profile, or a file that cannot be read or is not TOML.
pub fn levels(
    dir: &Path,
    instance: Option<&str>,
    character: Option<&str>,
    name: &str,
) -> Result<(Vec<toml::Table>, Vec<PathBuf>), LoadError> {
    let profile = profile_path(dir, name).ok_or_else(|| LoadError::BadName(name.to_owned()))?;
    let mut sources = Vec::new();
    let mut levels = Vec::new();
    if let Some(table) = optional(&global_path(dir), &mut sources)? {
        levels.push(table);
    }
    let table =
        optional(&profile, &mut sources)?.ok_or_else(|| LoadError::Missing(profile.clone()))?;
    levels.push(table);
    if let (Some(instance), Some(character)) = (instance, character)
        && let Some(path) = character_path(dir, instance, character)
        && let Some(table) = optional(&path, &mut sources)?
    {
        levels.push(table);
    }
    Ok((levels, sources))
}

/// Which level of the chain a setting came from (`plan/50` §7 step 6: the
/// author asked to see *"the chain in effect, each value with where it came
/// from"*).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// Hunt's own default: no file sets it.
    Default,
    /// `global.toml`, everyone's.
    Global,
    /// The profile.
    Profile,
    /// The character's own file, which wins over the rest.
    Character,
}

impl Level {
    /// Where a setting came from, as a player reads it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Default => "built in",
            Self::Global => "global",
            Self::Profile => "the profile",
            Self::Character => "the character's file",
        }
    }
}

/// [`levels`], each table with the level it is.
///
/// # Errors
///
/// As [`levels`].
pub fn named_levels(
    dir: &Path,
    instance: Option<&str>,
    character: Option<&str>,
    name: &str,
) -> Result<Vec<(Level, toml::Table)>, LoadError> {
    let (tables, sources) = levels(dir, instance, character, name)?;
    let own = instance
        .zip(character)
        .and_then(|(i, c)| character_path(dir, i, c));
    Ok(sources
        .iter()
        .zip(tables)
        .map(|(source, table)| {
            let level = if *source == global_path(dir) {
                Level::Global
            } else if own.as_ref() == Some(source) {
                Level::Character
            } else {
                Level::Profile
            };
            (level, table)
        })
        .collect())
}

/// Every setting in effect -- each leaf of [`merge`]'s table, by its dotted
/// name, in order -- with its value and the level that set it last. A table
/// is followed into; anything else, a list of tables among them, is one
/// setting, as [`overlay`] replaces it whole.
///
/// # Errors
///
/// As [`merge`].
pub fn origins(
    levels: Vec<(Level, toml::Table)>,
) -> Result<Vec<(String, toml::Value, Level)>, String> {
    let mut from = std::collections::HashMap::new();
    for (level, table) in &levels {
        for (name, _) in leaves(table) {
            from.insert(name, *level);
        }
    }
    let merged = merge(levels.into_iter().map(|(_, table)| table).collect())?;
    Ok(leaves(&merged)
        .into_iter()
        .map(|(name, value)| {
            let level = from.get(&name).copied().unwrap_or(Level::Default);
            (name, value, level)
        })
        .collect())
}

/// Every leaf of `table`, by its dotted name, in order.
fn leaves(table: &toml::Table) -> Vec<(String, toml::Value)> {
    fn walk(prefix: &str, table: &toml::Table, found: &mut Vec<(String, toml::Value)>) {
        for (key, value) in table {
            let name = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            match value {
                toml::Value::Table(inner) => walk(&name, inner, found),
                other => found.push((name, other.clone())),
            }
        }
    }
    let mut found = Vec::new();
    walk("", table, &mut found);
    found
}

/// Whether the list setting `name`, empty wherever it is set, holds
/// numbers: tried with one, as the profile would read it. An empty list
/// says nothing of what it holds, and the menu must ask for the right kind.
#[must_use]
pub fn holds_numbers(merged: &toml::Table, name: &str) -> bool {
    let mut tried = merged.clone();
    let mut parts: Vec<&str> = name.split('.').collect();
    let Some(last) = parts.pop() else {
        return false;
    };
    let mut at = &mut tried;
    for part in parts {
        match at.get_mut(part) {
            Some(toml::Value::Table(inner)) => at = inner,
            _ => return false,
        }
    }
    at.insert(
        last.to_owned(),
        toml::Value::Array(vec![toml::Value::Integer(1)]),
    );
    tried.try_into::<Profile>().is_ok()
}

/// The profiles there are, by name, in order.
///
/// # Errors
///
/// The profiles' folder is there and cannot be read.
pub fn profile_names(dir: &Path) -> io::Result<Vec<String>> {
    let mut names: Vec<String> = match fs::read_dir(profiles_dir(dir)) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|x| x == "toml"))
            .filter_map(|path| {
                path.file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
            })
            .collect(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e),
    };
    names.sort();
    Ok(names)
}

/// One level's table, if its file exists; the path is recorded when it does.
fn optional(path: &Path, sources: &mut Vec<PathBuf>) -> Result<Option<toml::Table>, LoadError> {
    match fs::read_to_string(path) {
        Ok(text) => {
            sources.push(path.to_owned());
            text.parse::<toml::Table>()
                .map(Some)
                .map_err(|e| LoadError::Malformed {
                    path: Some(path.to_owned()),
                    why: e.to_string(),
                })
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(LoadError::Unreadable {
            path: path.to_owned(),
            why: e.to_string(),
        }),
    }
}

/// Write a new file, creating its directory, and **refusing to replace one
/// that exists**: a profile a player has edited is not overwritten by an
/// import. The `AlreadyExists` error kind says which.
///
/// # Errors
///
/// The directory cannot be made, the file exists, or it cannot be written.
pub fn write_new(path: &Path, text: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    io::Write::write_all(&mut file, text.as_bytes())
}
