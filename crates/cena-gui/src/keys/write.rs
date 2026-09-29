//! The keybinds file's writer (`plan/50` §7 step 2), until now a hand edit
//! only. The Keys page's changes are made one at a time to the file's text,
//! a line at a time, so what a player wrote there by hand -- a comment, the
//! order -- is kept. The result is read back before it is saved: a file
//! this cannot change in place, a `[keys]` written as an inline table say,
//! is refused rather than mangled.

use std::path::Path;

#[cfg(test)]
use super::Macro;
use super::page::KeyChange;
use super::{Chord, FILE, Keybinds};

/// Make `change` to the keybinds file in `data`, the data folder, and save
/// it. What was done, in words for the player.
///
/// # Errors
///
/// Why nothing was changed.
pub(crate) fn apply(data: &Path, change: &KeyChange) -> Result<String, String> {
    let path = super::path(data);
    let old = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(why) => return Err(format!("Keys: nothing was changed: {FILE}: {why}")),
    };
    if let Err(why) = toml::from_str::<toml::Table>(&old) {
        return Err(format!(
            "Keys: nothing was changed while {FILE} does not read: {why}"
        ));
    }
    let (text, done) = changed(&old, change)?;
    cena_session::store::save_text(data, &path, &text)
        .map_err(|why| format!("Keys: {done}, but {FILE} was not saved: {why}"))?;
    Ok(format!("Keys: {done}."))
}

/// `old` with `change` made, read back to be sure it holds, and what was
/// done.
fn changed(old: &str, change: &KeyChange) -> Result<(String, String), String> {
    let chord = |written: &str| Chord::parse(written).map_err(|why| format!("Keys: {why}"));
    let hydras = Keybinds::default();
    let (text, done) = match change {
        KeyChange::Bind { key, does, was } => {
            let to = chord(key)?;
            if to.types() {
                return Err(format!(
                    "Keys: {key} types: bind it with Ctrl, Alt or Cmd, or it could not be typed."
                ));
            }
            does.check().map_err(|why| format!("Keys: {key}: {why}."))?;
            let text = match was.as_deref().map(chord).transpose()? {
                Some(was) => vacate(old, &was, &hydras),
                None => old.to_owned(),
            };
            (
                bind(&text, &to, Some(&does.written())),
                format!("{} {}", to.written(), does.said()),
            )
        }
        KeyChange::Unbind(key) => {
            let gone = chord(key)?;
            (
                vacate(old, &gone, &hydras),
                format!("{} does nothing", gone.written()),
            )
        }
        KeyChange::Restore(key) => {
            let back = chord(key)?;
            let done = match hydras.does(&back) {
                Some(does) => format!(
                    "{} {} again, as Hydra binds it",
                    back.written(),
                    does.said()
                ),
                None => format!("{} does nothing", back.written()),
            };
            (bind(old, &back, None), done)
        }
        KeyChange::NumpadAlways(always) => (
            numpad(old, *always),
            if *always {
                "the numpad sends its keys with NumLock on too"
            } else {
                "the numpad sends its keys with NumLock off"
            }
            .to_owned(),
        ),
    };
    let does = |read: &Keybinds, key: &str| {
        Chord::parse(key)
            .ok()
            .and_then(|key| read.does(&key).cloned())
    };
    let holds = toml::from_str::<toml::Table>(&text).is_ok() && {
        let read = Keybinds::read(&text).0;
        match change {
            KeyChange::Bind {
                key,
                does: made,
                was,
            } => {
                does(&read, key).as_ref() == Some(made)
                    && was.as_deref().is_none_or(|was| does(&read, was).is_none())
            }
            KeyChange::Unbind(key) => does(&read, key).is_none(),
            KeyChange::Restore(key) => does(&read, key) == does(&hydras, key),
            KeyChange::NumpadAlways(always) => read.numpad_always == *always,
        }
    };
    if !holds {
        return Err(format!(
            "Keys: nothing was changed: {FILE} is written in a way this cannot change; edit it by hand."
        ));
    }
    Ok((text, done))
}

/// `text` with `chord` doing nothing: written `""` where Hydra binds it,
/// so the default is unbound too, and taken out of the file where not.
fn vacate(text: &str, chord: &Chord, hydras: &Keybinds) -> String {
    if hydras.has_default(chord) {
        bind(text, chord, Some("\"\""))
    } else {
        bind(text, chord, None)
    }
}

/// `text` with `chord` bound to `value`, a TOML value as the file writes
/// it, or to nothing of the file's: each line that binds it changed or
/// taken out, or one added at the end of `[keys]`.
fn bind(text: &str, chord: &Chord, value: Option<&str>) -> String {
    let mut entry = value.map(|value| format!("{} = {value}", quoted(&chord.written())));
    let mut lines: Vec<String> = Vec::new();
    let mut in_keys = false;
    let mut end_of_keys = None;
    for raw in text.lines() {
        if let Some(keys) = table(raw) {
            in_keys = keys;
            lines.push(raw.to_owned());
            if keys {
                end_of_keys = Some(lines.len());
            }
            continue;
        }
        let key = in_keys.then(|| entry_key(raw)).flatten();
        if let Some(key) = key {
            if Chord::parse(&key).is_ok_and(|bound| bound == *chord) {
                lines.extend(entry.take());
            } else {
                lines.push(raw.to_owned());
            }
            end_of_keys = Some(lines.len());
            continue;
        }
        lines.push(raw.to_owned());
    }
    if let Some(entry) = entry {
        if let Some(at) = end_of_keys {
            lines.insert(at, entry);
        } else {
            if lines.last().is_some_and(|last| !last.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push("[keys]".to_owned());
            lines.push(entry);
        }
    }
    joined(&lines)
}

/// `text` with `numpad = "always"` above its tables, or without it: the
/// default, `numlock`, is written by leaving it out.
fn numpad(text: &str, always: bool) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut first_table = None;
    for raw in text.lines() {
        if first_table.is_none() {
            if table(raw).is_some() {
                first_table = Some(lines.len());
            } else if entry_key(raw).as_deref() == Some("numpad") {
                continue;
            }
        }
        lines.push(raw.to_owned());
    }
    if always {
        let at = first_table.unwrap_or(lines.len());
        if at < lines.len() {
            lines.insert(at, String::new());
        }
        lines.insert(at, "numpad = \"always\"".to_owned());
    }
    joined(&lines)
}

/// Whether `raw` opens a table, and if it does, whether it is `[keys]`.
fn table(raw: &str) -> Option<bool> {
    let trimmed = raw.trim_start();
    if !trimmed.starts_with('[') {
        return None;
    }
    let keys = !trimmed.starts_with("[[")
        && toml::from_str::<toml::Table>(trimmed).is_ok_and(|table| {
            table.len() == 1
                && table
                    .get("keys")
                    .and_then(toml::Value::as_table)
                    .is_some_and(toml::Table::is_empty)
        });
    Some(keys)
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

    const FILE_BY_HAND: &str = "\
# My keys, by hand.
numpad = \"numlock\"

[keys]
# Movement.
Numpad8 = \"north\"
\"ctrl+F1\" = \"look\"   # the room

# The end.
";

    fn made(change: &KeyChange) -> Result<(String, String), String> {
        changed(FILE_BY_HAND, change)
    }

    fn bound(key: &str, line: &str, was: Option<&str>) -> KeyChange {
        KeyChange::Bind {
            key: key.to_owned(),
            does: Macro::Send(line.to_owned()),
            was: was.map(str::to_owned),
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

        let (text, done) = made(&KeyChange::Unbind("Numpad8".to_owned())).expect("gone");
        assert_eq!(done, "Numpad8 does nothing");
        assert_eq!(
            text,
            FILE_BY_HAND.replace("Numpad8 = \"north\"", "\"Numpad8\" = \"\"")
        );
        let (text, _) = made(&KeyChange::Unbind("Ctrl+F1".to_owned())).expect("gone");
        assert!(!text.contains("look"), "not Hydra's, so taken out: {text}");
    }

    /// Restored, a key's line leaves the file and Hydra's default is back;
    /// a key Hydra does not bind does nothing.
    #[test]
    fn a_default_is_restored() {
        let unbound = changed(FILE_BY_HAND, &KeyChange::Unbind("Numpad8".to_owned()))
            .expect("gone")
            .0;
        let (text, done) =
            changed(&unbound, &KeyChange::Restore("Numpad8".to_owned())).expect("back");
        assert_eq!(done, "Numpad8 sends `north` again, as Hydra binds it");
        assert!(!text.contains("Numpad8"), "{text}");
        assert_eq!(
            does(&text, "Numpad8"),
            Some(Macro::Send("north".to_owned()))
        );
        let (_, done) = made(&KeyChange::Restore("Ctrl+F1".to_owned())).expect("gone");
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
            text.starts_with("# My keys, by hand.\n\nnumpad = \"always\"\n\n[keys]\n"),
            "{text}"
        );
        assert!(Keybinds::read(&text).0.numpad_always);
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
        assert_eq!(
            apply(&data, &bound("F5", "look", None)).as_deref(),
            Ok("Keys: F5 sends `look`.")
        );
        let (read, problems) = Keybinds::load(&super::super::path(&data));
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            read.does(&Chord::parse("F5").expect("a key")),
            Some(&Macro::Send("look".to_owned()))
        );

        std::fs::write(super::super::path(&data), "[keys\n").expect("written");
        let Err(why) = apply(&data, &bound("F6", "hide", None)) else {
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
}
