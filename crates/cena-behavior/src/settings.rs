//! A setting changed from the game line, so a player never has to open a
//! profile file: `;hunt set`, `;hunt unset`, `;hunt show`, `;heal set` and
//! `;heal show` all come here (author, 2026-09-26: *"You can't expect me to
//! test ;hunt if I have to write files by hand"*).
//!
//! A key is dotted, as `;hunt show` prints it: `rooms.resting`,
//! `rest.until.mana`, `sequences.volley.when`. A number picks one of a list,
//! counting from 1 as `;hunt check` does: `targets.2.routine`.
//!
//! A value is typed as it would be said:
//!
//! | Typed | Is |
//! |---|---|
//! | `on`, `yes`, `true` / `off`, `no`, `false` | a switch |
//! | `29877`, `0.5` | a number |
//! | `["store weapon", "ready weapon"]`, `{ name = "warg", routine = "a" }` | a list, a table |
//! | anything else: `herb pouch`, `expiring "Briar Betrayer" 7` | the words, as written |
//!
//! These functions change the text; the caller writes it and then reads the
//! file back the way its behavior will, putting the old text back if that
//! fails, so a bad value is refused by name rather than saved.
//!
//! The comment block at the head of a file is kept: the importer writes what
//! it held there. The rest is written back in the file's own key order.

use std::io;
use std::path::Path;

use toml::{Table, Value};

/// A per-character settings file, as it was found.
///
/// Three answers, not two: reading `.ok()` made a file that is there and
/// broken look missing, so an edit started from the defaults and wrote them
/// over everything the player had (`plan/44` Q05).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stored<T> {
    /// No file: an edit starts from the defaults.
    Missing,
    /// Read and understood.
    Found(T),
    /// There, and unreadable or not understood. Never written over by an
    /// edit; the message names the file and what is wrong with it.
    Broken(String),
}

/// One setting as the settings menu shows it (`plan/50` §7 step 1): its
/// key in the file, what a player calls it, a line of what it does, and the
/// kind of value it takes. Each profile keeps a table of these beside its
/// struct, one per field, in the struct's order; a test holds the two
/// together, as `VellumFE`'s settings registry does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    /// The key in the file.
    pub name: &'static str,
    /// What a player calls it.
    pub label: &'static str,
    /// A line of what it does.
    pub help: &'static str,
    /// The kind of value it takes.
    pub kind: KeyKind,
}

/// The kind of value a setting takes, which decides how it is edited.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KeyKind {
    /// On or off.
    Toggle,
    /// A whole number, from `min` to `max`.
    Whole {
        /// The least.
        min: u32,
        /// The most.
        max: u32,
    },
    /// A number, from `min` to `max`.
    Number {
        /// The least.
        min: f64,
        /// The most.
        max: f64,
    },
    /// Words.
    Text,
    /// A list of whole numbers: spells, rooms.
    Numbers,
    /// A list of words.
    Words,
    /// Names each given a value, changed with the behavior's own command
    /// (`;sc alias`): shown, not edited, in the menu.
    Map,
}

/// The names of `table`'s keys, in its order.
#[must_use]
pub fn names(table: &[Key]) -> Vec<&'static str> {
    table.iter().map(|key| key.name).collect()
}

/// A setting's value as a profile holds it, for the settings menu: numbers
/// as the file writes them, so nothing is lost to a conversion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Held {
    /// On or off.
    Bool(bool),
    /// A number or words, as written.
    Text(String),
    /// A list, each item as written.
    List(Vec<String>),
    /// Names each given a value.
    Map(Vec<(String, String)>),
}

/// One of a table's settings as a profile holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    /// Which setting.
    pub key: Key,
    /// Its value in effect; `None` when it is unset and has no default.
    pub value: Option<Held>,
    /// Whether the file itself sets it, rather than the default.
    pub here: bool,
}

/// Each of `table`'s settings, in its order, as the file's text `own` holds
/// it. `canonical` is the same text read back as the behavior reads it, its
/// defaults filled in.
///
/// # Errors
///
/// Either text is not TOML.
pub fn shown(own: &str, canonical: &str, table: &[Key]) -> Result<Vec<Shown>, String> {
    let own: Table = own.parse().map_err(|e: toml::de::Error| e.to_string())?;
    let full: Table = canonical
        .parse()
        .map_err(|e: toml::de::Error| e.to_string())?;
    Ok(table
        .iter()
        .map(|key| Shown {
            key: *key,
            value: full.get(key.name).map(held),
            here: own.contains_key(key.name),
        })
        .collect())
}

/// A TOML value as the menu shows it.
fn held(value: &Value) -> Held {
    let item = |value: &Value| match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    };
    match value {
        Value::Boolean(on) => Held::Bool(*on),
        Value::Array(items) => Held::List(items.iter().map(item).collect()),
        Value::Table(table) => Held::Map(
            table
                .iter()
                .map(|(name, value)| (name.clone(), item(value)))
                .collect(),
        ),
        other => Held::Text(item(other)),
    }
}

/// Read a settings file with `parse`.
pub fn read<T>(path: &Path, parse: impl FnOnce(&str) -> Result<T, String>) -> Stored<T> {
    match std::fs::read_to_string(path) {
        Ok(text) => match parse(&text) {
            Ok(value) => Stored::Found(value),
            Err(why) => Stored::Broken(format!(
                "{} does not read: {why}. Fix it there, or delete the file to start again from the defaults",
                path.display()
            )),
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => Stored::Missing,
        Err(e) => Stored::Broken(format!("cannot read {}: {e}", path.display())),
    }
}

/// A settings file's text, for an edit: [`Stored::Broken`], naming the file,
/// when it is not TOML. Only the syntax is asked here, so a file with a key
/// its behavior refuses can still be edited, and `unset` can take the key
/// out.
#[must_use]
pub fn read_text(path: &Path) -> Stored<String> {
    read(path, |text| parse(text).map(|_| text.to_owned()))
}

/// Write `text` to `path` whole or not at all: to a file beside it, then
/// renamed over it, so a failure part way leaves the old file as it was.
/// The directory is made if missing.
///
/// # Errors
///
/// The directory cannot be made, or the file written or renamed.
pub fn save(path: &Path, text: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let beside = path.with_extension("toml.saving");
    std::fs::write(&beside, text)?;
    std::fs::rename(&beside, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&beside);
    })
}

/// A value as a player typed it (the module docs).
#[must_use]
pub fn typed(words: &str) -> Value {
    let words = words.trim();
    match words.to_ascii_lowercase().as_str() {
        "on" | "yes" | "true" => return Value::Boolean(true),
        "off" | "no" | "false" => return Value::Boolean(false),
        _ => {}
    }
    format!("v = {words}")
        .parse::<Table>()
        .ok()
        .and_then(|mut table| table.remove("v"))
        .unwrap_or_else(|| Value::String(words.to_owned()))
}

/// A path as typed, without the quotes around it: Windows' "Copy as path"
/// adds them, and `"` cannot be in a Windows file name (os error 123). What
/// `;hunt import`, `;hunt import-loot` and `;trigger import` read.
#[must_use]
pub fn unquoted(path: &str) -> String {
    ['"', '\'']
        .iter()
        .find_map(|&q| path.strip_prefix(q)?.strip_suffix(q))
        .unwrap_or(path)
        .to_owned()
}

/// `text` with `key` set to `value`, and the value it replaced, if any.
///
/// # Errors
///
/// `text` is not TOML, the key is empty, or a number in it picks nothing.
pub fn set(text: &str, key: &str, value: Value) -> Result<(String, Option<Value>), String> {
    let (head, mut table) = split(text)?;
    let old = set_in(&mut table, key, value)?;
    Ok((join(&head, &table)?, old))
}

/// [`set`], in a table already read: `key` set to `value`, and the value it
/// replaced.
pub(crate) fn set_in(table: &mut Table, key: &str, value: Value) -> Result<Option<Value>, String> {
    let (last, parents) = path(key)?;
    Ok(descend(table, &parents, true)?
        .ok_or_else(|| format!("`{key}` is not a setting's name"))?
        .insert(last.to_owned(), value))
}

/// `text` without `key`, so the level below decides it again; and whether
/// it was there.
///
/// # Errors
///
/// `text` is not TOML, or the key is empty.
pub fn unset(text: &str, key: &str) -> Result<(String, bool), String> {
    let (head, mut table) = split(text)?;
    let removed = unset_in(&mut table, key)?;
    Ok((join(&head, &table)?, removed))
}

/// [`unset`], in a table already read: whether `key` was there.
pub(crate) fn unset_in(table: &mut Table, key: &str) -> Result<bool, String> {
    let (last, parents) = path(key)?;
    Ok(descend(table, &parents, false)?
        .and_then(|at| at.remove(last))
        .is_some())
}

/// Every setting under `key` (or all of them), one per line, as `set` takes
/// them: `rest.until.mana = 90`.
///
/// # Errors
///
/// `key` names nothing in `table`.
pub fn lines(table: &Table, key: Option<&str>) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    match key.map(str::trim).filter(|k| !k.is_empty()) {
        None => flatten("", table, &mut out),
        Some(key) => {
            let value = lookup(table, key).ok_or_else(|| format!("there is no `{key}`"))?;
            match value {
                Value::Table(inner) => flatten(key, inner, &mut out),
                other => out.push(format!("{key} = {other}")),
            }
        }
    }
    Ok(out)
}

/// [`lines`], over a file's text.
///
/// # Errors
///
/// `text` is not TOML, or `key` names nothing in it.
pub fn text_lines(text: &str, key: Option<&str>) -> Result<Vec<String>, String> {
    lines(&parse(text)?, key)
}

/// Those of `keys` that `text` does not set.
///
/// # Errors
///
/// `text` is not TOML.
pub fn not_set<'a>(text: &str, keys: &[&'a str]) -> Result<Vec<&'a str>, String> {
    let table = parse(text)?;
    Ok(keys
        .iter()
        .copied()
        .filter(|key| lookup(&table, key).is_none())
        .collect())
}

fn parse(text: &str) -> Result<Table, String> {
    text.parse().map_err(|e: toml::de::Error| e.to_string())
}

/// The value at a dotted key.
#[must_use]
pub fn lookup<'a>(table: &'a Table, key: &str) -> Option<&'a Value> {
    let mut parts = key.split('.');
    let mut at = table.get(parts.next()?)?;
    for part in parts {
        at = match at {
            Value::Table(inner) => inner.get(part)?,
            Value::Array(items) => items.get(part.parse::<usize>().ok()?.checked_sub(1)?)?,
            _ => return None,
        };
    }
    Some(at)
}

fn flatten(prefix: &str, table: &Table, out: &mut Vec<String>) {
    for (key, value) in table {
        let key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        match value {
            Value::Table(inner) if !inner.is_empty() => flatten(&key, inner, out),
            Value::Array(items)
                if !items.is_empty() && items.iter().all(|i| matches!(i, Value::Table(_))) =>
            {
                for (n, item) in items.iter().enumerate() {
                    if let Value::Table(inner) = item {
                        flatten(&format!("{key}.{}", n + 1), inner, out);
                    }
                }
            }
            other => out.push(format!("{key} = {other}")),
        }
    }
}

/// The key's last part, and the parts leading to it.
fn path(key: &str) -> Result<(&str, Vec<&str>), String> {
    let mut parts: Vec<&str> = key.trim().split('.').collect();
    let last = parts.pop().filter(|p| !p.is_empty());
    match last {
        Some(last) if parts.iter().all(|p| !p.is_empty()) => Ok((last, parts)),
        _ => Err(format!("`{key}` is not a setting's name")),
    }
}

/// The table the dotted `parts` lead to. A number after a list of tables
/// picks one of it, from 1. With `make`, a missing table is made, a list of
/// anything else becomes a table holding it as `steps` (a sequence written
/// as a list, given a `when`, keeps its steps: `profile/sequence.rs`), and
/// anything else in the way is replaced; without `make`, those answer `None`.
fn descend<'a>(
    mut at: &'a mut Table,
    parts: &[&str],
    make: bool,
) -> Result<Option<&'a mut Table>, String> {
    let mut parts = parts.iter().peekable();
    while let Some(&part) = parts.next() {
        if !make && !at.contains_key(part) {
            return Ok(None);
        }
        let slot = at
            .entry(part.to_owned())
            .or_insert_with(|| Value::Table(Table::new()));
        let numbered = parts.peek().is_some_and(|n| n.parse::<usize>().is_ok());
        if make
            && !numbered
            && let Value::Array(items) = slot
            && !items.iter().any(|i| matches!(i, Value::Table(_)))
        {
            let steps = Value::Array(std::mem::take(items));
            *slot = Value::Table(Table::from_iter([("steps".to_owned(), steps)]));
        }
        at = match slot {
            Value::Table(inner) => inner,
            Value::Array(items) => {
                let picked = parts
                    .next()
                    .and_then(|n| n.parse::<usize>().ok())
                    .and_then(|n| n.checked_sub(1))
                    .and_then(|n| items.get_mut(n));
                match picked {
                    Some(Value::Table(inner)) => inner,
                    _ => {
                        return Err(format!(
                            "`{part}` is a list: pick one of its tables by number, as `{part}.1`"
                        ));
                    }
                }
            }
            other if make => {
                *other = Value::Table(Table::new());
                match other {
                    Value::Table(inner) => inner,
                    _ => return Ok(None),
                }
            }
            _ => return Ok(None),
        };
    }
    Ok(Some(at))
}

/// The file's head comment block, and its table.
pub(crate) fn split(text: &str) -> Result<(String, Table), String> {
    let mut head = String::new();
    for line in text
        .lines()
        .take_while(|line| line.trim_start().starts_with('#') || line.trim().is_empty())
    {
        head.push_str(line);
        head.push('\n');
    }
    let table = text.parse::<Table>().map_err(|e| e.to_string())?;
    Ok((head, table))
}

/// The file's text again: its head comment block, then its table.
pub(crate) fn join(head: &str, table: &Table) -> Result<String, String> {
    let body = toml::to_string_pretty(table).map_err(|e| e.to_string())?;
    Ok(format!("{head}{body}"))
}

#[cfg(test)]
mod tests {
    use super::{lines, set, typed, unset};
    use toml::Value;

    #[test]
    fn a_value_is_typed_as_it_is_said() {
        assert_eq!(typed("on"), Value::Boolean(true));
        assert_eq!(typed("No"), Value::Boolean(false));
        assert_eq!(typed("29877"), Value::Integer(29877));
        assert_eq!(typed("herb pouch"), Value::String("herb pouch".into()));
        assert_eq!(
            typed(r#"expiring "Briar Betrayer" 7"#),
            Value::String(r#"expiring "Briar Betrayer" 7"#.into())
        );
        assert!(matches!(typed(r#"["a", "b"]"#), Value::Array(items) if items.len() == 2));
    }

    #[test]
    fn set_keeps_the_head_comments_and_the_other_keys() -> Result<(), String> {
        let text = "# imported\n# held: nothing\n\nprepare = [\"x\"]\n\n[rooms]\nhunting = 1\nresting = 2\n";
        let (out, old) = set(text, "rooms.resting", typed("29877"))?;
        assert!(out.starts_with("# imported\n# held: nothing\n"), "{out}");
        assert!(out.contains("resting = 29877"), "{out}");
        assert!(out.contains("hunting = 1"), "{out}");
        assert_eq!(old, Some(Value::Integer(2)));
        let (kept, _) = set("[zeta]\na = 1\n\n[alpha]\nb = 2\n", "zeta.a", typed("3"))?;
        assert!(
            kept.find("[zeta]") < kept.find("[alpha]"),
            "the file's order: {kept}"
        );
        let (made, _) = set("", "rest.until.mana", typed("90"))?;
        let table: toml::Table = made.parse().map_err(|e: toml::de::Error| e.to_string())?;
        assert_eq!(
            lines(&table, None)?,
            ["rest.until.mana = 90"],
            "the tables on the way are made"
        );
        Ok(())
    }

    #[test]
    fn a_list_of_tables_is_picked_by_number_from_1() -> Result<(), String> {
        let text = "[[targets]]\nname = \"warg\"\nroutine = \"a\"\n\n[[targets]]\nany = true\nroutine = \"f\"\n";
        let table: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
        assert_eq!(
            lines(&table, Some("targets.2"))?,
            ["targets.2.any = true", "targets.2.routine = \"f\""]
        );
        assert!(set(text, "targets.routine", typed("b")).is_err());
        let (made, _) = set("[s]\nvolley = [\"a\"]\n", "s.volley.when", typed("hidden"))?;
        let table: toml::Table = made.parse().map_err(|e: toml::de::Error| e.to_string())?;
        assert_eq!(
            lines(&table, None)?,
            ["s.volley.steps = [\"a\"]", "s.volley.when = \"hidden\""],
            "a list given a `when` keeps its steps"
        );
        let (out, _) = set(text, "targets.1.routine", typed("b"))?;
        assert!(
            out.contains("routine = \"b\"") && out.contains("routine = \"f\""),
            "{out}"
        );
        Ok(())
    }

    /// Missing, found and broken are three answers: a broken file is never
    /// mistaken for a missing one, and `save` replaces a file whole.
    #[test]
    fn a_broken_file_is_not_a_missing_one() -> Result<(), String> {
        use super::{Stored, read, save};
        let dir = std::env::temp_dir().join(format!("cena-stored-{}", std::process::id()));
        let path = dir.join("p.toml");
        let parse = |t: &str| t.parse::<toml::Table>().map_err(|e| e.to_string());
        assert_eq!(read(&path, parse), Stored::Missing);
        save(&path, "a = 1\n").map_err(|e| e.to_string())?;
        assert!(matches!(read(&path, parse), Stored::Found(_)));
        save(&path, "a = \n").map_err(|e| e.to_string())?;
        let broken = read(&path, parse);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            matches!(&broken, Stored::Broken(why) if why.contains("p.toml")),
            "{broken:?}"
        );
        Ok(())
    }

    #[test]
    fn unset_removes_so_the_level_below_decides() -> Result<(), String> {
        let (out, removed) = unset("[rooms]\nresting = 2\nhunting = 1\n", "rooms.resting")?;
        assert!(removed && !out.contains("resting"), "{out}");
        let (_, removed) = unset("[rooms]\nhunting = 1\n", "rest.fried")?;
        assert!(!removed);
        Ok(())
    }

    /// Each setting is shown with its value in effect -- the file's own, or
    /// the default the behavior fills in -- and whether the file sets it; a
    /// list, a map and an unset one each as they are.
    #[test]
    fn a_setting_is_shown_with_where_it_comes_from() {
        use super::{Held, Key, KeyKind, shown};
        let key = |name: &'static str, kind| Key {
            name,
            label: name,
            help: "",
            kind,
        };
        let table = [
            key("on", KeyKind::Toggle),
            key("list", KeyKind::Numbers),
            key("map", KeyKind::Map),
            key("none", KeyKind::Text),
        ];
        let canonical = "on = true
list = [1, 2]

[map]
boom = 910
";
        let shown = shown(
            "on = true
",
            canonical,
            &table,
        )
        .expect("both read");
        let held: Vec<(Option<Held>, bool)> = shown
            .into_iter()
            .map(|shown| (shown.value, shown.here))
            .collect();
        assert_eq!(
            held,
            [
                (Some(Held::Bool(true)), true),
                (Some(Held::List(vec!["1".into(), "2".into()])), false),
                (Some(Held::Map(vec![("boom".into(), "910".into())])), false),
                (None, false),
            ]
        );
        assert!(super::shown("x = [", "", &table).is_err());
    }
}
