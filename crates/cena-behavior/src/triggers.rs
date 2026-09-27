//! The one triggers file (`plan/45` §5): `<CENA_DATA_DIR>/triggers.toml`,
//! every trigger by name, read into each character's own list of
//! [`Trigger`]s.
//!
//! ```toml
//! [trigger."stunned"]
//! category = "Combat"
//! text = "You are stunned"
//! look = { color = "#ff4040", bold = true, span = "line" }
//! characters = ["Nisugi"]          # only these; absent means everyone
//!
//! [trigger."ignore-spell-spam"]
//! category = "Ignores"
//! regex = '^\w+ gestures\.$'
//! squelch = true
//!
//! [trigger."ignore-spell-spam".for.Dicate]
//! enabled = false                  # one character's copy, field by field
//!
//! [trigger."low"]
//! condition = "!health_at_least 30"  # fires when it becomes true
//! flag = { name = "low", seconds = 60 }
//!
//! [categories]
//! Ignores = false                  # every trigger in it off
//!
//! [responses]
//! squelch = false                  # every squelch off
//! ```
//!
//! **One file**, with no packs and no profile level (author, 2026-09-26,
//! `plan/45` §1 row 3): *"One file, players won't be accessing the file most
//! of the time, we will have a gui editor which can give a category, they can
//! be sorted by category in the file."* What a trigger is and does is
//! [`Rule`]'s; this module adds who it is for and the switches.
//!
//! **For whom** is CLAUDE'S READING of "one file", to confirm (`plan/45`
//! §5b). A trigger is everyone's unless `characters` names who. `for.<name>`
//! changes one character's copy field by field, by the hunt chain's rule
//! ([`overlay`]): a table merges key by key and anything else is replaced.
//! That keeps `plan/12` §6a.2's global -> character override for triggers
//! without a profile level. Character names are matched ignoring case.
//!
//! **Master switches** turn off a category (`[categories]`) or one kind of
//! response everywhere (`[responses]`), the Saga complaint: *"no master
//! toggle for ignores"*.
//!
//! **Refused by name.** A trigger the file cannot type is left out, named,
//! with the reason ([`Refused`]), and the rest load: one bad regex among
//! 1,500 highlights does not cost the other 1,499. It is not silently gone
//! either, which is `VellumFE`'s way (`plan/45` §5a). The trigger is the
//! unit: a bad `for.<name>` refuses the whole trigger, rather than leave that
//! character running what the player meant to change. A file that is not
//! TOML loads nothing, and says so.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use cena_session::trigger::{Rule, Trigger};
use serde::Deserialize;
use toml::{Table, Value};

use crate::hunt::chain::overlay;

pub mod edit;

/// The file's name under the data directory.
pub const FILE: &str = "triggers.toml";

/// The triggers file: `<dir>/triggers.toml`.
#[must_use]
pub fn path(dir: &Path) -> PathBuf {
    dir.join(FILE)
}

/// Every trigger the file holds, checked, and its switches.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Triggers {
    entries: Vec<Entry>,
    categories: BTreeMap<String, bool>,
    responses: Responses,
}

/// The file, read: what loaded, and what was refused.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Loaded {
    /// The triggers that loaded.
    pub triggers: Triggers,
    /// What did not, by name.
    pub refused: Vec<Refused>,
}

/// A trigger, or a section of the file, left out, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    /// The trigger's name, or the section's.
    pub name: String,
    /// Why, as the player should read it.
    pub why: String,
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}`: {}", self.name, self.why)
    }
}

/// One trigger as the file has it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    name: String,
    /// Lowercased; `None` is everyone.
    characters: Option<Vec<String>>,
    /// Everyone's copy.
    base: Form,
    /// One character's copy, by lowercased name.
    overrides: Vec<(String, Form)>,
}

/// A trigger as someone has it: on or off, and its rule.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Form {
    enabled: bool,
    rule: Rule,
}

/// Which kinds of response are on, everywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the file's `[responses]` switches, one per kind, carried as written"
)]
struct Responses {
    look: bool,
    squelch: bool,
    substitute: bool,
    redirect: bool,
    flag: bool,
}

impl Default for Responses {
    fn default() -> Self {
        Self {
            look: true,
            squelch: true,
            substitute: true,
            redirect: true,
            flag: true,
        }
    }
}

impl Responses {
    /// `rule` without the kinds switched off; `None` when that leaves it
    /// nothing to do.
    fn apply(self, mut rule: Rule) -> Option<Rule> {
        if !self.look {
            rule.look = None;
        }
        rule.squelch &= self.squelch;
        if !self.substitute {
            rule.substitute = None;
        }
        if !self.redirect {
            rule.redirect = None;
        }
        if !self.flag {
            rule.flag = None;
        }
        let responds = rule.look.is_some()
            || rule.squelch
            || rule.substitute.is_some()
            || rule.redirect.is_some()
            || rule.flag.is_some();
        responds.then_some(rule)
    }
}

impl Triggers {
    /// The triggers `character` runs: theirs or everyone's, in their own
    /// copy, on, with the switches applied, in file order -- category, then
    /// name.
    #[must_use]
    pub fn for_character(&self, character: &str) -> Vec<Trigger> {
        let who = character.to_lowercase();
        let mut mine: Vec<Trigger> = self
            .entries
            .iter()
            .filter(|entry| {
                entry
                    .characters
                    .as_ref()
                    .is_none_or(|names| names.contains(&who))
            })
            .filter_map(|entry| {
                let form = entry
                    .overrides
                    .iter()
                    .find(|(name, _)| *name == who)
                    .map_or(&entry.base, |(_, form)| form);
                let category_on = self.categories.get(&form.rule.category) != Some(&false);
                if !form.enabled || !category_on {
                    return None;
                }
                Some(Trigger {
                    name: entry.name.clone(),
                    rule: self.responses.apply(form.rule.clone())?,
                })
            })
            .collect();
        mine.sort_by(|a, b| (&a.rule.category, &a.name).cmp(&(&b.rule.category, &b.name)));
        mine
    }
}

/// Read the triggers file under `dir`. No file is no triggers.
///
/// # Errors
///
/// The file exists and cannot be read, or is not TOML.
pub fn load(dir: &Path) -> Result<Loaded, String> {
    let path = path(dir);
    match fs::read_to_string(&path) {
        Ok(text) => read(&text).map_err(|why| format!("{} is not TOML: {why}", path.display())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Loaded::default()),
        Err(e) => Err(format!("cannot read {}: {e}", path.display())),
    }
}

/// Read a triggers file's text.
///
/// # Errors
///
/// It is not TOML.
pub fn read(text: &str) -> Result<Loaded, String> {
    let mut table: Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
    let mut loaded = Loaded::default();
    let mut refuse = |name: &str, why: String| {
        loaded.refused.push(Refused {
            name: name.to_owned(),
            why,
        });
    };
    let mut triggers = Triggers::default();
    if let Some(value) = table.remove("categories") {
        match value.try_into() {
            Ok(categories) => triggers.categories = categories,
            Err(e) => refuse("categories", switches(&e)),
        }
    }
    if let Some(value) = table.remove("responses") {
        match value.try_into() {
            Ok(responses) => triggers.responses = responses,
            Err(e) => refuse("responses", switches(&e)),
        }
    }
    match table.remove("trigger") {
        None => {}
        Some(Value::Table(all)) => {
            for (name, value) in all {
                match entry(&name, value) {
                    Ok(entry) => triggers.entries.push(entry),
                    Err(why) => refuse(&name, why),
                }
            }
        }
        Some(_) => refuse("trigger", "is not a table of triggers by name".into()),
    }
    for name in table.keys() {
        refuse(
            name,
            "is not a section of the triggers file, which has `trigger`, `categories` \
             and `responses`"
                .into(),
        );
    }
    loaded.triggers = triggers;
    Ok(loaded)
}

/// A switch section's refusal: on its own, its switches are all on.
fn switches(e: &toml::de::Error) -> String {
    format!("{}; its switches are all on until it is fixed", e.message())
}

fn entry(name: &str, value: Value) -> Result<Entry, String> {
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
    let overrides = match table.remove("for") {
        None => Table::new(),
        Some(Value::Table(overrides)) => overrides,
        Some(_) => return Err("its `for` is not a table of characters".into()),
    };
    let base = form(table.clone())?;
    let mut forms: Vec<(String, Form)> = Vec::new();
    for (character, value) in overrides {
        let Value::Table(over) = value else {
            return Err(format!("for {character}: not a table"));
        };
        if over.contains_key("characters") || over.contains_key("for") {
            return Err(format!(
                "for {character}: `characters` and `for` belong to the trigger, not one character"
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
    Ok(Entry {
        name: name.to_owned(),
        characters,
        base,
        overrides: forms,
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
