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
//! | `skills` | `character.skills`: the 46 by name, and the circles by theirs | filled |
//! | `known_spells`, `affordable_spells` | [`knows`](super::knows): Lich's `known?` and `affordable?` | filled |
//! | `society`, `society_rank`, `citizenship` | `character.standing` | filled |
//! | `worn`, `worn_nouns` | the inventory snapshot: `worn` on the `player` | filled |
//! | `urchin_access`, `day_pass:…` | pre-flight asks the game (`drive::preflight`), and the driver adds them | filled, by the driver |
//! | `premium_account`, `platinum`, `hunting`, `mounted`, `own_disk_here`, `leading_group` | nothing says yet | **not yet** |
//!
//! "Not yet" is safe and is not free: an exit priced on a fact that is still
//! unknown is impassable, so the walker goes round it. Each row filled in
//! opens exits; none of them can send the walker somewhere it cannot go.

use std::collections::{BTreeMap, HashMap, HashSet};

use cena_map::Walker;
use cena_session::{GameState, InventoryItem, SkillKind};

use super::knows::{affordable_spells, circle_ranks, known_spells, skills_listed};

/// What the character's travel file holds (`plan/24` §5): the profile's
/// settings, and what earlier crossings wrote down.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TravelNotes {
    /// `ice_mode` -> `auto`, `use_urchins` -> `true`, `key_sack` -> `cloak`.
    pub settings: HashMap<String, String>,
    /// `duskruin_origin` -> `228`.
    pub memories: HashMap<String, String>,
    /// go2's custom targets: `home` -> `[228]`. Ordered, because a prefix may
    /// fit several and the one chosen must be the same every time.
    pub targets: BTreeMap<String, Vec<u32>>,
    /// Where the character was last known to be: a tie-breaker, not a fact
    /// (`travel_store::TravelFile::last_room`).
    pub last_room: Option<u32>,
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
    let level = state
        .character
        .experience
        .level
        .as_deref()
        .and_then(level_in);
    let listed = &state.character.skills;
    // No skill ever listed is "not told", so an untrained one cannot be zero.
    let skills = skills_listed(state).then(|| {
        SkillKind::ALL
            .into_iter()
            .map(|kind| {
                let ranks = listed.get(kind).and_then(|skill| skill.ranks).unwrap_or(0);
                (kind.display_name().to_lowercase(), u32::from(ranks))
            })
            .chain(circle_ranks(state))
            .collect()
    });
    let known = known_spells(state, level);
    let standing = &state.character.standing;
    // An inventory never sent is not an empty one.
    let inventory = &state.inventory_snapshot;
    let worn: Option<Vec<&InventoryItem>> = (!inventory.is_empty()).then(|| {
        inventory
            .on_person()
            .filter(|item| item.relation == "worn")
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
        level,
        posture,
        exits: room.exits.clone(),
        // The room's things are known once the room is: an empty list is a
        // room with nothing in it, which is an answer.
        sees: room.id.is_some().then_some(sees),
        encumbrance: state.character.encumbrance_percent,
        active_spells,
        skills,
        affordable_spells: known
            .as_ref()
            .and_then(|known| affordable_spells(state, known)),
        known_spells: known,
        // Told, and in none: the empty name, which no question asks for.
        society: standing
            .society
            .map(|is| is.map_or_else(String::new, |society| society.as_str().to_owned())),
        society_rank: standing.society_rank.map(u32::from),
        citizenship: standing.citizenship.clone().map(Option::unwrap_or_default),
        worn: worn.as_ref().map(|worn| names_of(worn)),
        worn_nouns: worn
            .as_ref()
            .map(|worn| worn.iter().map(|item| item.noun.clone()).collect()),
        ..Walker::default()
    }
}

/// Names as Lich's `GameObj` has them, which is how the map asks: no article.
fn names_of(worn: &[&InventoryItem]) -> HashSet<String> {
    worn.iter()
        .map(|item| {
            format!("{} {}", item.adjective, item.noun)
                .trim()
                .to_owned()
        })
        .collect()
}

/// The number in `Level 100`, which the model keeps verbatim.
fn level_in(label: &str) -> Option<u32> {
    label.rsplit(' ').next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use cena_map::Cond;
    use cena_session::spells::spell;
    use cena_session::{SkillLine, Society, Vital};

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

    #[test]
    fn skills_and_circles_are_asked_for_by_the_names_the_map_uses() {
        let mut state = GameState::default();
        let nothing = walker_from(&state, &TravelNotes::default(), 0);
        assert_eq!(Cond::SkillUnder("climbing".into(), 10).ask(&nothing), None);

        for line in [
            "  Climbing...........................|     120      30",
            "  Major Elemental....................|              25",
        ] {
            let line = SkillLine::classify(line).unwrap();
            state.character.skills.apply(&line, false);
        }
        let walker = walker_from(&state, &TravelNotes::default(), 0);
        let skills = walker.skills.as_ref().unwrap();
        assert_eq!(skills.get("climbing"), Some(&30));
        assert_eq!(skills.get("major elemental"), Some(&25));
        // Listed, and not in the list: untrained, which is an answer.
        assert_eq!(skills.get("swimming"), Some(&0));
    }

    #[test]
    fn a_spell_is_known_by_ranks_capped_at_level_and_a_sigil_by_society() {
        let mut state = GameState::default();
        state.character.experience.level = Some("Level 10".into());
        let line = SkillLine::classify("  Major Elemental....................|              25");
        state.character.skills.apply(&line.unwrap(), false);
        state.character.standing.society = Some(Some(Society::GuardiansOfSunfist));
        state.character.standing.society_rank = Some(4);

        let known = walker_from(&state, &TravelNotes::default(), 0)
            .known_spells
            .unwrap();
        // 504 and 511: 25 ranks, but level 10 caps them at ten.
        assert!(known.contains(&spell(504).unwrap().name));
        assert!(!known.contains(&spell(511).unwrap().name));
        // 9704 at rank 4, and 9705 is a rank away. 9802 is WITHIN rank 4 and
        // is another society's: a 9805 here would be refused on rank alone,
        // and never reach the question of whose it is.
        assert!(known.contains("Sigil of Resolve"));
        assert!(!known.contains(&spell(9705).unwrap().name));
        assert!(!known.contains(&spell(9802).unwrap().name));
    }

    #[test]
    fn a_known_spell_is_affordable_only_with_the_mana() {
        let mut state = GameState::default();
        state.character.experience.level = Some("Level 50".into());
        let line = SkillLine::classify("  Major Elemental....................|              25");
        state.character.skills.apply(&line.unwrap(), false);
        let disk = spell(511).unwrap();
        let unpaid = walker_from(&state, &TravelNotes::default(), 0);
        assert!(unpaid.known_spells.unwrap().contains(&disk.name));
        assert_eq!(unpaid.affordable_spells, None, "no gauge seen yet");

        let gauge = |current| Vital {
            percent: 0,
            current: Some(current),
            max: Some(100),
        };
        for id in ["stamina", "spirit"] {
            state.vitals.insert(id.into(), gauge(100));
        }
        state.vitals.insert("mana".into(), gauge(1));
        let poor = walker_from(&state, &TravelNotes::default(), 0);
        assert!(!poor.affordable_spells.unwrap().contains(&disk.name));
        state.vitals.insert("mana".into(), gauge(100));
        let rich = walker_from(&state, &TravelNotes::default(), 0);
        assert!(rich.affordable_spells.unwrap().contains(&disk.name));
    }

    #[test]
    fn in_no_society_is_an_answer_and_not_told_is_not() {
        let mut state = GameState::default();
        let asked = Cond::Society("Order of Voln".into());
        assert_eq!(
            asked.ask(&walker_from(&state, &TravelNotes::default(), 0)),
            None
        );
        state.character.standing.society = Some(None);
        state.character.standing.citizenship = Some(Some("Wehnimer's Landing".into()));
        let walker = walker_from(&state, &TravelNotes::default(), 0);
        assert_eq!(asked.ask(&walker), Some(false));
        assert!(Cond::Citizenship("Wehnimer's Landing".into()).holds(&walker));
    }

    #[test]
    fn worn_is_what_is_worn_on_the_player_and_nothing_else() {
        let mut state = GameState::default();
        let asked = Cond::WearingNoun("keyring".into());
        let unseen = walker_from(&state, &TravelNotes::default(), 0);
        assert_eq!(asked.ask(&unseen), None, "no inventory yet");

        let item =
            |id: &str, relation: &str, parent: &str, adjective: &str, noun: &str| InventoryItem {
                id: id.into(),
                relation: relation.into(),
                parent: parent.into(),
                article: "a".into(),
                adjective: adjective.into(),
                noun: noun.into(),
                ..InventoryItem::default()
            };
        let items = [
            item("1", "worn", "player", "brass", "keyring"),
            item("2", "worn", "player", "dark", "cloak"),
            // A key in the cloak is carried, and is not worn...
            item("3", "in", "2", "iron", "key"),
            // ...and neither is what is on the player some other way. The
            // author's logs show only `worn` there (`payload.rs`: 1,152 of
            // them), so this row is the rule's, not the wire's.
            item("4", "in", "player", "plain", "gift"),
        ];
        state
            .inventory_snapshot
            .apply_snapshot("7", &items, &[], None);
        let walker = walker_from(&state, &TravelNotes::default(), 0);
        assert!(asked.holds(&walker));
        assert!(Cond::Wearing("brass keyring".into()).holds(&walker));
        assert_eq!(Cond::WearingNoun("key".into()).ask(&walker), Some(false));
        assert_eq!(Cond::WearingNoun("gift".into()).ask(&walker), Some(false));
    }

    /// `spell.rb:597`: spirit must be left with one to spare.
    #[test]
    fn the_last_spirit_is_not_spent() {
        let mut state = GameState::default();
        state.character.experience.level = Some("Level 50".into());
        let line = SkillLine::classify("  Climbing...........................|     120      30");
        state.character.skills.apply(&line.unwrap(), false);
        state.character.standing.society = Some(Some(Society::CouncilOfLight));
        state.character.standing.society_rank = Some(20);
        let healing = spell(9915).unwrap();
        assert_eq!(healing.spirit, Some(2), "the fixture: Sign of Healing");

        let gauge = |current| Vital {
            percent: 0,
            current: Some(current),
            max: Some(100),
        };
        for id in ["mana", "stamina"] {
            state.vitals.insert(id.into(), gauge(100));
        }
        state.vitals.insert("spirit".into(), gauge(2));
        let spent = walker_from(&state, &TravelNotes::default(), 0);
        assert!(spent.known_spells.unwrap().contains(&healing.name));
        assert!(!spent.affordable_spells.unwrap().contains(&healing.name));
        state.vitals.insert("spirit".into(), gauge(3));
        let spare = walker_from(&state, &TravelNotes::default(), 0);
        assert!(spare.affordable_spells.unwrap().contains(&healing.name));
    }
}
