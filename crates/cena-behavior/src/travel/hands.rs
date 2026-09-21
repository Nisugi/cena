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
//! # Taking back is one command
//!
//! Lich's `stash.rb:173-193` takes a worn thing back with `remove #id` and
//! anything else with `get #id`, then polls the hands and swaps. This sends
//! the one command and **stops** (author): the character may not be able to
//! hold a shield where it now stands, and a trip that ends by insisting is
//! worse than one that ends by saying what is still stored.

use cena_session::GameState;
use cena_session::containers::{ReadySlot, StoreMode};
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

/// The commands that store what the hands hold, right first, and what each
/// stores. A hand holding something the wire gave no id for is left alone:
/// nothing could take it back.
#[must_use]
pub fn store_commands(state: &GameState) -> Vec<(Stored, &'static str)> {
    [
        (&state.right_hand, "store right"),
        (&state.left_hand, "store left"),
    ]
    .into_iter()
    .filter_map(|(hand, command)| match hand {
        Hand::Holding {
            id: Some(id), name, ..
        } => Some((
            Stored {
                id: id.clone(),
                name: name.clone(),
            },
            command,
        )),
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

    fn holding(id: &str, name: &str) -> Hand {
        Hand::Holding {
            id: Some(id.into()),
            noun: None,
            name: name.into(),
        }
    }

    #[test]
    fn only_what_can_be_taken_back_is_stored() {
        let mut state = GameState::default();
        assert!(store_commands(&state).is_empty(), "unknown hands: nothing");
        state.right_hand = holding("11", "a broadsword");
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
