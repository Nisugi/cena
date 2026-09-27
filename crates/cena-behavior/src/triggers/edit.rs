//! Changing the triggers file from the game line (`;trigger`, `plan/45`
//! §5c), on the author's rule for the hunt: *"You can't expect me to test
//! ;hunt if I have to write files by hand"*.
//!
//! Each change takes the file's text and gives back the new text, sorted by
//! category, then name (author, `plan/45` §1 row 3), after reading it back
//! as Hydra will: a change that leaves its trigger refused is refused, with
//! the reason, and the caller writes nothing. The head comment block is
//! kept, as the hunt's settings keep theirs (`crate::settings`).

use toml::{Table, Value};

use super::wrayth::{self, Import};
use super::{Refused, read};
use crate::settings;

/// The kinds of response a master switch turns on or off everywhere.
pub const KINDS: [&str; 9] = [
    "look",
    "squelch",
    "substitute",
    "redirect",
    "flag",
    "sound",
    "notify",
    "alert",
    "send",
];

/// What `on` or `off` switches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Switch<'a> {
    /// One trigger, by name.
    Trigger(&'a str),
    /// Every trigger in a category.
    Category(&'a str),
    /// One kind of response, everywhere: one of [`KINDS`].
    Every(&'a str),
}

/// One trigger, as `;trigger list` shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    /// Its category; empty when it has none.
    pub category: String,
    /// Its name.
    pub name: String,
    /// Whether it is on for everyone (before any `for.<name>`).
    pub enabled: bool,
    /// Why it cannot be used, when it cannot.
    pub refused: Option<String>,
}

/// A new trigger `name` on `words`, making them bold: a trigger must do
/// something to be read, and bold is the least a look can do. `set` changes
/// it from there.
///
/// # Errors
///
/// The file is not TOML, the name or the words are empty, the name is taken,
/// or the trigger is refused.
pub fn add(text: &str, name: &str, words: &str) -> Result<String, String> {
    if name.trim().is_empty() || words.is_empty() {
        return Err("a trigger needs a name and the words it matches".into());
    }
    change(text, &[name], |table| {
        let triggers = section(table, "trigger")?;
        if triggers.contains_key(name) {
            return Err(format!("there is already a trigger `{name}`"));
        }
        let look = Table::from_iter([("bold".to_owned(), Value::Boolean(true))]);
        let trigger = Table::from_iter([
            ("text".to_owned(), Value::String(words.to_owned())),
            ("look".to_owned(), Value::Table(look)),
        ]);
        triggers.insert(name.to_owned(), Value::Table(trigger));
        Ok(())
    })
}

/// `name`'s `key` set to `value`, and the value it replaced. `key` is dotted
/// within the trigger, as `show` prints it: `look.color`, `for.Dicate.enabled`.
///
/// # Errors
///
/// The file is not TOML, there is no such trigger, the key is not a
/// setting's name, or the trigger is refused with it.
pub fn set(
    text: &str,
    name: &str,
    key: &str,
    value: Value,
) -> Result<(String, Option<Value>), String> {
    let mut old = None;
    let text = change(text, &[name], |table| {
        old = settings::set_in(trigger(table, name)?, key, value)?;
        Ok(())
    })?;
    Ok((text, old))
}

/// `name` without `key`, so its default decides.
///
/// # Errors
///
/// The file is not TOML, there is no such trigger, it does not set `key`, or
/// the trigger is refused without it.
pub fn unset(text: &str, name: &str, key: &str) -> Result<String, String> {
    change(text, &[name], |table| {
        if settings::unset_in(trigger(table, name)?, key)? {
            Ok(())
        } else {
            Err(format!("`{name}` does not set `{key}`"))
        }
    })
}

/// `name`, which came from elsewhere, approved to send its line: `approved`
/// names that line, so a later change to it is held again. The new text,
/// and the line approved.
///
/// # Errors
///
/// The file is not TOML, there is no such trigger, it is the player's own
/// (it sends already), or it sends nothing.
pub fn approve(text: &str, name: &str) -> Result<(String, String), String> {
    let mut approved = String::new();
    let text = change(text, &[name], |table| {
        let trigger = trigger(table, name)?;
        if trigger.get("origin").is_none() {
            return Err(format!(
                "`{name}` is the player's own, and sends without approval"
            ));
        }
        let send = trigger
            .get("send")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("`{name}` sends nothing"))?
            .to_owned();
        trigger.insert("approved".to_owned(), Value::String(send.clone()));
        approved = send;
        Ok(())
    })?;
    Ok((text, approved))
}

/// The file without trigger `name`.
///
/// # Errors
///
/// The file is not TOML, or there is no such trigger.
pub fn remove(text: &str, name: &str) -> Result<String, String> {
    change(text, &[], |table| {
        section(table, "trigger")?
            .remove(name)
            .map(|_| ())
            .ok_or_else(|| format!("there is no trigger `{name}`"))
    })
}

/// `which` switched on or off.
///
/// # Errors
///
/// The file is not TOML, there is no such trigger, or the kind is not one of
/// [`KINDS`].
pub fn switch(text: &str, which: Switch<'_>, on: bool) -> Result<String, String> {
    match which {
        Switch::Trigger(name) => change(text, &[name], |table| {
            trigger(table, name)?.insert("enabled".to_owned(), Value::Boolean(on));
            Ok(())
        }),
        Switch::Category(category) => change(text, &["categories"], |table| {
            section(table, "categories")?.insert(category.to_owned(), Value::Boolean(on));
            Ok(())
        }),
        Switch::Every(kind) if KINDS.contains(&kind) => change(text, &["responses"], |table| {
            section(table, "responses")?.insert(kind.to_owned(), Value::Boolean(on));
            Ok(())
        }),
        Switch::Every(kind) => Err(format!(
            "`{kind}` is not a kind of response: {}",
            KINDS.join(", ")
        )),
    }
}

/// Trigger `name`'s settings, one per line, as `set` takes them.
///
/// # Errors
///
/// The file is not TOML, or there is no such trigger.
pub fn show(text: &str, name: &str) -> Result<Vec<String>, String> {
    let (_, mut table) = settings::split(text)?;
    settings::lines(trigger(&mut table, name)?, None)
}

/// Every trigger in the file, in its order: category, then name.
///
/// # Errors
///
/// The file is not TOML.
pub fn list(text: &str) -> Result<Vec<Listed>, String> {
    let loaded = read(text)?;
    let (_, table) = settings::split(text)?;
    let Some(Value::Table(triggers)) = table.get("trigger") else {
        return Ok(Vec::new());
    };
    let mut listed: Vec<Listed> = triggers
        .iter()
        .map(|(name, value)| Listed {
            category: category(value).to_owned(),
            name: name.clone(),
            enabled: value.get("enabled").and_then(Value::as_bool) != Some(false),
            refused: loaded
                .refused
                .iter()
                .find(|refused| refused.name == *name)
                .map(|refused| refused.why.clone()),
        })
        .collect();
    listed.sort_by(|a, b| (&a.category, &a.name).cmp(&(&b.category, &b.name)));
    Ok(listed)
}

/// What an import did to the file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Merged {
    /// Triggers an earlier import from the same origin brought, replaced.
    pub replaced: usize,
    /// Triggers given another name because theirs was taken: `(had, given)`.
    pub renamed: Vec<(String, String)>,
    /// Triggers left out, as the file would refuse them.
    pub refused: Vec<Refused>,
}

/// `text` with `import`'s triggers in it, marked with `origin`. What an
/// earlier import from the same `origin` brought is replaced whole, so the
/// same file imported twice is imported once; a name another trigger has is
/// given `(Wrayth)`; a trigger the file would refuse is left out and named.
/// The file's `disable` on its ignores sets their category's switch.
///
/// # Errors
///
/// The triggers file is not TOML, or its `trigger` or `categories` is not a
/// table.
pub fn import(text: &str, origin: &str, import: &Import) -> Result<(String, Merged), String> {
    let (head, mut table) = settings::split(text)?;
    let mut merged = Merged::default();
    let mut names = Vec::new();
    let triggers = section(&mut table, "trigger")?;
    let before = triggers.len();
    triggers.retain(|_, trigger| trigger.get("origin").and_then(Value::as_str) != Some(origin));
    merged.replaced = before - triggers.len();
    for (name, trigger) in &import.triggers {
        let mut given = name.clone();
        let mut count = 1;
        while triggers.contains_key(&given) {
            count += 1;
            given = if count == 2 {
                format!("{name} (Wrayth)")
            } else {
                format!("{name} (Wrayth {count})")
            };
        }
        if given != *name {
            merged.renamed.push((name.clone(), given.clone()));
        }
        triggers.insert(given.clone(), Value::Table(trigger.clone()));
        names.push(given);
    }
    if let Some(on) = import.ignores_on {
        section(&mut table, "categories")?.insert(wrayth::IGNORES.to_owned(), Value::Boolean(on));
    }
    sort(&mut table);
    merged.refused = read(&settings::join(&head, &table)?)?
        .refused
        .into_iter()
        .filter(|refused| names.contains(&refused.name))
        .collect();
    let triggers = section(&mut table, "trigger")?;
    for refused in &merged.refused {
        triggers.remove(&refused.name);
    }
    Ok((settings::join(&head, &table)?, merged))
}

/// `text` with `edit` made and its triggers sorted, unless that leaves any of
/// `checked` -- a trigger, or a switch section -- refused.
fn change(
    text: &str,
    checked: &[&str],
    edit: impl FnOnce(&mut Table) -> Result<(), String>,
) -> Result<String, String> {
    let (head, mut table) = settings::split(text)?;
    edit(&mut table)?;
    sort(&mut table);
    let out = settings::join(&head, &table)?;
    let loaded = read(&out)?;
    if let Some(refused) = loaded
        .refused
        .iter()
        .find(|refused| checked.contains(&refused.name.as_str()))
    {
        return Err(refused.why.clone());
    }
    Ok(out)
}

/// The triggers in file order: category, then name.
fn sort(table: &mut Table) {
    let Some(Value::Table(triggers)) = table.get_mut("trigger") else {
        return;
    };
    let mut entries: Vec<(String, Value)> = std::mem::take(triggers).into_iter().collect();
    entries.sort_by(|(a, at), (b, bt)| (category(at), a).cmp(&(category(bt), b)));
    *triggers = entries.into_iter().collect();
}

fn category(trigger: &Value) -> &str {
    trigger
        .get("category")
        .and_then(Value::as_str)
        .unwrap_or_default()
}

/// The root's table `name`, made if it is missing.
fn section<'a>(table: &'a mut Table, name: &str) -> Result<&'a mut Table, String> {
    table
        .entry(name.to_owned())
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .ok_or_else(|| format!("the file's `{name}` is not a table"))
}

/// Trigger `name`'s own table.
fn trigger<'a>(table: &'a mut Table, name: &str) -> Result<&'a mut Table, String> {
    table
        .get_mut("trigger")
        .and_then(Value::as_table_mut)
        .and_then(|triggers| triggers.get_mut(name))
        .and_then(Value::as_table_mut)
        .ok_or_else(|| format!("there is no trigger `{name}`"))
}
