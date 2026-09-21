//! What the driver sends for the deeds the trip cannot spell (`plan/24`
//! stage 4c). **Pure**: the model in, command lines out.
//!
//! # Stored, not stowed
//!
//! The author, 2026-09-21: the word is *stored*, because it is the opposite of
//! *ready*, and the game already knows where each readied thing goes -- a
//! sheath, a container, or **worn**. `store right` asks the game to do what
//! the player set up with `store set`; `stow right` would put a shield in a
//! cloak that its owner wears on the shoulder. Both verbs are in use upstream
//! (`grep -rhoE "(stow|store) (right|left)" reference/scripts`).
//!
//! **And only an armament can be stored** (author, same day): a weapon, a
//! runestaff, a shield. Whatever else is in a hand is stowed.
//!
//! # Taking back is one command
//!
//! Lich's `stash.rb:173-193` takes a worn thing back with `remove #id` and
//! anything else with `get #id`, then polls the hands and swaps. This sends
//! the one command and **stops** (author): the character may not be able to
//! hold a shield where it now stands, and a trip that ends by insisting is
//! worse than one that ends by saying what is still stored.

use cena_session::GameState;
use cena_session::containers::{ReadySlot, StoreMode};
use cena_session::gameobj;
use cena_session::hands::Hand;
use cena_session::spell_named;

/// Something a trip stored, to be taken back when it ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    /// `exist=`. What `get #id` names.
    pub id: String,
    /// For a person: `a steel shield`.
    pub name: String,
}

/// The lowest spell number that is a society's power rather than a spell:
/// those are used by name (`effect-list.xml`'s `cast-proc` for 9704 is
/// `sigil of resolve`), where a spell is `incant`ed by number.
const FIRST_SOCIETY_POWER: u16 = 9700;

/// The nouns Lich takes for a shield (`stash.rb:173`), less the bows it lists
/// beside them: those are weapons, and the type table says so.
const SHIELD_NOUNS: [&str; 10] = [
    "shield",
    "buckler",
    "targe",
    "heater",
    "parma",
    "aegis",
    "scutum",
    "greatshield",
    "mantlet",
    "pavis",
];

/// Whether the game will `store` this: a weapon (a runestaff is one, in the
/// type table) or a shield. **Only armaments can be stored** (author,
/// 2026-09-21); anything else -- a gem, a gift, a lockpick -- is refused by
/// `store` and has to be stowed.
#[must_use]
pub fn is_armament(noun: &str, name: &str) -> bool {
    SHIELD_NOUNS.contains(&noun) || gameobj::classify(noun, name).is("weapon")
}

/// The commands that put away what the hands hold, right first, and what each
/// puts away: `store` for an armament, which sends it where the player set it
/// to go, and `stow` for anything else. A hand holding something the wire gave
/// no id for is left alone: nothing could take it back.
#[must_use]
pub fn store_commands(state: &GameState) -> Vec<(Stored, String)> {
    [(&state.right_hand, "right"), (&state.left_hand, "left")]
        .into_iter()
        .filter_map(|(hand, side)| match hand {
            Hand::Holding {
                id: Some(id),
                noun,
                name,
            } => {
                let armament = noun.as_deref().is_some_and(|noun| is_armament(noun, name));
                let verb = if armament { "store" } else { "stow" };
                Some((
                    Stored {
                        id: id.clone(),
                        name: name.clone(),
                    },
                    format!("{verb} {side}"),
                ))
            }
            _ => None,
        })
        .collect()
}

/// The one command that takes a stored thing back: `remove` when the ready
/// list says its slot is worn when stored, `get` otherwise.
#[must_use]
pub fn take_back(state: &GameState, stored: &Stored) -> String {
    let ready = &state.containers;
    let worn = [
        ReadySlot::Shield,
        ReadySlot::Weapon,
        ReadySlot::SecondaryWeapon,
        ReadySlot::RangedWeapon,
    ]
    .into_iter()
    .any(|slot| {
        ready.ready(slot).is_some_and(|item| item.id == stored.id)
            && ready.store_mode(slot) == Some(StoreMode::WornIfPossible)
    });
    let verb = if worn { "remove" } else { "get" };
    format!("{verb} #{}", stored.id)
}

/// The commands that cast `spell`, at `target` if there is one. `None` for a
/// name the spell table does not have.
#[must_use]
pub fn cast_commands(spell: &str, target: Option<&str>) -> Option<Vec<String>> {
    let found = spell_named(spell)?;
    if found.number >= FIRST_SOCIETY_POWER {
        return Some(vec![found.name.to_lowercase()]);
    }
    Some(match target {
        None => vec![format!("incant {}", found.number)],
        Some(target) => vec![
            format!("prepare {}", found.number),
            format!("cast {target}"),
        ],
    })
}

#[cfg(test)]
mod tests {
    use cena_session::containers::{ContainerEvent, ItemRef};

    use super::*;

    fn holding(id: &str, noun: &str, name: &str) -> Hand {
        Hand::Holding {
            id: Some(id.into()),
            noun: Some(noun.into()),
            name: name.into(),
        }
    }

    #[test]
    fn only_what_can_be_taken_back_is_stored() {
        let mut state = GameState::default();
        assert!(store_commands(&state).is_empty(), "unknown hands: nothing");
        state.right_hand = holding("11", "broadsword", "a broadsword");
        state.left_hand = Hand::Holding {
            id: None,
            noun: None,
            name: "something".into(),
        };
        let stored = store_commands(&state);
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].1, "store right");
        assert_eq!(stored[0].0.id, "11");
    }

    #[test]
    fn an_armament_is_stored_and_anything_else_is_stowed() {
        let mut state = GameState::default();
        state.right_hand = holding("1", "runestaff", "an oak runestaff");
        state.left_hand = holding("2", "shield", "a steel shield");
        let commands: Vec<String> = store_commands(&state).into_iter().map(|c| c.1).collect();
        assert_eq!(commands, ["store right", "store left"]);

        state.right_hand = holding("3", "gift", "a plain gift");
        state.left_hand = holding("4", "lockpick", "a copper lockpick");
        let commands: Vec<String> = store_commands(&state).into_iter().map(|c| c.1).collect();
        assert_eq!(commands, ["stow right", "stow left"]);
    }

    #[test]
    fn a_thing_is_taken_back_with_get_unless_the_game_wears_it() {
        let state = GameState::default();
        let shield = Stored {
            id: "22".into(),
            name: "a steel shield".into(),
        };
        assert_eq!(take_back(&state, &shield), "get #22");
        // The ready list names it as the shield, worn when stored.
        let mut state = state;
        state.containers.apply(&ContainerEvent::ReadySet {
            slot: ReadySlot::Shield,
            item: Some(ItemRef {
                id: "22".into(),
                noun: "shield".into(),
                text: "steel shield".into(),
            }),
            store: Some(StoreMode::WornIfPossible),
        });
        assert_eq!(take_back(&state, &shield), "remove #22");
        // ...and it is the id that decides, not there being a worn shield.
        let sword = Stored {
            id: "11".into(),
            name: "a broadsword".into(),
        };
        assert_eq!(take_back(&state, &sword), "get #11");
    }

    #[test]
    fn a_spell_is_incanted_and_a_society_power_is_named() {
        assert_eq!(
            cast_commands("Sigil of Resolve", None),
            Some(vec!["sigil of resolve".to_owned()])
        );
        let walking = cast_commands("Water Walking", None).unwrap();
        assert_eq!(walking.len(), 1);
        assert!(walking[0].starts_with("incant "));
        let phase = cast_commands("Phase", Some("insignia")).unwrap();
        assert!(phase[0].starts_with("prepare "));
        assert_eq!(phase[1], "cast insignia");
        assert_eq!(cast_commands("No Such Spell", None), None);
    }
}
