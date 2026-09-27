//! One trigger as the file writes it, read: who it is for, everyone's copy,
//! each character's copy merged over it, and whether its send is held (the
//! parent's module docs). Moved down out of `triggers.rs` when the held send
//! came, rather than grow a file at its cap.

use toml::{Table, Value};

use super::{Entry, Form};
use crate::hunt::chain::overlay;

pub(super) fn entry(name: &str, value: Value) -> Result<Entry, String> {
    if name.trim().is_empty() {
        return Err("a trigger needs a name".into());
    }
    let Value::Table(mut table) = value else {
        return Err("is not a table".into());
    };
    let characters = match table.remove("characters") {
        None => None,
        Some(value) => {
            let names: Vec<String> = value
                .try_into()
                .map_err(|e: toml::de::Error| format!("its `characters`: {}", e.message()))?;
            if names.is_empty() {
                return Err("its `characters` names nobody; leave it out to mean everyone".into());
            }
            Some(names.iter().map(|name| name.to_lowercase()).collect())
        }
    };
    let origin = match table.remove("origin") {
        None => None,
        Some(Value::String(origin)) => Some(origin),
        Some(other) => return Err(format!("its `origin` is {other}, not where it came from")),
    };
    let approved = match table.remove("approved") {
        None => None,
        Some(Value::String(approved)) => Some(approved),
        Some(other) => return Err(format!("its `approved` is {other}, not the line approved")),
    };
    match table.remove("held") {
        None => {}
        Some(Value::Table(held)) if held.values().all(Value::is_str) => {}
        Some(_) => {
            return Err("its `held` is not a table of what was kept and not done".into());
        }
    }
    let overrides = match table.remove("for") {
        None => Table::new(),
        Some(Value::Table(overrides)) => overrides,
        Some(_) => return Err("its `for` is not a table of characters".into()),
    };
    let mut base = form(table.clone())?;
    let mut forms: Vec<(String, Form)> = Vec::new();
    for (character, value) in overrides {
        let Value::Table(over) = value else {
            return Err(format!("for {character}: not a table"));
        };
        if ["characters", "for", "origin", "held", "approved"]
            .iter()
            .any(|key| over.contains_key(*key))
        {
            return Err(format!(
                "for {character}: `characters`, `for`, `origin`, `held` and `approved` \
                 belong to the trigger, not one character"
            ));
        }
        let key = character.to_lowercase();
        if forms.iter().any(|(name, _)| *name == key) {
            return Err(format!("for {character}: that character is named twice"));
        }
        let mut merged = table.clone();
        overlay(&mut merged, over);
        let form = form(merged).map_err(|why| format!("for {character}, {why}"))?;
        forms.push((key, form));
    }
    let mut held = None;
    if let Some(origin) = &origin {
        for form in std::iter::once(&mut base).chain(forms.iter_mut().map(|(_, form)| form)) {
            if form.rule.send != approved
                && let Some(send) = form.rule.send.take()
            {
                held = Some(format!(
                    "its send \"{send}\" came from {origin}, and waits for `trigger approve \
                     {name}`"
                ));
            }
        }
    }
    Ok(Entry {
        name: name.to_owned(),
        characters,
        base,
        overrides: forms,
        held,
    })
}

fn form(mut table: Table) -> Result<Form, String> {
    let enabled = match table.remove("enabled") {
        None => true,
        Some(Value::Boolean(on)) => on,
        Some(other) => return Err(format!("its `enabled` is {other}, not true or false")),
    };
    let rule = table
        .try_into()
        .map_err(|e: toml::de::Error| e.message().to_owned())?;
    Ok(Form { enabled, rule })
}
