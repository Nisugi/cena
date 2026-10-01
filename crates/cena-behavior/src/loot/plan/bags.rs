//! Which bag a thing goes into, opening it first when it is shut, and
//! closing it again after when the profile keeps the bags closed. Moved down
//! from `plan.rs` at its cap.
//!
//! **Where.** A box goes on a disk before any bag: the character's own, then
//! the group's when the profile says so (`single_drag_box`,
//! `eloot.lic:4035-4066`); nothing but a box goes on a disk. Then the stow
//! list's bag for the kind, the default, and the overflow containers in
//! order (`single_drag`, `:3958-4007`). A bag known full is passed by.
//!
//! **Shut.** A bag is shut when the game said so this visit, when it closes
//! itself, or, with `keep_closed`, when its contents are not listed. eloot
//! tells the first two apart by whether the bag's contents are still listed
//! when the game says *It's closed!* (`store_item`, `:4119-4124`; `single_loot`,
//! `:4085-4094`): a bag closed by hand leaves the container list, one that
//! closes itself does not. The second is learned, and the profile told
//! (`super::super::learned`); either is opened before the next try. With
//! `keep_closed`, what was opened is closed again when the visit is over
//! (`close_sell_containers`, `:3737-3746`).

use cena_session::GameState;
use cena_session::containers::StowSlot;
use cena_session::gameobj::ObjectTypes;

use super::super::learned::Learned;
use super::super::worth::{has_word, stow_slot};
use super::{Planner, Step};

impl Planner {
    /// The bag for things of these kinds, none that is known full.
    pub(super) fn bag_for(&self, state: &GameState, types: &ObjectTypes) -> Option<String> {
        let mut candidates: Vec<String> = Vec::new();
        if types.is("box") {
            candidates.extend(disks(state, self.profile.disk, self.profile.disk_group));
        }
        for slot in [stow_slot(types), StowSlot::Default] {
            if let Some(bag) = state.containers.stow(slot) {
                candidates.push(bag.id.clone());
            }
        }
        candidates.extend(named_bags(state, &self.profile.overflow));
        candidates
            .into_iter()
            .find(|bag| !self.memory.full.contains(bag))
    }

    /// `open #bag` before the next thing goes in, when the bag is shut and
    /// has not been opened since.
    pub(super) fn open_first(&mut self, state: &GameState, bag: &str) -> Option<Step> {
        let unlisted = self.profile.keep_closed
            && state.inventory.container(bag).is_none()
            && !is_disk(state, bag);
        let shut = self.closed.contains(bag) || self.closes_itself(state, bag) || unlisted;
        if !shut || self.opened.contains(bag) {
            return None;
        }
        self.opened.insert(bag.to_owned());
        self.closed.remove(bag);
        Some(Step::Open(bag.to_owned()))
    }

    /// With `keep_closed`, the next bag this visit opened, to close again
    /// before it ends; never a disk, and not for a box emptied in the
    /// selling round, whose bags the round closes.
    pub(super) fn reclose(&mut self, state: &GameState) -> Option<String> {
        if !self.profile.keep_closed || self.boxed.is_some() {
            return None;
        }
        let bag = self
            .opened
            .iter()
            .find(|bag| !self.reclosed.contains(*bag) && !is_disk(state, bag))?
            .clone();
        self.reclosed.insert(bag.clone());
        Some(bag)
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

/// The disks a box may go on, by id: the character's own when `own`, and,
/// when `group`, its own and its group's in the room, its own first
/// (`Group.disks`, `single_drag_box`, `eloot.lic:4049-4051`).
pub(crate) fn disks(state: &GameState, own: bool, group: bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if (own || group)
        && let Some(disk) = state
            .character
            .name
            .as_deref()
            .and_then(|me| state.room.disk_of(me))
    {
        out.push(disk.id);
    }
    if group {
        for member in state.group.members() {
            if let Some(disk) = state.room.disk_of(&member.noun)
                && !out.contains(&disk.id)
            {
                out.push(disk.id);
            }
        }
    }
    out
}

/// The containers these names mean, by id and in their order: what is worn
/// or carried whose name has the name's words (`ensure_items`,
/// `eloot.lic:2230`, `/\b#{name}\b/i` over `GameObj.inv`), else a listed
/// container's title. A name nothing answers to is passed by.
pub(crate) fn named_bags(state: &GameState, names: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for name in names {
        let worn = state
            .worn
            .items()
            .and_then(|items| items.iter().find(|item| has_word(&item.text, name)))
            .map(|item| item.id.clone());
        let titled = || {
            state
                .inventory
                .containers()
                .find(|(_, bag)| bag.title.as_deref().is_some_and(|t| has_word(t, name)))
                .map(|(id, _)| id.to_owned())
        };
        if let Some(id) = worn.or_else(titled)
            && !out.contains(&id)
        {
            out.push(id);
        }
    }
    out
}

/// Whether `id` is a disk in the room.
fn is_disk(state: &GameState, id: &str) -> bool {
    state.room.disks().any(|disk| disk.id == id)
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
