//! The one triggers file, given to each character's session, and `;trigger`
//! (`plan/45`).
//!
//! The join only: the file is `cena_behavior::triggers`, read, and
//! `cena_behavior::triggers::edit`, changed; what a trigger does to a line
//! is the session's ([`SessionHandle::set_triggers`]). The file is read as a
//! character starts, after every `;trigger` change, and at `;trigger
//! reload`, and each character is given its own list.
//!
//! **A change reaches every running character**: the one it was typed at
//! at once, with all there is to say, and each other one through its
//! triggers task ([`follow`]), with a line naming who changed it.
//!
//! A change is written whole or not at all -- to a file beside it, then over
//! it -- because a player's triggers file may hold years of rules.

mod act;
pub(crate) mod book;
mod explain;
mod follow;
mod form;
pub(crate) mod import;
mod load;
mod words;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{fs, io};

use cena_behavior::settings::typed;
use cena_behavior::triggers;
use cena_behavior::triggers::edit::{self, Switch};
use cena_session::command::claimant::Claimed;
use cena_session::{Notice, NoticeKind, SessionHandle};

use crate::commands::Commands;
use load::{counted, counted_as, loaded, reload};
use words::{Command, HELP, Target};

pub(crate) use follow::{Changes, run};

/// What `;trigger` says, one notice a line.
type Said = Vec<(NoticeKind, String)>;

/// A change to the file's text: the new text, and what to say it did.
type Edit<'a> = &'a dyn Fn(&str) -> Result<(String, String), String>;

/// Give `character`'s session its triggers from the file under `dir`, and
/// say what was left out. No file is no triggers, and nothing is said.
pub(crate) fn open(handle: &SessionHandle, dir: &Path, character: &str) {
    for (kind, text) in loaded(handle, reload(handle, dir, character), false) {
        handle.say(Notice::line(kind, format!("Triggers: {text}")));
    }
}

/// Register `;trigger` on `handle`'s command line, for `character`, over
/// the file under `dir`; a change is told to `others`.
pub(crate) fn command(
    handle: &SessionHandle,
    commands: &Commands,
    dir: PathBuf,
    character: String,
    others: Changes,
) {
    let told = handle.clone();
    commands.trigger(Arc::new(move |line: &str| {
        let parsed = words::parse(line)?;
        for (kind, text) in answer(parsed, &told, &dir, &character, &others) {
            told.say(Notice::line(kind, format!("Triggers: {text}")).answering());
        }
        Some(Claimed::Done)
    }));
}

/// Do `parsed`, and say what became of it.
fn answer(
    parsed: Result<Command, String>,
    handle: &SessionHandle,
    dir: &Path,
    character: &str,
    others: &Changes,
) -> Said {
    let info = |lines: Vec<String>| lines.into_iter().map(|l| (NoticeKind::Info, l)).collect();
    let command = match parsed {
        Ok(command) => command,
        Err(why) => return vec![(NoticeKind::Error, why)],
    };
    let change = |edit: Edit<'_>| change(handle, dir, character, others, edit);
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
        Command::Reload => {
            others.tell(character, "reloaded");
            loaded(handle, reload(handle, dir, character), true)
        }
        Command::Approve(name) => change(&|text| {
            let (text, line) = edit::approve(text, &name)?;
            Ok((text, format!("`{name}` approved: it sends \"{line}\"")))
        }),
        Command::Import(path) => import::import(handle, dir, character, others, Path::new(&path)),
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

/// Make `edit` to the file, write it, give this character the result, and
/// tell `others`. A change the file cannot use is refused by `edit`, and
/// nothing is written. Read, changed and written with no other change to
/// the file between, so two characters changing it at once each keep
/// theirs (`cena_session::store::changing`; the crate review of 2026-09-28, R5).
fn change(
    handle: &SessionHandle,
    dir: &Path,
    character: &str,
    others: &Changes,
    edit: Edit<'_>,
) -> Said {
    let changed = cena_session::store::changing(&triggers::path(dir), || {
        let (text, done) = file(dir)
            .and_then(|old| edit(&old))
            .map_err(|why| (NoticeKind::Error, format!("not changed: {why}")))?;
        write(dir, &text).map_err(|why| (NoticeKind::Error, format!("not saved: {why}")))?;
        Ok(done)
    });
    let done = match changed {
        Ok(done) => done,
        Err(said) => return vec![said],
    };
    others.tell(character, &done);
    match reload(handle, dir, character) {
        Ok(load::Reload { count, unfound, .. }) => {
            let mut said = vec![(
                NoticeKind::Info,
                format!(
                    "{done}. {} on for {character}; every other character reads it again.",
                    counted(count)
                ),
            )];
            said.extend(unfound.map(|unfound| (NoticeKind::Warn, unfound)));
            said
        }
        Err(why) => vec![
            (NoticeKind::Info, format!("{done}.")),
            (
                NoticeKind::Warn,
                format!("{why}. {}", load::still_on(handle)),
            ),
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

/// Write `text` whole or not at all, through the store's one atomic write.
fn write(dir: &Path, text: &str) -> Result<(), String> {
    let path = triggers::path(dir);
    cena_session::store::save_text(dir, &path, text).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests;
