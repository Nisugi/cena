//! Which bag a thing goes into, and opening it first when it is shut.
//! Moved down from `plan.rs` at its cap.
//!
//! A bag is shut when the game said so this visit, or when it closes itself.
//! eloot tells the two apart by whether the bag's contents are still listed
//! when the game says *It's closed!* (`store_item`, `eloot.lic:4119-4124`;
//! `single_loot`, `:4085-4094`): a bag closed by hand leaves the container
//! list, one that closes itself does not. The second is learned, and the
//! profile told (`super::super::learned`); either is opened before the next
//! try.

use cena_session::GameState;
use cena_session::containers::StowSlot;
use cena_session::gameobj::ObjectTypes;

use super::super::learned::Learned;
use super::super::worth::{has_word, stow_slot};
use super::{Planner, Step};

impl Planner {
    /// The bag for things of these kinds: the stow list's slot, else the
    /// default, else the disk when the profile uses it; none that is known
    /// full.
    pub(super) fn bag_for(&self, state: &GameState, types: &ObjectTypes) -> Option<String> {
        let slot = stow_slot(types);
        let mut candidates: Vec<String> = Vec::new();
        for slot in [slot, StowSlot::Default] {
            if let Some(bag) = state.containers.stow(slot) {
                candidates.push(bag.id.clone());
            }
        }
        if self.profile.disk
            && let Some(disk) = own_disk(state)
        {
            candidates.push(disk);
        }
        candidates
            .into_iter()
            .find(|bag| !self.memory.full.contains(bag))
    }

    /// `open #bag` before the next thing goes in, when the bag is shut and
    /// has not been opened since.
    pub(super) fn open_first(&mut self, state: &GameState, bag: &str) -> Option<Step> {
        let shut = self.closed.contains(bag) || self.closes_itself(state, bag);
        if !shut || self.opened.contains(bag) {
            return None;
        }
        self.opened.insert(bag.to_owned());
        self.closed.remove(bag);
        Some(Step::Open(bag.to_owned()))
    }

    /// Whether a bag closes itself: learned so, or named in the profile's
    /// `autoclose`.
    fn closes_itself(&self, state: &GameState, bag: &str) -> bool {
        self.memory.autoclosers.contains(bag)
            || bag_name(state, bag).is_some_and(|name| {
                self.profile
                    .autoclose
                    .iter()
                    .any(|named| named.eq_ignore_ascii_case(&name) || has_word(&name, named))
            })
    }

    /// The bag the last step was putting something into, which the game
    /// says is closed.
    pub(super) fn shut_bag(&mut self) -> Option<String> {
        match self.last.as_ref()? {
            Step::Drag { bag, .. } => Some(bag.clone()),
            // `loot #id` names the item; the bag is the one it was bound for.
            Step::LootItem(_) => self.into.take(),
            _ => None,
        }
    }

    /// The game said `bag` is closed. Its contents still listed, it closed
    /// itself: learned, and named for the profile when it is not there yet.
    /// Either way it is opened before the next try. Without the state, it
    /// is only shut.
    pub(super) fn closed_on(&mut self, state: Option<&GameState>, bag: &str) {
        let listed = state.is_some_and(|state| state.inventory.container(bag).is_some());
        if listed {
            if self.memory.autoclosers.insert(bag.to_owned())
                && let Some(name) = state.and_then(|state| bag_name(state, bag))
                && !self
                    .profile
                    .autoclose
                    .iter()
                    .any(|named| named.eq_ignore_ascii_case(&name))
            {
                Learned::add(&mut self.learned.autoclose, &name);
            }
        } else {
            self.closed.insert(bag.to_owned());
        }
        self.opened.remove(bag);
    }
}

/// A bag's name: as the stow list names it, else as worn, else its window's
/// title.
pub(super) fn bag_name(state: &GameState, id: &str) -> Option<String> {
    StowSlot::ALL
        .iter()
        .filter_map(|slot| state.containers.stow(*slot))
        .find(|bag| bag.id == id)
        .map(|bag| bag.text.clone())
        .or_else(|| {
            state
                .worn
                .items()?
                .iter()
                .find(|item| item.id == id)
                .map(|item| item.text.clone())
        })
        .or_else(|| state.inventory.container(id)?.title.clone())
}

/// This character's own disk on the floor, by id: a `disk` whose name
/// begins with the character's name.
pub(super) fn own_disk(state: &GameState) -> Option<String> {
    let name = state.character.name.as_deref()?;
    state
        .room
        .objects
        .iter()
        .find(|item| item.noun == "disk" && item.text.starts_with(name))
        .map(|item| item.id.clone())
}
