//! The one triggers file, given to each character's session, and `;trigger`
//! (`plan/45`).
//!
//! The join only: the file is `cena_behavior::triggers`, read, and
//! `cena_behavior::triggers::edit`, changed; what a trigger does to a line
//! is the session's ([`SessionHandle::set_triggers`]). The file is read as a
//! character starts, after every `;trigger` change, and at `;trigger
//! reload`, and each character is given its own list.
//!
//! **A change reaches this character at once, and the others at their next
//! start or `;trigger reload`.** One file serves every character, but a
//! command runs in one character's session, which holds no handle on the
//! others. CLAUDE'S CALL (`plan/45` §5c).
//!
//! A change is written whole or not at all -- to a file beside it, then over
//! it -- because a player's triggers file may hold years of rules.

mod explain;
mod words;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{fs, io};

use cena_behavior::settings::typed;
use cena_behavior::triggers::edit::{self, Switch};
use cena_behavior::triggers::{self, Refused};
use cena_session::command::claimant::Claimed;
use cena_session::trigger::Matcher;
use cena_session::{Notice, NoticeKind, SessionHandle};

use crate::commands::Commands;
use words::{Command, HELP, Target};

/// What `;trigger` says, one notice a line.
type Said = Vec<(NoticeKind, String)>;

/// A change to the file's text: the new text, and what to say it did.
type Edit<'a> = &'a dyn Fn(&str) -> Result<(String, String), String>;

/// Give `character`'s session its triggers from the file under `dir`, and
/// say what was left out. No file is no triggers, and nothing is said.
pub(crate) fn open(handle: &SessionHandle, dir: &Path, character: &str) {
    for (kind, text) in loaded(reload(handle, dir, character), false) {
        handle.say(Notice::line(kind, format!("Triggers: {text}")));
    }
}

/// Register `;trigger` on `handle`'s command line, for `character`, over
/// the file under `dir`.
pub(crate) fn command(
    handle: &SessionHandle,
    commands: &Commands,
    dir: PathBuf,
    character: String,
) {
    let told = handle.clone();
    commands.trigger(Arc::new(move |line: &str| {
        let parsed = words::parse(line)?;
        for (kind, text) in answer(parsed, &told, &dir, &character) {
            told.say(Notice::line(kind, format!("Triggers: {text}")));
        }
        Some(Claimed::Done)
    }));
}

/// Read the file and give `character`'s session its triggers: how many are
/// on, and what was left out.
fn reload(
    handle: &SessionHandle,
    dir: &Path,
    character: &str,
) -> Result<(usize, Vec<Refused>), String> {
    let loaded = triggers::load(dir)?;
    let mine = loaded.triggers.for_character(character);
    let count = mine.len();
    handle.set_triggers(Matcher::new(mine)?);
    Ok((count, loaded.refused))
}

/// What to say of a reload: each refusal, and how many are on -- always
/// when `asked`, otherwise only when any are.
fn loaded(reloaded: Result<(usize, Vec<Refused>), String>, asked: bool) -> Said {
    let (count, refused) = match reloaded {
        Ok(reloaded) => reloaded,
        Err(why) => return vec![(NoticeKind::Warn, format!("{why}. None are on."))],
    };
    let mut said: Said = refused
        .iter()
        .map(|refused| (NoticeKind::Warn, format!("{refused} It is left out.")))
        .collect();
    if asked || count > 0 {
        said.push((NoticeKind::Info, format!("{} on.", counted(count))));
    }
    said
}

fn counted(count: usize) -> String {
    let noun = if count == 1 { "trigger" } else { "triggers" };
    format!("{count} {noun}")
}

/// Do `parsed`, and say what became of it.
fn answer(
    parsed: Result<Command, String>,
    handle: &SessionHandle,
    dir: &Path,
    character: &str,
) -> Said {
    let info = |lines: Vec<String>| lines.into_iter().map(|l| (NoticeKind::Info, l)).collect();
    let command = match parsed {
        Ok(command) => command,
        Err(why) => return vec![(NoticeKind::Error, why)],
    };
    let change = |edit: Edit<'_>| change(handle, dir, character, edit);
    match command {
        Command::Help => info(HELP.iter().map(|line| (*line).to_owned()).collect()),
        Command::List => list(dir),
        Command::Show(name) => match file(dir).and_then(|text| edit::show(&text, &name)) {
            Ok(lines) => info(
                lines
                    .iter()
                    .map(|line| format!("`{name}` {line}"))
                    .collect(),
            ),
            Err(why) => vec![(NoticeKind::Error, why)],
        },
        Command::Test(words) => info(explain::explain(&handle.triggers(), &words)),
        Command::Reload => loaded(reload(handle, dir, character), true),
        Command::Add { name, words } => change(&|text| {
            let text = edit::add(text, &name, &words)?;
            Ok((text, format!("`{name}` added, making \"{words}\" bold")))
        }),
        Command::Set { name, key, value } => change(&|text| {
            let value = typed(&value);
            let shown = value.to_string();
            let (text, old) = edit::set(text, &name, &key, value)?;
            let was = old.map_or_else(|| "unset".to_owned(), |old| old.to_string());
            Ok((text, format!("`{name}` {key} = {shown} (was {was})")))
        }),
        Command::Unset { name, key } => change(&|text| {
            Ok((
                edit::unset(text, &name, &key)?,
                format!("`{name}` {key} unset"),
            ))
        }),
        Command::Remove(name) => {
            change(&|text| Ok((edit::remove(text, &name)?, format!("`{name}` removed"))))
        }
        Command::Switch { on, target } => {
            let state = if on { "on" } else { "off" };
            let (which, what) = match &target {
                Target::Trigger(name) => (Switch::Trigger(name), format!("`{name}`")),
                Target::Category(category) => {
                    (Switch::Category(category), format!("category {category}"))
                }
                Target::Every(kind) => (Switch::Every(kind), format!("every {kind}")),
            };
            change(&|text| Ok((edit::switch(text, which, on)?, format!("{what} {state}"))))
        }
    }
}

/// Make `edit` to the file, write it, and give this character the result.
/// A change the file cannot use is refused by `edit`, and nothing is
/// written.
fn change(handle: &SessionHandle, dir: &Path, character: &str, edit: Edit<'_>) -> Said {
    let changed = file(dir).and_then(|old| edit(&old));
    let (text, done) = match changed {
        Ok(changed) => changed,
        Err(why) => return vec![(NoticeKind::Error, format!("not changed: {why}"))],
    };
    if let Err(why) = write(dir, &text) {
        return vec![(NoticeKind::Error, format!("not saved: {why}"))];
    }
    match reload(handle, dir, character) {
        Ok((count, _)) => vec![(
            NoticeKind::Info,
            format!(
                "{done}. {} on for {character}; other characters take it at `trigger reload`.",
                counted(count)
            ),
        )],
        Err(why) => vec![
            (NoticeKind::Info, format!("{done}.")),
            (NoticeKind::Warn, format!("{why}. None are on.")),
        ],
    }
}

/// `;trigger list`: every trigger, a line per category.
fn list(dir: &Path) -> Said {
    let listed = match file(dir).and_then(|text| edit::list(&text)) {
        Ok(listed) => listed,
        Err(why) => return vec![(NoticeKind::Error, why)],
    };
    if listed.is_empty() {
        return vec![(
            NoticeKind::Info,
            "none yet; `trigger add <name> <words>` makes one.".to_owned(),
        )];
    }
    let mut said: Said = Vec::new();
    let mut at = 0;
    while let Some(first) = listed.get(at) {
        let category = &first.category;
        let names: Vec<String> = listed[at..]
            .iter()
            .take_while(|row| row.category == *category)
            .map(|row| match (&row.refused, row.enabled) {
                (Some(_), _) => format!("{} (refused)", row.name),
                (None, false) => format!("{} (off)", row.name),
                (None, true) => row.name.clone(),
            })
            .collect();
        at += names.len();
        let heading = if category.is_empty() {
            "(no category)"
        } else {
            category
        };
        said.push((NoticeKind::Info, format!("{heading}: {}", names.join(", "))));
    }
    said
}

/// The file's text; empty when there is none yet.
fn file(dir: &Path) -> Result<String, String> {
    let path = triggers::path(dir);
    match fs::read_to_string(&path) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(format!("cannot read {}: {e}", path.display())),
    }
}

/// Write `text` whole or not at all: to a file beside the triggers file,
/// then over it.
fn write(dir: &Path, text: &str) -> Result<(), String> {
    let path = triggers::path(dir);
    let beside = path.with_extension("toml.new");
    fs::create_dir_all(dir)
        .and_then(|()| fs::write(&beside, text))
        .and_then(|()| fs::rename(&beside, &path))
        .map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests;
