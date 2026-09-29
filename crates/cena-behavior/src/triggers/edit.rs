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
    /// What it watches and what it does, in a line: `"You are stunned" ->
    /// look, sound` (the trigger editor's list, `plan/54` step 1).
    pub summary: String,
    /// Its send, while it waits for the player's approval: it came from
    /// elsewhere (`plan/45` §1 row 1).
    pub held: Option<String>,
    /// Where it came from, when an import brought it.
    pub origin: Option<String>,
}

/// The master switches (`plan/45` §5a): each category and each kind of
/// response, on or off. A category no switch names is on.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Switches {
    /// Every category a trigger has or a switch names, sorted.
    pub categories: Vec<(String, bool)>,
    /// Each of [`KINDS`], in its order.
    pub kinds: Vec<(&'static str, bool)>,
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
            summary: summary(value),
            held: loaded
                .held
                .iter()
                .any(|held| held.name == *name)
                .then(|| value.get("send").and_then(Value::as_str).map(str::to_owned))
                .flatten(),
            origin: value
                .get("origin")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
        .collect();
    listed.sort_by(|a, b| (&a.category, &a.name).cmp(&(&b.category, &b.name)));
    Ok(listed)
}

/// The master switches, as the file sets them.
///
/// # Errors
///
/// The file is not TOML.
pub fn switches(text: &str) -> Result<Switches, String> {
    let (_, table) = settings::split(text)?;
    let set = |name: &str| -> std::collections::BTreeMap<String, bool> {
        table
            .get(name)
            .and_then(Value::as_table)
            .map(|switches| {
                switches
                    .iter()
                    .filter_map(|(key, on)| on.as_bool().map(|on| (key.clone(), on)))
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut categories = set("categories");
    if let Some(Value::Table(triggers)) = table.get("trigger") {
        for trigger in triggers.values() {
            let category = category(trigger);
            if !category.is_empty() {
                categories.entry(category.to_owned()).or_insert(true);
            }
        }
    }
    let kinds = set("responses");
    Ok(Switches {
        categories: categories.into_iter().collect(),
        kinds: KINDS
            .iter()
            .map(|kind| (*kind, kinds.get(*kind).copied().unwrap_or(true)))
            .collect(),
    })
}

/// What a trigger's table watches and does, in a line.
fn summary(trigger: &Value) -> String {
    let get = |key: &str| trigger.get(key);
    let words = |value: &Value| match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map_or_else(|| item.to_string(), str::to_owned)
            })
            .collect::<Vec<_>>()
            .join(" and "),
        other => other.to_string(),
    };
    let mut when = Vec::new();
    if let Some(text) = get("text") {
        when.push(format!("\"{}\"", words(text)));
    }
    if let Some(regex) = get("regex") {
        when.push(format!("/{}/", words(regex)));
    }
    if let Some(event) = get("event") {
        when.push(format!("on {}", words(event)));
    }
    if let Some(condition) = get("condition") {
        when.push(format!("when {}", words(condition)));
    }
    let does: Vec<&str> = KINDS
        .iter()
        .copied()
        .filter(|kind| match get(kind) {
            Some(Value::Boolean(on)) => *on,
            Some(_) => true,
            None => false,
        })
        .collect();
    format!("{} -> {}", when.join(" "), does.join(", "))
}

/// Every key of a trigger's table the trigger editor's form edits
/// (`plan/54` step 2). The rest -- `for`, `origin`, `held`, `approved` --
/// the form leaves as the file has them.
pub const FORM_KEYS: [&str; 23] = [
    "category",
    "enabled",
    "text",
    "regex",
    "case_sensitive",
    "whole_word",
    "stream",
    "event",
    "condition",
    "rearm",
    "only_if",
    "look",
    "squelch",
    "substitute",
    "redirect",
    "flag",
    "sound",
    "notify",
    "alert",
    "send",
    "cooldown",
    "priority",
    "characters",
];

/// Each trigger's own table, by name, in the file's order.
///
/// # Errors
///
/// The file is not TOML.
pub fn tables(text: &str) -> Result<Vec<(String, Table)>, String> {
    let (_, table) = settings::split(text)?;
    let Some(Value::Table(triggers)) = table.get("trigger") else {
        return Ok(Vec::new());
    };
    Ok(triggers
        .iter()
        .filter_map(|(name, value)| Some((name.clone(), value.as_table()?.clone())))
        .collect())
}

/// The trigger editor's form saved: `fields` (only [`FORM_KEYS`]) as
/// trigger `name`'s, which was `was` (`None` for a new trigger, renamed when
/// the names differ). What the form does not edit is kept. With `approve`,
/// its send is approved: the player typed it (`plan/54` §1 row 2).
///
/// # Errors
///
/// The file is not TOML, the name is empty or taken, `was` is not a trigger,
/// or the trigger as saved would be refused (the reason).
pub fn save(
    text: &str,
    was: Option<&str>,
    name: &str,
    fields: Table,
    approve: bool,
) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("a trigger needs a name".to_owned());
    }
    change(text, &[name], |table| {
        let triggers = section(table, "trigger")?;
        let mut own = match was {
            Some(was) => triggers
                .remove(was)
                .and_then(|value| value.as_table().cloned())
                .ok_or_else(|| format!("there is no trigger `{was}`"))?,
            None => Table::new(),
        };
        if triggers.contains_key(name) {
            return Err(format!("there is already a trigger `{name}`"));
        }
        own.retain(|key, _| !FORM_KEYS.contains(&key));
        own.extend(fields);
        let send = own.get("send").and_then(Value::as_str).map(str::to_owned);
        match send {
            Some(send) if approve && own.contains_key("origin") => {
                own.insert("approved".to_owned(), Value::String(send));
            }
            None => {
                own.remove("approved");
            }
            Some(_) => {}
        }
        triggers.insert(name.to_owned(), Value::Table(own));
        Ok(())
    })
}

/// Another player's triggers file read as an import, each trigger marked
/// with `origin` (`plan/54` step 5): what made it theirs, where it came from
/// and what it was approved to send (`for`, `origin`, `held`, `approved`) is
/// left behind, so every send it brings is held until this player approves it
/// (`plan/45` §1 row 1).
///
/// # Errors
///
/// The file is not TOML, or holds no triggers.
pub fn shared(text: &str, origin: &str) -> Result<Import, String> {
    let (_, table) = settings::split(text)?;
    let Some(Value::Table(triggers)) = table.get("trigger") else {
        return Err("it holds no triggers".to_owned());
    };
    let mut brought = Import::default();
    for (name, value) in triggers {
        let Some(own) = value.as_table() else {
            continue;
        };
        let mut own = own.clone();
        for theirs in ["for", "origin", "held", "approved"] {
            own.remove(theirs);
        }
        if own.contains_key("sound") {
            brought.sounds += 1;
        }
        own.insert("origin".to_owned(), Value::String(origin.to_owned()));
        brought.triggers.push((name.clone(), own));
    }
    if brought.triggers.is_empty() {
        return Err("it holds no triggers".to_owned());
    }
    Ok(brought)
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
