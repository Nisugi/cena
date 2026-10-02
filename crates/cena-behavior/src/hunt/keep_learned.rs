//! What looting learned, written into the character's loot profile: eloot
//! saves its profile each time it learns a bag that closes itself, a thing
//! that crumbles, one it cannot hold, or a creature it cannot skin
//! (`crate::loot::learned`), so the next run knows it. Moved out of
//! `desk.rs` at its cap.

use std::path::Path;

use cena_session::{Notice, NoticeKind, SessionHandle};

use crate::loot::{self, Learned};

/// Write `learned` into the loot profile at `file`, with no other change to
/// the profile between its reading and its writing: a player may be changing
/// it from the menu (the crate review of 2026-09-28, R5). Said either way.
pub(super) fn remember(handle: &SessionHandle, file: Option<&Path>, learned: &Learned) {
    let say = |kind, text: String| handle.say(Notice::line(kind, format!("Loot: {text}")));
    let Some(file) = file else { return };
    let saved = cena_session::store::changing(file, || {
        std::fs::read_to_string(file)
            .map_err(|e| e.to_string())
            .and_then(|text| loot::remember(&text, learned))
            .and_then(|written| match written {
                Some(text) => crate::settings::save(file, &text).map_err(|e| e.to_string()),
                None => Ok(()),
            })
    });
    for line in learned.lines() {
        match &saved {
            Ok(()) => say(
                NoticeKind::Info,
                format!("{line} The loot profile remembers."),
            ),
            Err(why) => say(
                NoticeKind::Warn,
                format!("{line} The loot profile could not be updated -- {why}."),
            ),
        }
    }
}
