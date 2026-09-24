//! The loot planner: the next command, given the state and what was
//! learned so far. Pure, as the hunt engine is (`plan/30` §3).
//!
//! eloot's cycle (`ELoot.loot`, `eloot.lic:2483-2506`) in order: the stance,
//! each corpse searched, then the floor. The floor goes by one `loot room`
//! when everything on it is wanted, since the game sorts by its own STOW
//! LIST; item by item when wanted and unwanted are mixed, `loot #id` for the
//! kinds the game stows itself and a drag into the right bag for the rest
//! (`plan/31` §2b). Each reply is fed back as an [`Outcome`], and what it
//! teaches -- a bag full, a bag that closes itself, a name that crumbles --
//! is kept in [`Memory`] for the rest of the hunt.
//!
//! Three-valued where the model is: a stow list never taught is asked for,
//! not guessed.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use cena_session::containers::StowSlot;
use cena_session::gameobj::ObjectTypes;
use cena_session::{GameState, RoomItem};

use super::outcome::Outcome;
use super::profile::LootProfile;
use super::worth::{Verdict, lootable_by_verb, stow_slot, verdict};
use crate::stance::{self, Want};

/// How many times a corpse is searched before it is given up on
/// (`eloot.lic:5670`, `3.times`).
const SEARCH_TRIES: u8 = 3;
/// How many times an item is dragged before it is given up on
/// (`eloot.lic:4059`, `5.times`).
const DRAG_TRIES: u8 = 5;

/// What the planner learned that outlasts one corpse: eloot's `sacks_full`,
/// `auto_close` and `crumbly`, kept for the hunt.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Memory {
    /// Bags that said `won't fit`, by id.
    pub full: BTreeSet<String>,
    /// Bags that close themselves, by id: opened before a drag.
    pub autoclosers: BTreeSet<String>,
    /// Names that crumbled when stowed.
    pub crumbly: BTreeSet<String>,
    /// Names this character could not hold.
    pub unlootable: BTreeSet<String>,
}

/// One command for the driver to send, or the end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// A stance command (`loot_defensive`).
    Stance(String),
    /// Ask the game for a fact the planner needs: `stow list`.
    Ask(&'static str),
    /// `loot #id` on a corpse.
    Search(i64),
    /// `loot room`: everything left on the floor is wanted.
    LootRoom,
    /// `loot #id` on one thing the game stows itself.
    LootItem(String),
    /// `open #bag`: the bag closes itself.
    Open(String),
    /// Drag one thing into one bag.
    Drag {
        /// The item's id.
        item: String,
        /// The bag's id.
        bag: String,
    },
    /// Nothing more to do here.
    Done(Left),
}

/// How the looting ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Left {
    /// Everything wanted is stowed.
    Nothing,
    /// Something wanted could go in no bag: a reason to rest (author:
    /// *"too much loot"*).
    BagsFull,
    /// A box stayed in hand that no bag would take: a reason to rest
    /// (author: *"we don't want to drop it, so we head in to rest"*).
    BoxInHand,
}

/// The planner for one visit to one room.
#[derive(Clone, Debug)]
pub struct Planner {
    profile: LootProfile,
    memory: Memory,
    corpses: VecDeque<i64>,
    tries: BTreeMap<String, u8>,
    /// Items given up on this visit, by id.
    skipped: BTreeSet<String>,
    /// Bags opened this visit, so `Closed` is not answered twice.
    opened: BTreeSet<String>,
    room_looted: bool,
    last: Option<Step>,
    bags_full: bool,
}

impl Planner {
    /// A planner for the corpses here, with what earlier rooms taught.
    #[must_use]
    pub fn new(profile: LootProfile, memory: Memory, corpses: &[i64]) -> Self {
        let mut memory = memory;
        memory.crumbly.extend(profile.crumbly.iter().cloned());
        memory.unlootable.extend(profile.unlootable.iter().cloned());
        Planner {
            profile,
            memory,
            corpses: corpses.iter().copied().collect(),
            tries: BTreeMap::new(),
            skipped: BTreeSet::new(),
            opened: BTreeSet::new(),
            room_looted: false,
            last: None,
            bags_full: false,
        }
    }

    /// What was learned, for the next room.
    #[must_use]
    pub fn memory(&self) -> &Memory {
        &self.memory
    }

    /// The next command.
    pub fn next(&mut self, state: &GameState) -> Step {
        let step = self.decide(state);
        self.last = Some(step.clone());
        step
    }

    fn decide(&mut self, state: &GameState) -> Step {
        if self.profile.defensive
            && let Ok(want) = Want::parse("defensive")
            && let Some(line) = stance::command(want, state)
        {
            return Step::Stance(line);
        }
        if let Some(corpse) = self.corpses.front().copied() {
            let key = corpse.to_string();
            if *self.tries.get(&key).unwrap_or(&0) >= SEARCH_TRIES {
                self.corpses.pop_front();
                return self.decide(state);
            }
            *self.tries.entry(key).or_insert(0) += 1;
            return Step::Search(corpse);
        }
        if self.bags_full {
            return Step::Done(Left::BagsFull);
        }
        let (wanted, unwanted): (Vec<(&RoomItem, ObjectTypes)>, usize) = self.split(state);
        if wanted.is_empty() {
            return Step::Done(Self::left(state));
        }
        if unwanted == 0 && !self.room_looted {
            self.room_looted = true;
            return Step::LootRoom;
        }
        if !state.containers.stow_checked() {
            return Step::Ask("stow list");
        }
        for (item, types) in wanted {
            if *self.tries.get(&item.id).unwrap_or(&0) >= DRAG_TRIES {
                self.skipped.insert(item.id.clone());
                continue;
            }
            *self.tries.entry(item.id.clone()).or_insert(0) += 1;
            if lootable_by_verb(&types) && self.bag_for(state, &types).is_some() {
                return Step::LootItem(item.id.clone());
            }
            let Some(bag) = self.bag_for(state, &types) else {
                self.bags_full = true;
                return Step::Done(Left::BagsFull);
            };
            if self.memory.autoclosers.contains(&bag) && !self.opened.contains(&bag) {
                self.opened.insert(bag.clone());
                return Step::Open(bag);
            }
            return Step::Drag {
                item: item.id.clone(),
                bag,
            };
        }
        Step::Done(Self::left(state))
    }

    /// The floor split into wanted things with their kinds, and how many
    /// were left for a reason.
    fn split<'a>(&self, state: &'a GameState) -> (Vec<(&'a RoomItem, ObjectTypes)>, usize) {
        let mut wanted = Vec::new();
        let mut unwanted = 0;
        for item in &state.room.objects {
            if self.skipped.contains(&item.id)
                || self.memory.crumbly.contains(&item.text)
                || self.memory.unlootable.contains(&item.text)
            {
                unwanted += 1;
                continue;
            }
            match verdict(item, &self.profile) {
                Verdict::Take(types) => wanted.push((item, types)),
                Verdict::Leave(_) => unwanted += 1,
            }
        }
        (wanted, unwanted)
    }

    /// The bag for things of these kinds: the stow list's slot, else the
    /// default, else the disk when the profile uses it; none that is known
    /// full.
    fn bag_for(&self, state: &GameState, types: &ObjectTypes) -> Option<String> {
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

    /// How it ends when nothing wanted is left: a box in hand is a reason
    /// to rest.
    fn left(state: &GameState) -> Left {
        let box_in_hand = [&state.right_hand, &state.left_hand].iter().any(|hand| {
            hand.noun()
                .zip(hand.name())
                .is_some_and(|(noun, name)| cena_session::gameobj::classify(noun, name).is("box"))
        });
        if box_in_hand {
            Left::BoxInHand
        } else {
            Left::Nothing
        }
    }

    /// What the game said to the last step.
    pub fn outcome(&mut self, outcome: &Outcome) {
        let Some(last) = self.last.clone() else {
            return;
        };
        match (last, outcome) {
            (Step::Search(corpse), Outcome::Searched | Outcome::NotFound) => {
                self.corpses.retain(|c| *c != corpse);
            }
            (Step::LootRoom, Outcome::TooMuch) => {
                // What landed in the hands is dragged item by item; the
                // room is looted again once they are away.
                self.room_looted = false;
            }
            (Step::Drag { bag, .. } | Step::LootItem(bag), Outcome::WontFit) => {
                self.memory.full.insert(bag);
            }
            (Step::Drag { bag, .. }, Outcome::Closed) => {
                self.memory.autoclosers.insert(bag);
            }
            (
                Step::Drag { item, .. } | Step::LootItem(item),
                Outcome::Crumbled | Outcome::NotYours | Outcome::Unlootable | Outcome::NotFound,
            ) => {
                self.skipped.insert(item);
            }
            _ => {}
        }
    }

    /// A name learned crumbly or unlootable, by the item the last step
    /// touched. The driver calls this with the item's name when the outcome
    /// was [`Outcome::Crumbled`] or [`Outcome::Unlootable`].
    pub fn learn(&mut self, outcome: &Outcome, name: &str) {
        match outcome {
            Outcome::Crumbled => {
                self.memory.crumbly.insert(name.to_owned());
            }
            Outcome::Unlootable => {
                self.memory.unlootable.insert(name.to_owned());
            }
            _ => {}
        }
    }
}

/// This character's own disk on the floor, by id: a `disk` whose name
/// begins with the character's name.
fn own_disk(state: &GameState) -> Option<String> {
    let name = state.character.name.as_deref()?;
    state
        .room
        .objects
        .iter()
        .find(|item| item.noun == "disk" && item.text.starts_with(name))
        .map(|item| item.id.clone())
}
