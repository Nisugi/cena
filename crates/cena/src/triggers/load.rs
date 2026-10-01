//! Reading the triggers file into a character's session, and what to say
//! of it. Moved down out of `triggers.rs` when the held sends came (`plan/45`
//! Stage 5), rather than grow a file at its cap.

use std::path::Path;

use cena_behavior::triggers::{self, Refused, sound};
use cena_session::trigger::Matcher;
use cena_session::{NoticeKind, SessionHandle};

use super::Said;

/// A reload: what is on, and what to say of the rest.
pub(super) struct Reload {
    /// How many triggers are on for the character.
    pub(super) count: usize,
    /// Triggers the file refused.
    pub(super) refused: Vec<Refused>,
    /// Triggers whose send waits for the player's approval.
    pub(super) held: Vec<Refused>,
    /// The sounds that will not play, said: refused as paths, or found
    /// nowhere.
    pub(super) unfound: Vec<String>,
}

/// Read the file and give `character`'s session its triggers.
pub(super) fn reload(
    handle: &SessionHandle,
    dir: &Path,
    character: &str,
) -> Result<Reload, String> {
    let loaded = triggers::load(dir)?;
    let mine = loaded.triggers.for_character(character);
    let count = mine.len();
    let unfound = unplayable(
        &crate::attention::sounds_dir(dir),
        mine.iter()
            .filter_map(|trigger| trigger.rule.sound.clone())
            .collect(),
    );
    handle.set_triggers(Matcher::new(mine)?);
    Ok(Reload {
        count,
        refused: loaded.refused,
        held: loaded.held,
        unfound,
    })
}

/// What to say of the `named` sounds that will not play from `sounds`: each
/// that is not a file name, and why, before anything is looked for (a path
/// is never followed: `sound::refused`); then those found nowhere, in one
/// line.
pub(super) fn unplayable(sounds: &Path, mut named: Vec<String>) -> Vec<String> {
    named.sort();
    named.dedup();
    let (refused, named): (Vec<String>, Vec<String>) = named
        .into_iter()
        .partition(|sound| sound::refused(sound).is_some());
    let mut said: Vec<String> = refused
        .iter()
        .map(|written| {
            let why = sound::refused(written).unwrap_or_default();
            format!(
                "The sound `{written}` is not played: it {why}. Set it to its file name, `{}`, \
                 and put the file in {}.",
                sound::file_name(written),
                sounds.display()
            )
        })
        .collect();
    let missing: Vec<String> = named
        .into_iter()
        .filter(|sound| crate::attention::found(sounds, sound).is_none())
        .collect();
    if !missing.is_empty() {
        said.push(format!(
            "{} not found: {}. Put {} in {}.",
            counted_as(missing.len(), "sound"),
            missing
                .iter()
                .map(|sound| format!("`{sound}`"))
                .collect::<Vec<_>>()
                .join(", "),
            if missing.len() == 1 { "it" } else { "them" },
            sounds.display()
        ));
    }
    said
}

/// What stays on when the file cannot be read: the triggers read before,
/// which `handle`'s session still answers with. A file that will not read
/// changes nothing (the crate review of 2026-09-28, R4: this said "None are
/// on" while they, sending ones too, answered on).
pub(super) fn still_on(handle: &SessionHandle) -> String {
    match handle.triggers().triggers().len() {
        0 => "None are on.".to_owned(),
        1 => "The 1 trigger read before stays on.".to_owned(),
        count => format!("The {count} triggers read before stay on."),
    }
}

/// What to say of a reload: each refusal, each send held, the sounds not
/// found, and how many are on -- always when `asked`, otherwise only when
/// any are. A file that will not read says what stays on (`still_on`).
pub(super) fn loaded(
    handle: &SessionHandle,
    reloaded: Result<Reload, String>,
    asked: bool,
) -> Said {
    let reload = match reloaded {
        Ok(reload) => reload,
        Err(why) => return vec![(NoticeKind::Warn, format!("{why}. {}", still_on(handle)))],
    };
    let mut said: Said = reload
        .refused
        .iter()
        .map(|refused| (NoticeKind::Warn, format!("{refused} It is left out.")))
        .collect();
    said.extend(
        reload
            .held
            .iter()
            .map(|held| (NoticeKind::Info, format!("{held}."))),
    );
    said.extend(
        reload
            .unfound
            .into_iter()
            .map(|unfound| (NoticeKind::Warn, unfound)),
    );
    if asked || reload.count > 0 {
        said.push((NoticeKind::Info, format!("{} on.", counted(reload.count))));
    }
    said
}

/// `count` of `noun`, the noun made plural by an `s`.
pub(super) fn counted_as(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// `count` triggers.
pub(super) fn counted(count: usize) -> String {
    counted_as(count, "trigger")
}
