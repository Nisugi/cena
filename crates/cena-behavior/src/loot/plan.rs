//! The loot planner: the next command, given the state and what was
//! learned so far. Pure, as the hunt engine is (`plan/30` §3).
//!
//! eloot's cycle (`ELoot.loot`, `eloot.lic:2483-2506`) in order: the stance,
//! each corpse searched, then the floor. The floor goes in eloot's two
//! passes (`loot_specials`, `:5509-5570`, then `loot_regular`, `:5222`):
//! the **specials** first, item by item -- boxes, jewelry, clothing and the
//! other kinds `loot room` is not trusted with, the three uncommon
//! Hinterwilds items it does not take, and a bag a critter dropped, which
//! is opened, looked in and emptied before it is taken itself -- then the
//! rest by one `loot room` when everything left is wanted, since the game
//! sorts by its own STOW LIST; item by item when not, `loot #id` for the
//! kinds the game stows itself and a drag into the right bag for the rest
//! (`plan/31` §2b). Each reply is fed back as an [`Outcome`], and what it
//! teaches -- a bag full, a bag that closes itself, a name that crumbles, a
//! critter's bag already emptied -- is kept in [`Memory`] for the hunt.
//!
//! Three-valued where the model is: a stow list never taught is asked for,
//! not guessed.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use cena_session::containers::StowSlot;
use cena_session::gameobj::ObjectTypes;
use cena_session::{GameState, RoomItem};

use super::outcome::Outcome;
use super::profile::LootProfile;
use super::worth::{Verdict, is_special, lootable_by_verb, stow_slot, verdict};
use crate::stance::{self, Want};

/// How many times a corpse is searched before it is given up on
/// (`eloot.lic:5670`, `3.times`).
const SEARCH_TRIES: u8 = 3;
/// How many times an item is dragged before it is given up on
/// (`eloot.lic:4059`, `5.times`).
const DRAG_TRIES: u8 = 5;
/// Sigil of Determination, cast as Lich casts a Sunfist sigil
/// (`spell.rb`: `incant <num>`; `sunfist.rs`: 9716), when a corpse is
/// *not in any condition* to be searched (`eloot.lic:5679-5685`).
const SIGIL_OF_DETERMINATION: &str = "incant 9716";
/// The sigil's name in the effects list.
const SIGIL_NAME: &str = "Sigil of Determination";

/// What the planner learned that outlasts one corpse: eloot's `sacks_full`,
/// `auto_close`, `crumbly` and `checked_bags`, kept for the hunt.
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
    /// Critters' bags already opened and emptied, by id.
    pub checked_bags: BTreeSet<String>,
}

/// One command for the driver to send, or the end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// A stance command (`loot_defensive`).
    Stance(String),
    /// Ask the game for a fact the planner needs: `stow list`.
    Ask(&'static str),
    /// Cast a spell: the sigil, after a failed search.
    Cast(String),
    /// `loot #id` on a corpse.
    Search(i64),
    /// `open #bag`: a critter's bag, or a bag that closes itself.
    Open(String),
    /// `look in #bag`: a critter's bag, to learn what it holds.
    LookIn(String),
    /// `loot room`: everything left on the floor is wanted.
    LootRoom,
    /// `loot #id` on one thing the game stows itself.
    LootItem(String),
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

/// Where a critter's bag is in being emptied (`bag_loot`, `eloot.lic:4980`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BagPhase {
    Opened,
    Looked,
}

/// Whether Sigil of Determination is wanted for a search that failed on
/// the character's condition, and whether it has been cast this visit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sigil {
    NotNeeded,
    Needed,
    Cast,
}

/// The floor, sorted into eloot's two passes and what is left.
#[derive(Default)]
struct Floor {
    specials: Vec<(RoomItem, ObjectTypes)>,
    regular: Vec<(RoomItem, ObjectTypes)>,
    unwanted: usize,
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
    /// Critters' bags being emptied this visit.
    bags: BTreeMap<String, BagPhase>,
    room_looted: bool,
    /// A search failed for the character's condition; the sigil helps once.
    sigil: Sigil,
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
            bags: BTreeMap::new(),
            room_looted: false,
            sigil: Sigil::NotNeeded,
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
        if self.sigil == Sigil::Needed {
            self.sigil = Sigil::Cast;
            if !sigil_up(state) {
                return Step::Cast(SIGIL_OF_DETERMINATION.to_owned());
            }
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
        let floor = self.split(state);
        if floor.specials.is_empty() && floor.regular.is_empty() {
            return Step::Done(Self::left(state));
        }
        // eloot's first pass: the specials, one by one, a critter's bag
        // emptied before it is taken.
        for (item, types) in &floor.specials {
            if let Some(step) = self.special(state, item, types) {
                return step;
            }
        }
        if floor.specials.is_empty() && floor.unwanted == 0 && !self.room_looted {
            self.room_looted = true;
            return Step::LootRoom;
        }
        if !state.containers.stow_checked() {
            return Step::Ask("stow list");
        }
        for (item, types) in floor.specials.iter().chain(floor.regular.iter()) {
            if let Some(step) = self.take(state, item, types) {
                return step;
            }
        }
        Step::Done(Self::left(state))
    }

    /// A critter's bag is opened, looked in and emptied before it is taken
    /// (`bag_loot`). `None` when the thing is ready to be taken as any
    /// other.
    fn special(&mut self, state: &GameState, item: &RoomItem, types: &ObjectTypes) -> Option<Step> {
        if !types.is("clothing") || self.memory.checked_bags.contains(&item.id) {
            return None;
        }
        match self.bags.get(&item.id) {
            None => {
                self.bags.insert(item.id.clone(), BagPhase::Opened);
                Some(Step::Open(item.id.clone()))
            }
            Some(BagPhase::Opened) => {
                self.bags.insert(item.id.clone(), BagPhase::Looked);
                Some(Step::LookIn(item.id.clone()))
            }
            Some(BagPhase::Looked) => {
                if !state.containers.stow_checked() {
                    return Some(Step::Ask("stow list"));
                }
                let inside: Vec<(RoomItem, ObjectTypes)> = state
                    .inventory
                    .container(&item.id)
                    .map(|bag| bag.items.clone())
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|thing| !self.skipped.contains(&thing.id))
                    .filter_map(|thing| match verdict(&thing, &self.profile) {
                        Verdict::Take(kinds) => Some((thing, kinds)),
                        Verdict::Leave(_) => None,
                    })
                    .collect();
                for (thing, kinds) in &inside {
                    if let Some(step) = self.take(state, thing, kinds) {
                        return Some(step);
                    }
                }
                self.memory.checked_bags.insert(item.id.clone());
                None
            }
        }
    }

    /// Take one thing: by the game's verb where it stows itself, else a
    /// drag into the right bag. `None` when it has been given up on.
    fn take(&mut self, state: &GameState, item: &RoomItem, types: &ObjectTypes) -> Option<Step> {
        if *self.tries.get(&item.id).unwrap_or(&0) >= DRAG_TRIES {
            self.skipped.insert(item.id.clone());
            return None;
        }
        *self.tries.entry(item.id.clone()).or_insert(0) += 1;
        let bag = self.bag_for(state, types);
        if lootable_by_verb(types) && bag.is_some() {
            return Some(Step::LootItem(item.id.clone()));
        }
        let Some(bag) = bag else {
            self.bags_full = true;
            return Some(Step::Done(Left::BagsFull));
        };
        if self.memory.autoclosers.contains(&bag) && !self.opened.contains(&bag) {
            self.opened.insert(bag.clone());
            return Some(Step::Open(bag));
        }
        Some(Step::Drag {
            item: item.id.clone(),
            bag,
        })
    }

    /// The floor split into eloot's two passes and what is left.
    fn split(&self, state: &GameState) -> Floor {
        let mut floor = Floor::default();
        for item in &state.room.objects {
            if self.skipped.contains(&item.id)
                || self.memory.crumbly.contains(&item.text)
                || self.memory.unlootable.contains(&item.text)
            {
                floor.unwanted += 1;
                continue;
            }
            match verdict(item, &self.profile) {
                Verdict::Take(types) if is_special(item, &types) => {
                    floor.specials.push((item.clone(), types));
                }
                Verdict::Take(types) => floor.regular.push((item.clone(), types)),
                Verdict::Leave(_) => floor.unwanted += 1,
            }
        }
        floor
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
            (Step::Search(_), Outcome::NotInCondition) => {
                if self.profile.sigil_on_fail && self.sigil == Sigil::NotNeeded {
                    self.sigil = Sigil::Needed;
                }
            }
            (Step::LootRoom, Outcome::TooMuch) => {
                // What landed in the hands is dragged item by item; the
                // room is looted again once they are away.
                self.room_looted = false;
            }
            (Step::Open(bag), Outcome::NotAContainer | Outcome::NotFound) => {
                // Not a bag after all: nothing to empty, take it as it is.
                self.bags.remove(&bag);
                self.memory.checked_bags.insert(bag);
            }
            (Step::Open(bag), Outcome::Crumbled) => {
                self.bags.remove(&bag);
                self.skipped.insert(bag);
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

/// Is Sigil of Determination up, by the effects list?
fn sigil_up(state: &GameState) -> bool {
    let now = state.game_time_now();
    state.effects.iter().any(|(id, effect)| {
        effect.text.eq_ignore_ascii_case(SIGIL_NAME)
            && now.is_some_and(|now| state.effects.active(id, now) == Some(true))
    })
}
