//! Which spells the walker knows, and which it can pay for now: Lich's
//! `Spell#known?` and `Spell#affordable?` (`lib/common/spell.rb:464-522`,
//! `:591-611`), over the model. **Pure.**
//!
//! # Known
//!
//! A spell's circle is its number less its last two digits, and it is known
//! when the ranks in that circle -- **capped at the character's level** --
//! reach those last two digits. A society's powers (97, 98, 99) go by rank in
//! *that* society. 1700 is known to the five pure professions and nothing
//! else in its circle is known to anyone.
//!
//! # Affordable
//!
//! Enough stamina, mana and spirit for what the table says it costs, and
//! spirit must be left with **one to spare**. Not ported, and said so rather
//! than guessed at: the Council of Light signs that raise the spirit to
//! spare, Mental Acuity (a Monk feat that pays for 1200s in stamina), and the
//! `Overexerted` debuff. Each makes a spell *less* affordable than this says,
//! so the cost of the gap is a cast the game refuses, which the trip already
//! recovers from.

use std::collections::HashSet;

use cena_session::spells::{self, Spell};
use cena_session::{GameState, Society};

/// The spell circles as `skills` prints them, by circle number. Lich's
/// `SPELL_CIRCLE_INDEX_TO_NAME` (`lib/gemstone/armaments.rb:54-72`), less the
/// rows no character trains.
const CIRCLES: [(u16, &str); 12] = [
    (1, "Minor Spiritual"),
    (2, "Major Spiritual"),
    (3, "Cleric"),
    (4, "Minor Elemental"),
    (5, "Major Elemental"),
    (6, "Ranger"),
    (7, "Sorcerer"),
    (9, "Wizard"),
    (10, "Bard"),
    (11, "Empath"),
    (12, "Minor Mental"),
    (16, "Paladin"),
];

/// The professions to whom 1700 is known (`spell.rb:498`).
const PURES: [&str; 5] = ["Wizard", "Cleric", "Empath", "Sorcerer", "Savant"];

/// Whether `skills` has ever been read: a skill row or a circle row.
pub(super) fn skills_listed(state: &GameState) -> bool {
    let skills = &state.character.skills;
    skills.known_count() > 0 || skills.circles().next().is_some()
}

/// The circle names as the map's questions spell them, with the ranks in
/// each: `major elemental` -> 25.
pub(super) fn circle_ranks(state: &GameState) -> impl Iterator<Item = (String, u32)> + '_ {
    let skills = &state.character.skills;
    CIRCLES.iter().map(move |(_, name)| {
        let ranks = skills.circle(name).unwrap_or(0);
        (name.to_lowercase(), u32::from(ranks))
    })
}

/// The ranks that decide whether `spell` is known, or `None` when nothing
/// could make it so.
fn deciding_ranks(state: &GameState, spell: &Spell, level: u32) -> Option<u32> {
    let circle = spell.number / 100;
    let society = |which: Society| {
        let standing = &state.character.standing;
        (standing.society == Some(Some(which))).then(|| standing.society_rank.map_or(0, u32::from))
    };
    match circle {
        97 => society(Society::GuardiansOfSunfist),
        98 => society(Society::OrderOfVoln),
        99 => society(Society::CouncilOfLight),
        _ => {
            let (_, name) = CIRCLES.iter().find(|(number, _)| *number == circle)?;
            let ranks = state.character.skills.circle(name).unwrap_or(0);
            Some(u32::from(ranks).min(level))
        }
    }
}

/// Every spell and society power the character knows, by name. `None` until
/// the skills have been listed: before that, nothing is known *about* them.
pub(super) fn known_spells(state: &GameState, level: Option<u32>) -> Option<HashSet<String>> {
    if !skills_listed(state) {
        return None;
    }
    let level = level?;
    let pure = state
        .character
        .identity
        .profession
        .as_deref()
        .is_some_and(|is| PURES.contains(&is));
    Some(
        spells::all()
            .filter(|spell| {
                if spell.number == 1700 {
                    return pure;
                }
                deciding_ranks(state, spell, level)
                    .is_some_and(|ranks| u32::from(spell.number % 100) <= ranks)
            })
            .map(|spell| spell.name.clone())
            .collect(),
    )
}

/// Of `known`, the ones the character can pay for now. `None` until all three
/// gauges have been seen.
pub(super) fn affordable_spells(
    state: &GameState,
    known: &HashSet<String>,
) -> Option<HashSet<String>> {
    let have = |id: &str| state.vital(id)?.current;
    let (mana, stamina, spirit) = (have("mana")?, have("stamina")?, have("spirit")?);
    Some(
        spells::all()
            .filter(|spell| known.contains(&spell.name))
            .filter(|spell| {
                let costs = |cost: Option<u16>| i32::from(cost.unwrap_or(0));
                let spirit_cost = costs(spell.spirit);
                costs(spell.stamina) <= stamina
                    && costs(spell.mana) <= mana
                    // One to spare: a spell that takes the last spirit kills.
                    && (spirit_cost == 0 || spirit_cost < spirit)
            })
            .map(|spell| spell.name.clone())
            .collect(),
    )
}
