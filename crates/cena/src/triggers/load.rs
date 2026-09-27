//! Reading the triggers file into a character's session, and what to say
//! of it. Moved down out of `triggers.rs` when the held sends came (`plan/45`
//! Stage 5), rather than grow a file at its cap.

use std::path::Path;

use cena_behavior::triggers::{self, Refused};
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
    /// The sounds found nowhere, said.
    pub(super) unfound: Option<String>,
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
    let sounds = crate::attention::sounds_dir(dir);
    let mut missing: Vec<String> = mine
        .iter()
        .filter_map(|trigger| trigger.rule.sound.clone())
        .filter(|sound| crate::attention::found(&sounds, sound).is_none())
        .collect();
    missing.sort();
    missing.dedup();
    let unfound = (!missing.is_empty()).then(|| {
        format!(
            "{} not found: {}. Put {} in {}.",
            counted_as(missing.len(), "sound"),
            missing
                .iter()
                .map(|sound| format!("`{sound}`"))
                .collect::<Vec<_>>()
                .join(", "),
            if missing.len() == 1 { "it" } else { "them" },
            sounds.display()
        )
    });
    handle.set_triggers(Matcher::new(mine)?);
    Ok(Reload {
        count,
        refused: loaded.refused,
        held: loaded.held,
        unfound,
    })
}

/// What to say of a reload: each refusal, each send held, the sounds not
/// found, and how many are on -- always when `asked`, otherwise only when
/// any are.
pub(super) fn loaded(reloaded: Result<Reload, String>, asked: bool) -> Said {
    let reload = match reloaded {
        Ok(reload) => reload,
        Err(why) => return vec![(NoticeKind::Warn, format!("{why}. None are on."))],
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
    said.extend(reload.unfound.map(|unfound| (NoticeKind::Warn, unfound)));
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
