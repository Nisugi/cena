//! A keys file read (`plan/52` §2, step 2): every character's, the keybinds
//! file in the data folder, or one character's own beside its settings.
//! Both hold the ten macro sets, set 0 under `[keys]` and the others under
//! `[set1]` to `[set9]`; every character's holds the numpad's switch too, and
//! a character's the set it has chosen:
//!
//! ```toml
//! set = 1                       # a character's: the set in use
//!
//! [keys]                        # set 0, always active
//! F2 = "stance offensive"
//!
//! [set1]                        # over set 0 while set 1 is chosen
//! F4 = "loot"
//! ```

use std::collections::BTreeMap;
use std::path::Path;

use super::{Chord, Macro};

/// How many macro sets there are: 0 to 9, as Wrayth has.
pub(crate) const SETS: u8 = 10;

/// One set's keys in one file: a macro, or `None` where `""` unbinds what
/// would bind beneath it.
pub(crate) type Layer = BTreeMap<Chord, Option<Macro>>;

/// Whose keys a file holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Whose {
    /// Every character's: the keybinds file.
    Every,
    /// One character's own.
    Character,
}

/// A keys file, read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct KeyFile {
    /// Its ten sets, set 0 first.
    pub(crate) sets: [Layer; SETS as usize],
    /// Every character's: the numpad sends its keys with `NumLock` on too,
    /// `numpad = "always"`.
    pub(crate) numpad_always: bool,
    /// A character's: the set it has chosen, 0 for set 0 alone.
    pub(crate) chosen: u8,
}

/// The table set `set` is written under: `keys` for set 0, `set1` to
/// `set9` for the others.
pub(crate) fn table(set: u8) -> String {
    if set == 0 {
        "keys".to_owned()
    } else {
        format!("set{set}")
    }
}

/// The set a table is, by its name.
fn set_of(name: &str) -> Option<u8> {
    if name == "keys" {
        return Some(0);
    }
    let set: u8 = name.strip_prefix("set")?.parse().ok()?;
    (1..SETS).contains(&set).then_some(set)
}

impl KeyFile {
    /// Read `text`, a keys file of `whose`. Every binding that is wrong is
    /// said, and the rest still bind; a file that is not TOML binds nothing.
    pub(crate) fn read(text: &str, whose: Whose) -> (Self, Vec<String>) {
        let mut file = Self::default();
        let table = match toml::from_str::<toml::Table>(text) {
            Ok(table) => table,
            Err(why) => return (file, vec![why.to_string()]),
        };
        let mut problems = Vec::new();
        for (name, value) in table {
            match (name.as_str(), whose, value) {
                ("numpad", Whose::Every, toml::Value::String(said)) => match said.as_str() {
                    "numlock" => {}
                    "always" => file.numpad_always = true,
                    other => problems.push(format!(
                        "numpad = \"{other}\": it is \"numlock\" or \"always\"."
                    )),
                },
                ("set", Whose::Character, toml::Value::Integer(set))
                    if (0..i64::from(SETS)).contains(&set) =>
                {
                    file.chosen = u8::try_from(set).unwrap_or(0);
                }
                ("set", Whose::Character, _) => {
                    problems.push("set is a macro set from 0 to 9.".to_owned());
                }
                (name, _, toml::Value::Table(keys)) if set_of(name).is_some() => {
                    let set = set_of(name).unwrap_or(0);
                    let at = if set == 0 {
                        String::new()
                    } else {
                        format!("[{name}] ")
                    };
                    for (written, value) in keys {
                        match bound(&written, &value) {
                            Ok((chord, made)) => {
                                file.sets[usize::from(set)].insert(chord, made);
                            }
                            Err(why) => problems.push(format!("{at}{why}")),
                        }
                    }
                }
                (name, _, _) => problems.push(format!(
                    "`{name}` is not something a keys file holds: [keys], [set1] to [set9]{}.",
                    match whose {
                        Whose::Every => " and numpad",
                        Whose::Character => " and set",
                    }
                )),
            }
        }
        (file, problems)
    }

    /// Read the file of `whose` at `path`; none there is no bindings and no
    /// problem. Each problem names the file.
    pub(crate) fn load(path: &Path, whose: Whose) -> (Self, Vec<String>) {
        let name = path
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        let (file, problems) = match std::fs::read_to_string(path) {
            Ok(text) => Self::read(&text, whose),
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => (Self::default(), Vec::new()),
            Err(why) => (Self::default(), vec![why.to_string()]),
        };
        let problems = problems
            .into_iter()
            .map(|why| format!("{name}: {why}"))
            .collect();
        (file, problems)
    }

    /// How many keys it binds or unbinds, in every set.
    pub(crate) fn len(&self) -> usize {
        self.sets.iter().map(BTreeMap::len).sum()
    }
}

/// A key as the file writes it and its value: the chord and its macro, or
/// `None` for `""`.
fn bound(written: &str, value: &toml::Value) -> Result<(Chord, Option<Macro>), String> {
    let chord = Chord::parse(written)?;
    if let Some(why) = chord.refused() {
        return Err(why);
    }
    // Each command one line, or it is said, not bound: a line with a
    // newline would send two (the crate review of 2026-09-28, R10); a send
    // macro's commands are cut apart first.
    let made = Macro::read(value).map_err(|why| format!("`{written}`: {why}."))?;
    Ok((chord, made))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(written: &str) -> Chord {
        Chord::parse(written).expect("a key")
    }

    /// Each set is read from its own table; a character's file keeps the
    /// set it chose, every character's the numpad's switch, and each says
    /// what the other holds that it does not.
    #[test]
    fn each_set_is_read_from_its_table() {
        let text = "set = 3\n[keys]\nF2 = \"stance offensive\"\n[set3]\nF4 = \"loot\"\nF2 = \"\"\n";
        let (file, problems) = KeyFile::read(text, Whose::Character);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(file.chosen, 3);
        assert_eq!(
            file.sets[0].get(&key("F2")),
            Some(&Some(Macro::Send("stance offensive".to_owned())))
        );
        assert_eq!(
            file.sets[3].get(&key("F4")),
            Some(&Some(Macro::Send("loot".to_owned())))
        );
        assert_eq!(file.sets[3].get(&key("F2")), Some(&None));
        assert_eq!(file.len(), 3);

        let (every, problems) = KeyFile::read(text, Whose::Every);
        assert_eq!(every.chosen, 0);
        assert_eq!(problems.len(), 1, "{problems:?}");
        let (mine, problems) = KeyFile::read("numpad = \"always\"\n", Whose::Character);
        assert!(!mine.numpad_always);
        assert_eq!(problems.len(), 1, "{problems:?}");
    }

    /// A set out of range, a table that is no set, and a key wrong inside
    /// a set are each said, the set named.
    #[test]
    fn what_is_wrong_is_said() {
        let (file, problems) = KeyFile::read(
            "set = 10\n[set0]\nF1 = \"a\"\n[set10]\nF1 = \"a\"\n[set2]\nKeyA = \"attack\"\nF1 = \"hide\"\n",
            Whose::Character,
        );
        assert_eq!(file.chosen, 0);
        assert_eq!(file.len(), 1, "set 2's F1");
        assert_eq!(problems.len(), 4, "{problems:?}");
        assert!(
            problems
                .iter()
                .any(|why| why.starts_with("[set2] KeyA types"))
        );
        assert_eq!(set_of("set9"), Some(9));
        assert_eq!(table(0), "keys");
        assert_eq!(table(4), "set4");
    }
}
