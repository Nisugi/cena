//! The keys files' writer (`plan/50` §7 step 2), until then a hand edit
//! only: every character's keybinds file, and a character's own (`plan/52`
//! step 2). The Keys page's changes are made one at a time to a file's text,
//! a line at a time, so what a player wrote there by hand -- a comment, the
//! order -- is kept. The result is read back before it is saved: a file
//! this cannot change in place, a `[keys]` written as an inline table say,
//! is refused rather than mangled.

use std::path::Path;

#[cfg(test)]
use super::Macro;
use super::file::{self, KeyFile, Whose};
use super::page::{KeyChange, Place};
use super::{Chord, Keybinds};

/// Make `change` to the keys file at `path`, `whose` it is, in `data`, the
/// data folder, and save it; `keys` are the keys in effect, for what lies
/// beneath the file's own lines. What was done, in words for the player,
/// after `said`, whose keys they are.
///
/// # Errors
///
/// Why nothing was changed.
pub(crate) fn apply(
    data: &Path,
    (path, whose): (&Path, Whose),
    change: &KeyChange,
    (keys, said): (&Keybinds, &str),
) -> Result<String, String> {
    let name = path
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let old = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(why) => return Err(format!("{said}: nothing was changed: {name}: {why}")),
    };
    if let Err(why) = toml::from_str::<toml::Table>(&old) {
        return Err(format!(
            "{said}: nothing was changed while {name} does not read: {why}"
        ));
    }
    let (text, done) =
        changed(&old, whose, change, keys).map_err(|why| format!("{said}: {why}"))?;
    cena_session::store::save_text(data, path, &text)
        .map_err(|why| format!("{said}: {done}, but {name} was not saved: {why}"))?;
    Ok(format!("{said}: {done}."))
}

/// Move a key between the two files: `bound` made to the file at `to`,
/// `taken` to the one at `from`, both read and both changes made before
/// either is saved. Should the second save fail, the first is put back as
/// it was, so the key is in the file it started in, not in both with an
/// error naming one of them (the review of 2026-09-29). Each save is
/// atomic on its own (`cena_session::store::save_text`); this is the pair.
///
/// # Errors
///
/// Why nothing was changed, or, when the first file could not be put back
/// either, that the key is now in both.
pub(crate) fn apply_pair(
    data: &Path,
    (to, to_whose, bound): (&Path, Whose, &KeyChange),
    (from, from_whose, taken): (&Path, Whose, &KeyChange),
    (keys, said): (&Keybinds, &str),
) -> Result<String, String> {
    let read = |path: &Path| -> Result<String, String> {
        let name = path
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        let old = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(why) => return Err(format!("{said}: nothing was changed: {name}: {why}")),
        };
        if let Err(why) = toml::from_str::<toml::Table>(&old) {
            return Err(format!(
                "{said}: nothing was changed while {name} does not read: {why}"
            ));
        }
        Ok(old)
    };
    let (old_to, old_from) = (read(to)?, read(from)?);
    let (new_to, done) =
        changed(&old_to, to_whose, bound, keys).map_err(|why| format!("{said}: {why}"))?;
    let (new_from, _) =
        changed(&old_from, from_whose, taken, keys).map_err(|why| format!("{said}: {why}"))?;
    cena_session::store::save_text(data, to, &new_to)
        .map_err(|why| format!("{said}: nothing was changed: {why}"))?;
    if let Err(why) = cena_session::store::save_text(data, from, &new_from) {
        return Err(match cena_session::store::save_text(data, to, &old_to) {
            Ok(()) => format!("{said}: nothing was changed: {why}"),
            Err(again) => format!(
                "{said}: {done}, but it could not be taken out of the other file ({why}), \
                 nor put back ({again}): it is in both."
            ),
        });
    }
    Ok(done)
}

/// Make every one of `changes` to the keys file at `path`, `whose` it is,
/// in `data`, one after another in its text, and save it once: a Wrayth key
/// set imported (step 9).
///
/// # Errors
///
/// Why nothing was changed: the file does not read, a change could not be
/// made, or the file was not saved.
pub(crate) fn apply_all(
    data: &Path,
    (path, whose): (&Path, Whose),
    changes: &[KeyChange],
    keys: &Keybinds,
) -> Result<(), String> {
    let mut text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(why) => return Err(why.to_string()),
    };
    if let Err(why) = toml::from_str::<toml::Table>(&text) {
        return Err(format!("it does not read: {why}"));
    }
    for change in changes {
        text = changed(&text, whose, change, keys)?.0;
    }
    cena_session::store::save_text(data, path, &text).map_err(|why| why.to_string())
}

/// `old`, a keys file of `whose`, with `change` made, read back to be sure
/// it holds, and what was done.
fn changed(
    old: &str,
    whose: Whose,
    change: &KeyChange,
    keys: &Keybinds,
) -> Result<(String, String), String> {
    let chord = |written: &str| Chord::parse(written);
    let (text, done) = match change {
        KeyChange::Bind {
            key,
            does,
            was,
            place,
        } => {
            let to = chord(key)?;
            if let Some(why) = to.refused() {
                return Err(why);
            }
            does.check().map_err(|why| format!("{key}: {why}."))?;
            let text = match was.as_deref().map(chord).transpose()? {
                Some(was) => vacate(old, whose, place.set, &was, keys),
                None => old.to_owned(),
            };
            (
                bind(&text, place.set, &to, Some(&does.written())),
                format!("{} {}{}", to.written(), does.said(), in_set(place.set)),
            )
        }
        KeyChange::Unbind { key, place } => {
            let gone = chord(key)?;
            // A set from 1 to 9 falls to set 0 for a key it leaves out, so
            // a key taken out of one is not a key that does nothing: said
            // so, where this said `does nothing in set 3` of a key that
            // went on doing set 0's (the review of 2026-09-29).
            let done = match place.set {
                0 => format!("{} does nothing", gone.written()),
                set => format!(
                    "{} is taken out of set {set}, and does what it does in set 0",
                    gone.written()
                ),
            };
            (vacate(old, whose, place.set, &gone, keys), done)
        }
        KeyChange::Restore { key, place } => {
            let back = chord(key)?;
            let done = match (place.set, keys.beneath(whose, &back)) {
                (0, Some(does)) => format!("{} {} again", back.written(), does.said()),
                (0, None) => format!("{} does nothing", back.written()),
                (set, _) => format!("{} is taken out of set {set}", back.written()),
            };
            (bind(old, place.set, &back, None), done)
        }
        KeyChange::NumpadAlways(always) => (
            top(old, "numpad", always.then_some("\"always\"")),
            if *always {
                "the numpad sends its keys with NumLock on too"
            } else {
                "the numpad sends its keys with NumLock off"
            }
            .to_owned(),
        ),
        KeyChange::Choose(set) => (
            top(old, "set", (*set != 0).then(|| set.to_string()).as_deref()),
            if *set == 0 {
                "set 0 alone is in use".to_owned()
            } else {
                format!("macro set {set} is in use, over set 0")
            },
        ),
        KeyChange::Share { .. } => {
            return Err("a key is shared by two changes, one to each file".to_owned());
        }
    };
    let (read, _) = KeyFile::read(&text, whose);
    let entry = |place: &Place, key: &str| {
        chord(key)
            .ok()
            .and_then(|key| read.sets[usize::from(place.set)].get(&key).cloned())
    };
    let holds = toml::from_str::<toml::Table>(&text).is_ok()
        && match change {
            KeyChange::Bind {
                key,
                does,
                was,
                place,
            } => {
                entry(place, key) == Some(Some(does.clone()))
                    && was
                        .as_deref()
                        .is_none_or(|was| entry(place, was).flatten().is_none())
            }
            KeyChange::Unbind { key, place } => entry(place, key).flatten().is_none(),
            KeyChange::Restore { key, place } => entry(place, key).is_none(),
            KeyChange::NumpadAlways(always) => read.numpad_always == *always,
            KeyChange::Choose(set) => read.chosen == *set,
            KeyChange::Share { .. } => false,
        };
    if !holds {
        return Err(
            "nothing was changed: the file is written in a way this cannot change; edit it by hand."
                .to_owned(),
        );
    }
    Ok((text, done))
}

/// ` in set 3` after what a key does there, or nothing for set 0.
fn in_set(set: u8) -> String {
    if set == 0 {
        String::new()
    } else {
        format!(" in set {set}")
    }
}

/// `text`, a file of `whose`, with `chord` doing nothing in set `set`:
/// written `""` in set 0 where something beneath the file binds it, so that
/// goes too, and taken out of the file elsewhere. A set from 1 to 9 falls to
/// set 0 for a key it leaves out.
fn vacate(text: &str, whose: Whose, set: u8, chord: &Chord, keys: &Keybinds) -> String {
    if set == 0 && keys.beneath(whose, chord).is_some() {
        bind(text, set, chord, Some("\"\""))
    } else {
        bind(text, set, chord, None)
    }
}

/// `text` with `chord` bound in set `set` to `value`, a TOML value as the
/// file writes it, or to nothing of the file's: each line that binds it in
/// that set's table changed or taken out, or one added at the end of the
/// table, which is made when there is none.
fn bind(text: &str, set: u8, chord: &Chord, value: Option<&str>) -> String {
    let name = file::table(set);
    let mut entry = value.map(|value| format!("{} = {value}", quoted(&chord.written())));
    let mut lines: Vec<String> = Vec::new();
    let mut in_set = false;
    let mut end_of_set = None;
    for raw in text.lines() {
        if let Some(opened) = table(raw) {
            in_set = opened == name;
            lines.push(raw.to_owned());
            if in_set {
                end_of_set = Some(lines.len());
            }
            continue;
        }
        let key = in_set.then(|| entry_key(raw)).flatten();
        if let Some(key) = key {
            if Chord::parse(&key).is_ok_and(|bound| bound == *chord) {
                lines.extend(entry.take());
            } else {
                lines.push(raw.to_owned());
            }
            end_of_set = Some(lines.len());
            continue;
        }
        lines.push(raw.to_owned());
    }
    if let Some(entry) = entry {
        if let Some(at) = end_of_set {
            lines.insert(at, entry);
        } else {
            if lines.last().is_some_and(|last| !last.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push(format!("[{name}]"));
            lines.push(entry);
        }
    }
    joined(&lines)
}

/// `text` with `name = value` above its tables, a blank line after it, or
/// without it and its blank line: a setting's default is written by leaving
/// it out. Written and taken out again, the text is as it was.
fn top(text: &str, name: &str, value: Option<&str>) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut first_table = None;
    let mut taken = false;
    for raw in text.lines() {
        if first_table.is_none() {
            if std::mem::take(&mut taken) && raw.trim().is_empty() {
                continue;
            }
            if table(raw).is_some() {
                first_table = Some(lines.len());
            } else if entry_key(raw).as_deref() == Some(name) {
                taken = true;
                continue;
            }
        }
        lines.push(raw.to_owned());
    }
    if let Some(value) = value {
        let at = first_table.unwrap_or(lines.len());
        if at < lines.len() {
            lines.insert(at, String::new());
        }
        lines.insert(at, format!("{name} = {value}"));
    }
    joined(&lines)
}

/// Whether `raw` opens a table, and if it does, its name: empty for one
/// that is no plain `[name]`.
fn table(raw: &str) -> Option<String> {
    let trimmed = raw.trim_start();
    if !trimmed.starts_with('[') {
        return None;
    }
    let plain = (!trimmed.starts_with("[["))
        .then(|| toml::from_str::<toml::Table>(trimmed).ok())
        .flatten()
        .filter(|table| table.len() == 1)
        .and_then(|table| {
            let (name, value) = table.into_iter().next()?;
            value
                .as_table()
                .is_some_and(toml::Table::is_empty)
                .then_some(name)
        });
    Some(plain.unwrap_or_default())
}

/// The key a `key = value` line sets, when the line is one whole.
fn entry_key(raw: &str) -> Option<String> {
    let table = toml::from_str::<toml::Table>(raw).ok()?;
    let mut keys = table.keys();
    let key = keys.next()?.clone();
    keys.next().is_none().then_some(key)
}

/// `text` as a TOML string, in quotes, escaped.
fn quoted(text: &str) -> String {
    toml::Value::String(text.to_owned()).to_string()
}

/// `lines` as a file's text, a line end after each.
fn joined(lines: &[String]) -> String {
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::file::KeyFile;

    const FILE_BY_HAND: &str = "\
# My keys, by hand.
numpad = \"numlock\"

[keys]
# Movement.
Numpad8 = \"north\"
\"ctrl+F1\" = \"look\"   # the room

# The end.
";

    /// Set 0 of every character's file.
    const EVERY: Place = Place {
        set: 0,
        every: true,
    };

    fn changed(old: &str, change: &KeyChange) -> Result<(String, String), String> {
        super::changed(old, Whose::Every, change, &Keybinds::default())
    }

    fn made(change: &KeyChange) -> Result<(String, String), String> {
        changed(FILE_BY_HAND, change)
    }

    fn bound(key: &str, line: &str, was: Option<&str>) -> KeyChange {
        KeyChange::Bind {
            key: key.to_owned(),
            does: Macro::Send(line.to_owned()),
            was: was.map(str::to_owned),
            place: EVERY,
        }
    }

    fn unbound(key: &str) -> KeyChange {
        KeyChange::Unbind {
            key: key.to_owned(),
            place: EVERY,
        }
    }

    fn restored(key: &str) -> KeyChange {
        KeyChange::Restore {
            key: key.to_owned(),
            place: EVERY,
        }
    }

    fn does(text: &str, key: &str) -> Option<Macro> {
        Keybinds::read(text)
            .0
            .does(&Chord::parse(key).expect("a key"))
            .cloned()
    }

    /// A key bound again is changed where it is, a new one goes at the end
    /// of `[keys]`, and every comment and other table is kept.
    #[test]
    fn a_binding_is_changed_in_place_and_the_rest_kept() {
        let (text, done) = made(&bound("Ctrl+F1", "look around", None)).expect("changed");
        assert_eq!(done, "Ctrl+F1 sends `look around`");
        assert_eq!(
            text,
            FILE_BY_HAND.replace(
                "\"ctrl+F1\" = \"look\"   # the room",
                "\"Ctrl+F1\" = \"look around\""
            )
        );
        let (text, _) = made(&bound("F5", "hide", None)).expect("added");
        assert_eq!(
            text,
            FILE_BY_HAND.replace("# the room\n", "# the room\n\"F5\" = \"hide\"\n")
        );
    }

    /// A key moved does its macro from the new key only; one taken out is
    /// gone, comments and all else kept. A key Hydra binds is written `""`
    /// when it moves or is taken out, so the default goes too; one it does
    /// not is taken out of the file.
    #[test]
    fn a_binding_moves_and_is_taken_out() {
        let (text, _) = made(&bound("Ctrl+F2", "north", Some("Numpad8"))).expect("moved");
        assert_eq!(
            does(&text, "Ctrl+F2"),
            Some(Macro::Send("north".to_owned()))
        );
        assert_eq!(does(&text, "Numpad8"), None, "Hydra's north too");
        assert!(text.contains("\"Numpad8\" = \"\""), "{text}");
        assert!(text.contains("# Movement."));

        let (text, done) = made(&unbound("Numpad8")).expect("gone");
        assert_eq!(done, "Numpad8 does nothing");
        assert_eq!(
            text,
            FILE_BY_HAND.replace("Numpad8 = \"north\"", "\"Numpad8\" = \"\"")
        );
        let (text, _) = made(&unbound("Ctrl+F1")).expect("gone");
        assert!(!text.contains("look"), "not Hydra's, so taken out: {text}");
    }

    /// Restored, a key's line leaves the file and Hydra's default is back;
    /// a key Hydra does not bind does nothing.
    #[test]
    fn a_default_is_restored() {
        let gone = changed(FILE_BY_HAND, &unbound("Numpad8")).expect("gone").0;
        let (text, done) = changed(&gone, &restored("Numpad8")).expect("back");
        assert_eq!(done, "Numpad8 sends `north` again");
        assert!(!text.contains("Numpad8"), "{text}");
        assert_eq!(
            does(&text, "Numpad8"),
            Some(Macro::Send("north".to_owned()))
        );
        let (_, done) = made(&restored("Ctrl+F1")).expect("gone");
        assert_eq!(done, "Ctrl+F1 does nothing");
    }

    /// Each kind is written so the file reads it back: commands with a
    /// break between them, the input filled, an action.
    #[test]
    fn each_kind_is_written() {
        for made_to in [
            Macro::Send("stance off\rincant 610".to_owned()),
            Macro::Fill("prep 111 ".to_owned()),
            Macro::Act(crate::keys::Action::Stop),
        ] {
            let change = KeyChange::Bind {
                key: "F9".to_owned(),
                does: made_to.clone(),
                was: None,
                place: EVERY,
            };
            let (text, _) = made(&change).expect("written");
            assert_eq!(does(&text, "F9"), Some(made_to), "{text}");
        }
        let (text, _) = made(&bound("F9", "look\rsearch", None)).expect("written");
        assert!(text.contains(r#""F9" = "look\rsearch""#), "{text}");
        assert!(made(&bound("F9", "s1", None)).is_err(), "sends no command");
    }

    /// The numpad's switch is written above the tables, or left out for
    /// its default; with no file at all, a binding makes `[keys]`.
    #[test]
    fn the_numpad_switch_and_a_file_from_nothing() {
        let (text, _) = made(&KeyChange::NumpadAlways(true)).expect("switched");
        assert!(
            text.starts_with("# My keys, by hand.\nnumpad = \"always\"\n\n[keys]\n"),
            "{text}"
        );
        assert!(Keybinds::read(&text).0.numpad_always());
        let (text, _) = changed(&text, &KeyChange::NumpadAlways(false)).expect("back");
        assert!(!text.contains("numpad"), "{text}");

        let (text, _) = changed("", &bound("F5", "look", None)).expect("made");
        assert_eq!(text, "[keys]\n\"F5\" = \"look\"\n");
        let (text, _) = changed("", &KeyChange::NumpadAlways(true)).expect("made");
        assert_eq!(text, "numpad = \"always\"\n");
    }

    /// A key that types is refused; a file this cannot change in place is
    /// refused rather than mangled.
    #[test]
    fn what_cannot_be_written_is_refused() {
        assert!(made(&bound("KeyA", "look", None)).is_err());
        let inline = "keys = { F5 = \"look\" }\n";
        let Err(why) = changed(inline, &bound("F6", "hide", None)) else {
            panic!("an inline table is not changed");
        };
        assert!(why.contains("edit it by hand"), "{why}");
    }

    /// Saved to the data folder, the file binds what the page asked; a file
    /// that does not read is left as it is.
    #[test]
    fn a_change_is_saved_and_a_broken_file_left_alone() {
        let data = std::env::temp_dir().join(format!("cena-keys-write-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        let path = super::super::path(&data);
        let every = (path.as_path(), Whose::Every);
        let keys = (&Keybinds::default(), "Keys");
        assert_eq!(
            apply(&data, every, &bound("F5", "look", None), keys).as_deref(),
            Ok("Keys: F5 sends `look`.")
        );
        let (read, problems) = Keybinds::load(&super::super::path(&data));
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            read.does(&Chord::parse("F5").expect("a key")),
            Some(&Macro::Send("look".to_owned()))
        );

        std::fs::write(super::super::path(&data), "[keys\n").expect("written");
        let Err(why) = apply(&data, every, &bound("F6", "hide", None), keys) else {
            panic!("a file that does not read is not changed");
        };
        assert!(why.contains("does not read"), "{why}");
        assert_eq!(
            std::fs::read_to_string(super::super::path(&data))
                .ok()
                .as_deref(),
            Some("[keys\n")
        );
        let _ = std::fs::remove_dir_all(&data);
    }

    /// A key moved between the files, when the second cannot be saved, is
    /// left where it was: the first file put back, the error naming what
    /// happened.
    #[test]
    fn a_move_whose_second_save_fails_puts_the_first_file_back() {
        let data = std::env::temp_dir().join(format!("cena-keys-pair-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        let every = super::super::path(&data);
        let keys = (&Keybinds::default(), "Keys");
        assert!(
            apply(
                &data,
                (&every, Whose::Every),
                &bound("F5", "look", None),
                keys
            )
            .is_ok()
        );
        let before = std::fs::read_to_string(&every).expect("saved");
        // A file where the character's would be, so nothing can be saved
        // beneath it: the read finds nothing there, the save fails.
        let blocker = data.join("blocker");
        std::fs::write(&blocker, "").expect("written");
        let mine = blocker.join("gsiv_ashryn.keys.toml");
        let taken = KeyChange::Restore {
            key: "F5".to_owned(),
            place: EVERY,
        };
        let moved = KeyChange::Bind {
            key: "F5".to_owned(),
            does: Macro::Send("look".to_owned()),
            was: None,
            place: Place {
                set: 0,
                every: false,
            },
        };
        // The first save fails: nothing was saved.
        let Err(why) = apply_pair(
            &data,
            (&mine, Whose::Character, &moved),
            (&every, Whose::Every, &taken),
            keys,
        ) else {
            panic!("a save beneath a file cannot succeed");
        };
        assert!(why.contains("nothing was changed"), "{why}");
        assert_eq!(std::fs::read_to_string(&every).ok(), Some(before.clone()));
        // The second save fails: the first, which bound F6, is put back.
        let Err(why) = apply_pair(
            &data,
            (&every, Whose::Every, &bound("F6", "hide", None)),
            (&mine, Whose::Character, &taken),
            keys,
        ) else {
            panic!("a save beneath a file cannot succeed");
        };
        assert!(why.contains("nothing was changed"), "{why}");
        assert_eq!(
            std::fs::read_to_string(&every).ok(),
            Some(before),
            "put back"
        );
        let _ = std::fs::remove_dir_all(&data);
    }

    /// A set from 1 to 9 is written under its own table, made when there is
    /// none; a key taken out of it is taken out, not written `""`, so set
    /// 0's shows through again.
    #[test]
    fn a_set_is_written_under_its_table() {
        let in_set = |set| Place { set, every: true };
        let change = KeyChange::Bind {
            key: "F4".to_owned(),
            does: Macro::Send("loot".to_owned()),
            was: None,
            place: in_set(1),
        };
        let (text, done) = made(&change).expect("bound");
        assert_eq!(done, "F4 sends `loot` in set 1");
        assert!(
            text.ends_with("# The end.\n\n[set1]\n\"F4\" = \"loot\"\n"),
            "{text}"
        );
        let (read, _) = KeyFile::read(&text, Whose::Every);
        assert_eq!(read.sets[0].len(), 2, "set 0 as it was");
        let numpad = KeyChange::Bind {
            key: "Numpad8".to_owned(),
            does: Macro::Send("peer north".to_owned()),
            was: None,
            place: in_set(1),
        };
        let (text, _) = changed(&text, &numpad).expect("bound");
        assert!(text.contains("Numpad8 = \"north\""), "set 0's kept: {text}");
        let gone = KeyChange::Unbind {
            key: "Numpad8".to_owned(),
            place: in_set(1),
        };
        let (text, done) = changed(&text, &gone).expect("gone");
        assert_eq!(
            done, "Numpad8 is taken out of set 1, and does what it does in set 0",
            "not said to do nothing: it walks north again"
        );
        assert!(!text.contains("peer"), "{text}");
        assert!(!text.contains("\"Numpad8\" = \"\""), "not unbound: {text}");
    }

    /// A character's file writes `""` only where something beneath it binds
    /// the key -- every character's file, or Hydra -- and keeps the set it
    /// chose above its tables, left out for set 0 alone.
    #[test]
    fn a_characters_file_unbinds_what_is_beneath_it_and_keeps_its_set() {
        let (every, _) = Keybinds::read("[keys]\nF5 = \"look\"\n");
        let mine = |change: &KeyChange, old: &str| {
            super::changed(old, Whose::Character, change, &every).expect("changed")
        };
        let place = Place {
            set: 0,
            every: false,
        };
        let gone = |key: &str| KeyChange::Unbind {
            key: key.to_owned(),
            place,
        };
        assert_eq!(
            mine(&gone("F5"), "").0,
            "[keys]\n\"F5\" = \"\"\n",
            "every character's"
        );
        assert_eq!(
            mine(&gone("Numpad2"), "").0,
            "[keys]\n\"Numpad2\" = \"\"\n",
            "Hydra's"
        );
        let had = "[keys]\n\"F7\" = \"stand\"\n";
        assert_eq!(mine(&gone("F7"), had).0, "[keys]\n", "nothing beneath");
        let back = KeyChange::Restore {
            key: "F5".to_owned(),
            place,
        };
        let (_, done) = mine(&back, "[keys]\n\"F5\" = \"\"\n");
        assert_eq!(done, "F5 sends `look` again", "every character's again");

        let (text, done) = mine(&KeyChange::Choose(3), had);
        assert_eq!(text, "set = 3\n\n[keys]\n\"F7\" = \"stand\"\n");
        assert_eq!(done, "macro set 3 is in use, over set 0");
        assert_eq!(KeyFile::read(&text, Whose::Character).0.chosen, 3);
        let (text, done) = mine(&KeyChange::Choose(0), &text);
        assert_eq!(text, had, "as it was");
        assert_eq!(done, "set 0 alone is in use");
    }
}
