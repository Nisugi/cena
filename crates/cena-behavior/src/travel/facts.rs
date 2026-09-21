//! Filling [`Walker`] from the model (`plan/24` stage 4).
//!
//! `cena-map` never sees the model, so something has to say what the model's
//! facts mean as the map's. That is this file, and it is **pure**: state and
//! a profile in, a `Walker` out, so every line of it is tested without a game.
//!
//! # Unknown stays unknown
//!
//! A `Walker` fact is `None` until the game has said something, and a
//! question about it then has no answer (`cena_map::Cond`). So nothing here
//! defaults: a status the game has never reported is not `false`, and a
//! character whose profession has not arrived is not "no profession". The
//! model already keeps that distinction (`StatusInfo::known`, `Option`
//! everywhere); this file's job is not to lose it.
//!
//! # What is filled, and what is not yet
//!
//! | walker fact | from | |
//! |---|---|---|
//! | `settings`, `memories` | the travel file | filled |
//! | `profession`, `race`, `gender` | `character.identity` | filled |
//! | `level` | `character.experience.level`, the number in `Level 100` | filled |
//! | `posture`; flags `stunned`, `hidden`, `invisible` | `status`, only when reported | filled |
//! | `exits` | the room's compass | filled |
//! | `sees` | the room's objects and creatures | filled |
//! | `encumbrance` | `character.encumbrance_percent` | filled |
//! | `active_spells` | `effects` still running at the game's clock | filled |
//! | `room`, `still_here` | the trip lends these each tick | not here |
//! | `skills`, `known_spells`, `affordable_spells` | need the skill and spell tables joined | **not yet** |
//! | `society`, `society_rank`, `citizenship` | the model does not hold them yet | **not yet** |
//! | `worn`, `worn_nouns` | the inventory model, top level only | **not yet** |
//! | flags the planner works out (`urchin_access`, `day_pass:…`, `hunting`, …) | pre-flight, stage 5 | **not yet** |
//!
//! "Not yet" is safe and is not free: an exit priced on a fact that is still
//! unknown is impassable, so the walker goes round it. Each row filled in
//! opens exits; none of them can send the walker somewhere it cannot go.

use std::collections::HashMap;

use cena_map::Walker;
use cena_session::GameState;

/// What the character's travel file holds (`plan/24` §5): the profile's
/// settings, and what earlier crossings wrote down.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TravelNotes {
    /// `ice_mode` -> `auto`, `use_urchins` -> `true`, `key_sack` -> `cloak`.
    pub settings: HashMap<String, String>,
    /// `duskruin_origin` -> `228`.
    pub memories: HashMap<String, String>,
}

/// The walker's facts, as the model and the travel file have them now.
/// `now_server` is the game's clock, for which spells are still running.
#[must_use]
pub fn walker_from(state: &GameState, notes: &TravelNotes, now_server: u32) -> Walker {
    let identity = &state.character.identity;
    let status = state.status.known();
    let posture = [
        ("standing", status.standing()),
        ("kneeling", status.kneeling()),
        ("sitting", status.sitting()),
        ("prone", status.prone()),
    ]
    .into_iter()
    .find(|(_, reported)| *reported == Some(true))
    .map(|(name, _)| name.to_owned());
    let flags = [
        ("stunned", status.stunned()),
        ("hidden", status.hidden()),
        ("invisible", status.invisible()),
    ]
    .into_iter()
    .filter_map(|(name, reported)| Some((name.to_owned(), reported?)))
    .collect();
    let mut running = state.effects.iter().peekable();
    // No effect ever listed is "not told", not "nothing is up".
    let active_spells = running.peek().is_some().then(|| {
        running
            .filter(|(_, effect)| effect.ends_at.is_none_or(|ends| now_server < ends))
            .map(|(_, effect)| effect.text.clone())
            .collect()
    });
    let room = &state.room;
    let sees = room
        .objects
        .iter()
        .chain(&room.creatures)
        .map(|thing| thing.text.clone())
        .collect();
    Walker {
        settings: notes.settings.clone(),
        memories: notes.memories.clone(),
        flags,
        profession: identity.profession.clone(),
        race: identity.race.clone(),
        gender: identity.gender.clone(),
        level: state
            .character
            .experience
            .level
            .as_deref()
            .and_then(level_in),
        posture,
        exits: room.exits.clone(),
        // The room's things are known once the room is: an empty list is a
        // room with nothing in it, which is an answer.
        sees: room.id.is_some().then_some(sees),
        encumbrance: state.character.encumbrance_percent,
        active_spells,
        ..Walker::default()
    }
}

/// The number in `Level 100`, which the model keeps verbatim.
fn level_in(label: &str) -> Option<u32> {
    label.rsplit(' ').next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use cena_map::Cond;

    use super::*;

    #[test]
    fn a_fresh_state_knows_nothing_and_says_so() {
        let walker = walker_from(&GameState::default(), &TravelNotes::default(), 0);
        assert_eq!(walker, Walker::default());
        // ...so a question about it has no answer, rather than the answer no.
        assert_eq!(Cond::Flag("stunned".into()).ask(&walker), None);
        assert_eq!(Cond::Sees("door".into()).ask(&walker), None);
    }

    #[test]
    fn a_status_the_game_reported_is_a_fact_either_way() {
        let mut state = GameState::default();
        state.status.set("stunned", false);
        state.status.set("kneeling", true);
        let walker = walker_from(&state, &TravelNotes::default(), 0);
        assert_eq!(walker.flags.get("stunned"), Some(&false));
        assert_eq!(walker.flags.get("hidden"), None, "never reported");
        assert_eq!(walker.posture.as_deref(), Some("kneeling"));
    }

    #[test]
    fn the_level_is_the_number_in_the_label() {
        let mut state = GameState::default();
        state.character.experience.level = Some("Level 100".into());
        state.character.identity.profession = Some("Bard".into());
        let walker = walker_from(&state, &TravelNotes::default(), 0);
        assert!(Cond::LevelAtLeast(100).holds(&walker));
        assert!(Cond::Profession("Bard".into()).holds(&walker));
        assert_eq!(level_in("Level"), None);
    }

    #[test]
    fn the_travel_file_is_the_profile_and_the_memories() {
        let mut notes = TravelNotes::default();
        notes.settings.insert("ice_mode".into(), "wait".into());
        notes
            .memories
            .insert("duskruin_origin".into(), "228".into());
        let walker = walker_from(&GameState::default(), &notes, 0);
        assert!(Cond::Setting("ice_mode".into(), "wait".into()).holds(&walker));
        assert!(Cond::Remembered("duskruin_origin".into(), "228".into()).holds(&walker));
    }
}
