//! Which account each character is on, and which game.
//!
//! `--character Nerten` names a character; the login needs its account, and
//! the keyring holds passwords by account (`secrets.rs`). This is the bridge
//! (author's question, 2026-09-23: *"each one would need to log in
//! individually ... then you could launch with --character xxx --character
//! xxx to launch both?"*).
//!
//! **Not a secret, and holds none**: character, account and game code, in
//! `roster.json` in the data directory beside the character stores. Written
//! once a character's login reaches `Ready`, so a mistyped name never lands.
//!
//! # A character is named by game and name
//!
//! The same name on Prime and on Platinum is two characters (`plan/19`,
//! `AppInfo`). `--character Nisugi` finds the one entry by that name;
//! when two games both have one, `--character GS3:Nisugi` says which.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// The file, in the data directory.
pub(crate) const FILENAME: &str = "roster.json";

/// The schema this code writes.
const SCHEMA_VERSION: u32 = 1;

/// One character, and where it lives.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Entry {
    /// As the player spells it.
    pub(crate) character: String,
    /// The account it is on.
    pub(crate) account: String,
    /// The game code it logs in to (`GS3` is Prime).
    pub(crate) game_code: String,
}

impl Entry {
    /// The entry for a login about to be made.
    pub(crate) fn of(typed: &crate::ask::Typed) -> Self {
        Self {
            character: typed.character.clone(),
            account: typed.account.clone(),
            game_code: typed.game_code.clone(),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct File {
    schema_version: u32,
    /// Keyed `GAME:name`, lowercased.
    characters: BTreeMap<String, Entry>,
}

fn key(game_code: &str, character: &str) -> String {
    format!("{}:{}", game_code.trim(), character.trim()).to_lowercase()
}

fn load(dir: &Path) -> io::Result<File> {
    match std::fs::read_to_string(dir.join(FILENAME)) {
        Ok(text) => serde_json::from_str(&text)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{FILENAME}: {e}"))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(File::default()),
        Err(e) => Err(e),
    }
}

/// The entry `name` means: `Nisugi`, or `GS3:Nisugi` to pick a game.
///
/// # Errors
///
/// The file cannot be read, or the name is on more than one game and did
/// not say which.
pub(crate) fn find(dir: &Path, name: &str) -> io::Result<Option<Entry>> {
    let file = load(dir)?;
    if let Some((game, character)) = name.split_once(':') {
        return Ok(file.characters.get(&key(game, character)).cloned());
    }
    let matching: Vec<&Entry> = file
        .characters
        .values()
        .filter(|e| e.character.eq_ignore_ascii_case(name.trim()))
        .collect();
    match matching.as_slice() {
        [] => Ok(None),
        [one] => Ok(Some((*one).clone())),
        several => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{name} is on more than one game ({}); say which, as GAME:{name}",
                several
                    .iter()
                    .map(|e| e.game_code.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )),
    }
}

/// Remember `entry`, replacing what was known of that character.
///
/// # Errors
///
/// The file cannot be read or written.
pub(crate) fn record(dir: &Path, entry: Entry) -> io::Result<()> {
    let mut file = load(dir)?;
    file.schema_version = SCHEMA_VERSION;
    file.characters
        .insert(key(&entry.game_code, &entry.character), entry);
    std::fs::create_dir_all(dir)?;
    cena_session::store::save_json(dir, &dir.join(FILENAME), &file)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(game: &str, character: &str, account: &str) -> Entry {
        Entry {
            character: character.to_owned(),
            account: account.to_owned(),
            game_code: game.to_owned(),
        }
    }

    fn scratch(test: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("cena-roster-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_recorded_character_is_found_by_name_whatever_the_case() {
        let dir = scratch("found");
        assert_eq!(find(&dir, "Nisugi").ok(), Some(None), "no file is no entry");
        record(&dir, entry("GS3", "Nisugi", "ACCT1")).expect("recorded");
        record(&dir, entry("GS3", "Nerten", "ACCT2")).expect("recorded");
        assert_eq!(
            find(&dir, "nisugi").ok().flatten(),
            Some(entry("GS3", "Nisugi", "ACCT1"))
        );
        // Recording again replaces, rather than adding a second.
        record(&dir, entry("GS3", "Nisugi", "ACCT9")).expect("recorded");
        assert_eq!(
            find(&dir, "Nisugi").ok().flatten().map(|e| e.account),
            Some("ACCT9".to_owned())
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn one_name_on_two_games_must_say_which() {
        let dir = scratch("two-games");
        record(&dir, entry("GS3", "Nisugi", "ACCT1")).expect("recorded");
        record(&dir, entry("GSX", "Nisugi", "ACCT2")).expect("recorded");
        let Err(e) = find(&dir, "Nisugi") else {
            panic!("an ambiguous name picked a game");
        };
        assert!(e.to_string().contains("GAME:Nisugi"), "{e}");
        assert_eq!(
            find(&dir, "gsx:nisugi").ok().flatten().map(|e| e.account),
            Some("ACCT2".to_owned())
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
