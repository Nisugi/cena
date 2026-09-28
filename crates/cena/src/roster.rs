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
    /// Starred in the window's launcher, which lists it first (`plan/49`
    /// Stage C). Absent from a roster written before there were favourites.
    #[serde(default)]
    pub(crate) favourite: bool,
}

impl Entry {
    /// The entry for a login about to be made.
    pub(crate) fn of(typed: &crate::ask::Typed) -> Self {
        Self {
            character: typed.character.clone(),
            account: typed.account.clone(),
            game_code: typed.game_code.clone(),
            favourite: false,
        }
    }

    /// This character as the window's launcher shows it (`plan/49` Stage
    /// C), with `kept`, whether its account's password is: never the
    /// password.
    pub(crate) fn card(&self, kept: bool) -> cena_ui::RosterCard {
        cena_ui::RosterCard {
            character: self.character.clone(),
            account: self.account.clone(),
            game: self.game_code.clone(),
            kept,
            favourite: self.favourite,
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
    find_in(&load(dir)?, name)
}

/// [`find`], in `file` as it was read.
fn find_in(file: &File, name: &str) -> io::Result<Option<Entry>> {
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

/// Every character the roster knows.
///
/// # Errors
///
/// The file cannot be read.
pub(crate) fn all(dir: &Path) -> io::Result<Vec<Entry>> {
    Ok(load(dir)?.characters.into_values().collect())
}

/// Remember `entry`, replacing what was known of that character -- but for
/// its star, which a login does not know and must not undo.
///
/// # Errors
///
/// The file cannot be read or written.
pub(crate) fn record(dir: &Path, mut entry: Entry) -> io::Result<()> {
    changing(dir, || {
        let mut file = load(dir)?;
        let key = key(&entry.game_code, &entry.character);
        entry.favourite |= file
            .characters
            .get(&key)
            .is_some_and(|known| known.favourite);
        file.characters.insert(key, entry);
        save(dir, file)
    })
}

/// Take the character `name` means off the roster (`find`'s names): the
/// entry it had, or `None` when it had none.
///
/// # Errors
///
/// The file cannot be read or written, or the name is ambiguous.
pub(crate) fn forget(dir: &Path, name: &str) -> io::Result<Option<Entry>> {
    changing(dir, || {
        let mut file = load(dir)?;
        let Some(entry) = find_in(&file, name)? else {
            return Ok(None);
        };
        file.characters
            .remove(&key(&entry.game_code, &entry.character));
        save(dir, file)?;
        Ok(Some(entry))
    })
}

/// Star the character `name` means, or unstar it: its entry as it is now,
/// or `None` when it has none.
///
/// # Errors
///
/// The file cannot be read or written, or the name is ambiguous.
pub(crate) fn favourite(dir: &Path, name: &str, star: bool) -> io::Result<Option<Entry>> {
    changing(dir, || {
        let mut file = load(dir)?;
        let Some(mut entry) = find_in(&file, name)? else {
            return Ok(None);
        };
        entry.favourite = star;
        file.characters
            .insert(key(&entry.game_code, &entry.character), entry.clone());
        save(dir, file)?;
        Ok(Some(entry))
    })
}

/// Run `change` -- a read of the roster, a change and its write -- with no
/// other change to the file between: a login records its character while
/// the launcher stars another, and each keeps its change
/// (`cena_session::store::changing`; the crate review of 2026-09-28, R5).
fn changing<T>(dir: &Path, change: impl FnOnce() -> T) -> T {
    cena_session::store::changing(&dir.join(FILENAME), change)
}

fn save(dir: &Path, mut file: File) -> io::Result<()> {
    file.schema_version = SCHEMA_VERSION;
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
            favourite: false,
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

    /// A star outlives the next login, which does not know of it; one
    /// character of a name shared across games is starred or forgotten
    /// alone; and a name the roster has not is said to be absent.
    #[test]
    fn a_star_outlives_a_login_and_forgetting_takes_one() {
        let dir = scratch("stars");
        record(&dir, entry("GS3", "Nisugi", "ACCT1")).expect("recorded");
        record(&dir, entry("GSX", "Nisugi", "ACCT2")).expect("recorded");
        let starred = favourite(&dir, "gs3:nisugi", true).expect("starred");
        assert_eq!(starred.map(|e| e.character), Some("Nisugi".to_owned()));
        record(&dir, entry("GS3", "Nisugi", "ACCT1")).expect("logged in again");
        let starred = |name| find(&dir, name).ok().flatten().map(|e| e.favourite);
        assert_eq!(starred("GS3:Nisugi"), Some(true), "the login kept the star");
        assert_eq!(
            starred("GSX:Nisugi"),
            Some(false),
            "the other game's is apart"
        );

        favourite(&dir, "GS3:Nisugi", false).expect("unstarred");
        assert_eq!(starred("GS3:Nisugi"), Some(false));

        let gone = forget(&dir, "GSX:Nisugi").expect("forgotten");
        assert_eq!(gone.map(|e| e.account), Some("ACCT2".to_owned()));
        assert_eq!(all(&dir).map(|all| all.len()).ok(), Some(1));
        assert_eq!(forget(&dir, "GSX:Nisugi").ok(), Some(None), "not there");
        assert_eq!(favourite(&dir, "Stranger", true).ok(), Some(None));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A launcher's card says each of the entry's facts, and whether the
    /// password is kept as it was told.
    #[test]
    fn a_card_is_the_entry_and_whether_it_is_kept() {
        let mut starred = entry("GSX", "Nisugi", "ACCT2");
        starred.favourite = true;
        assert_eq!(
            starred.card(true),
            cena_ui::RosterCard {
                character: "Nisugi".to_owned(),
                account: "ACCT2".to_owned(),
                game: "GSX".to_owned(),
                kept: true,
                favourite: true,
            }
        );
        assert!(!entry("GS3", "Nerten", "ACCT1").card(false).kept);
    }

    /// Run `meanwhile` on another thread while a change to the roster is
    /// being made -- the file read, and written 200 ms later with Nisugi
    /// starred -- and wait for both.
    fn during_a_star(
        dir: &Path,
        meanwhile: impl FnOnce(std::path::PathBuf) -> io::Result<()> + Send + 'static,
    ) -> io::Result<()> {
        let other = cena_session::store::changing(&dir.join(FILENAME), || {
            let mut file = load(dir)?;
            let other = {
                let dir = dir.to_owned();
                std::thread::spawn(move || meanwhile(dir))
            };
            std::thread::sleep(std::time::Duration::from_millis(200));
            if let Some(nisugi) = file.characters.get_mut("gs3:nisugi") {
                nisugi.favourite = true;
            }
            save(dir, file)?;
            Ok::<_, io::Error>(other)
        })?;
        other
            .join()
            .map_err(|_| io::Error::other("its thread panicked"))?
    }

    /// A login's entry, a star and a forgetting, each made while another
    /// change to the roster is being made, wait for it, and every change is
    /// kept (the crate review of 2026-09-28, R5).
    #[test]
    fn a_change_made_meanwhile_waits_and_both_are_kept() -> io::Result<()> {
        let dir = scratch("meanwhile");
        record(&dir, entry("GS3", "Nisugi", "ACCT1"))?;
        record(&dir, entry("GS3", "Nerten", "ACCT2"))?;
        during_a_star(&dir, |dir| record(&dir, entry("GS3", "Dicate", "ACCT3")))?;
        assert!(find(&dir, "Dicate")?.is_some(), "the login's entry");
        during_a_star(&dir, |dir| favourite(&dir, "Nerten", true).map(drop))?;
        assert!(
            find(&dir, "Nerten")?.is_some_and(|e| e.favourite),
            "the star"
        );
        during_a_star(&dir, |dir| forget(&dir, "Dicate").map(drop))?;
        assert_eq!(find(&dir, "Dicate")?, None, "the forgetting");
        assert!(
            find(&dir, "Nisugi")?.is_some_and(|e| e.favourite),
            "and the first"
        );
        std::fs::remove_dir_all(&dir)
    }
}
