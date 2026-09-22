//! Resolving a noun to a thing: **M2 step 5**, `plan/18` §2e.
//!
//! Split out of `state.rs` under Rule 4.1 (`plan/05:352-353`).
//!
//! # What this is, and what it deliberately is not
//!
//! §2e says *"every interactable carries a stable id... it is what makes 'the
//! doublet' resolvable to a thing"*. The **capture** of `exist=`/`noun=` already
//! happened in steps 2 and 4 -- every `RoomItem` in the room
//! and in every container carries both. What was missing is the lookup, and that
//! is all this adds.
//!
//! It is **not a separate registry**. A second copy of every item, kept in sync
//! with the room and the inventory, would be a cache with two writers and a
//! staleness bug waiting: when a creature dies, `room objs` re-sends without it,
//! and a registry that merely accumulated would keep offering it forever. Reading
//! through to the live collections means a thing is resolvable exactly as long as
//! the game still says it is there.
//!
//! # Why a noun can be ambiguous, and why that is reported rather than guessed
//!
//! MEASURED on `GSIV-Monstr` (2025-09-04): a single `room objs` body carried two
//! creatures, and the author's own capture of 2025-04-18 shows a container
//! holding several items whose nouns repeat across containers. `noun=` is what a
//! command targets, and the game itself disambiguates with ordinals -- so a
//! consumer that silently took the first match would send `attack artificer` at
//! the wrong artificer.
//!
//! [`GameState::resolve_noun`] therefore returns **every** match, in a defined
//! order, and lets the caller decide. A behavior that wants "the only one" checks
//! the length; one that wants "the first in the room" takes `.first()` knowingly.

use super::{GameState, RoomItem};

/// Where a resolved thing was found.
///
/// Carried because *where* changes what a command can do: something in the room
/// can be attacked or picked up, something in a container is already held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Where<'a> {
    /// A bold entry of `room objs`.
    Creature,
    /// A non-bold entry of `room objs`.
    RoomObject,
    /// An entry of `room players`.
    Player,
    /// Inside the named container.
    Container(&'a str),
}

/// One thing a noun resolved to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Found<'a> {
    /// The item itself, borrowed from the live collection it lives in.
    pub item: &'a RoomItem,
    /// Where it was found.
    pub found_in: Where<'a>,
}

impl GameState {
    /// Every thing currently known by this noun.
    ///
    /// Searched in a **defined order** -- creatures, room objects, players, then
    /// containers by id -- so the result is deterministic for criterion 7 and so
    /// `.first()` means something stable to a caller who uses it.
    ///
    /// Creatures come first deliberately: a noun that matches both a creature and
    /// a floor item is overwhelmingly meant as the creature, because that is what
    /// a player is usually acting on.
    ///
    /// **Returns every match rather than the best one.** The game disambiguates
    /// with ordinals, so guessing here would send a command at the wrong target
    /// and report success. See the module docs.
    #[must_use]
    pub fn resolve_noun(&self, noun: &str) -> Vec<Found<'_>> {
        self.resolve_all().filter(|f| f.item.noun == noun).collect()
    }

    /// The thing with this `exist` id, wherever it is.
    ///
    /// The unambiguous lookup: ids are unique where nouns are not, so this
    /// returns at most one. What a consumer uses once it has *chosen* among
    /// [`Self::resolve_noun`]'s answers and wants to check the thing is still
    /// there before acting.
    #[must_use]
    pub fn find_by_id(&self, id: &str) -> Option<Found<'_>> {
        self.resolve_all().find(|f| f.item.id == id)
    }

    /// Every known thing, in the same defined order [`Self::resolve_noun`] uses.
    fn resolve_all(&self) -> impl Iterator<Item = Found<'_>> {
        let creatures = self.room.creatures.iter().map(|item| Found {
            item,
            found_in: Where::Creature,
        });
        let objects = self.room.objects.iter().map(|item| Found {
            item,
            found_in: Where::RoomObject,
        });
        let players = self.room.players.iter().map(|item| Found {
            item,
            found_in: Where::Player,
        });
        let carried = self.inventory.containers().flat_map(|(id, container)| {
            container.items.iter().map(move |item| Found {
                item,
                found_in: Where::Container(id),
            })
        });
        creatures.chain(objects).chain(players).chain(carried)
    }
}
