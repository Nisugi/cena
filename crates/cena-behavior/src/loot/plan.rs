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

use cena_session::gameobj::ObjectTypes;
use cena_session::{GameState, RoomItem};

use super::learned::Learned;
use super::outcome::Outcome;
use super::profile::LootProfile;
use super::skin::Skinning;
use super::worth::{Verdict, is_special, lootable_by_verb, verdict};
use crate::stance::{self, Want};

mod alone;
mod bags;
mod boxed;
mod hands;
mod learn;
mod phase;
mod step;
pub(crate) use bags::{disks, named_bags};
use boxed::Boxed;
pub use boxed::Emptied;
use hands::{PutAway, Side};
use phase::Phasing;
pub use step::{Left, Step};

/// How many times a corpse is searched before it is given up on
/// (`eloot.lic:5670`, `3.times`).
const SEARCH_TRIES: u8 = 3;
/// How many times an item is dragged before it is given up on
/// (`eloot.lic:4059`, `5.times`).
const DRAG_TRIES: u8 = 5;
/// Sigil of Determination (9716), sent as Lich's cast proc sends it
/// (`crate::cast::power`; `tests/cast.rs` holds the two to one line), when
/// a corpse is *not in any condition* to be searched (`eloot.lic:5679-5685`).
const SIGIL_OF_DETERMINATION: &str = "sigil of determination";
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
    /// Creatures the game said cannot be skinned, by name.
    pub unskinnable: BTreeSet<String>,
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
    /// Bags the game said are closed, by hand, to open before the next try.
    closed: BTreeSet<String>,
    /// Bags opened this visit and closed again before it ended.
    reclosed: BTreeSet<String>,
    /// What this visit learned that the profile does not hold yet.
    learned: Learned,
    /// Critters' bags being emptied this visit.
    bags: BTreeMap<String, BagPhase>,
    room_looted: bool,
    /// What lay on the floor when `loot room` was last sent, by id with its
    /// kinds: a hand holding one of them afterwards is emptied before the
    /// room is looted again (`loot_all`, `eloot.lic:5263`). Never anything
    /// else a hand holds: the weapon stays where it is.
    gathered: Vec<(String, ObjectTypes)>,
    /// The bag the last `loot #id` was bound for: the game chooses it by
    /// the stow list, as [`Planner::bag_for`] does, and its *won't fit* and
    /// *closed* are about that bag, not the item.
    into: Option<String>,
    /// A search failed for the character's condition; the sigil helps once.
    sigil: Sigil,
    /// Skinning, before the searches, when the profile turns it on.
    skinning: Option<Skinning>,
    last: Option<Step>,
    bags_full: bool,
    /// A box in hand being emptied, instead of corpses and a floor.
    boxed: Option<Boxed>,
    /// Skin and stop: no search, no floor (`loot skin`).
    only_skin: bool,
    /// A step waiting for a hand to be freed for it (`hands.rs`).
    pending: Option<Step>,
    /// Tries at freeing a hand for the pending step.
    free_tries: u8,
    /// What was put away to free a hand, to give back at the end.
    put_away: Vec<PutAway>,
    /// The hand a creature's loot lands in, and the creature.
    catch: Option<(Side, i64)>,
    /// A box being dragged into a bag, to phase once it is in (`phase.rs`).
    into_box: Option<String>,
    /// A box being phased.
    phasing: Option<Phasing>,
}

impl Planner {
    /// A planner for the corpses here, with what earlier rooms taught.
    #[must_use]
    pub fn new(profile: LootProfile, memory: Memory, corpses: &[i64]) -> Self {
        let mut memory = memory;
        memory.crumbly.extend(profile.crumbly.iter().cloned());
        // The list is read only when the profile remembers (`eloot.lic:5655`).
        if profile.remember_unlootable {
            memory.unlootable.extend(profile.unlootable.iter().cloned());
        }
        memory
            .unskinnable
            .extend(profile.skin.unskinnable.iter().cloned());
        Planner {
            skinning: None,
            profile,
            memory,
            corpses: corpses.iter().copied().collect(),
            tries: BTreeMap::new(),
            skipped: BTreeSet::new(),
            opened: BTreeSet::new(),
            closed: BTreeSet::new(),
            reclosed: BTreeSet::new(),
            learned: Learned::default(),
            bags: BTreeMap::new(),
            room_looted: false,
            gathered: Vec::new(),
            into: None,
            sigil: Sigil::NotNeeded,
            last: None,
            bags_full: false,
            boxed: None,
            only_skin: false,
            pending: None,
            free_tries: 0,
            put_away: Vec::new(),
            catch: None,
            into_box: None,
            phasing: None,
        }
    }

    /// A planner that empties the box in hand (`box_loot`): opened, looked
    /// in, its coins gathered -- by `charm`, a name, when there is one --
    /// and what it holds taken as the floor's things are.
    #[must_use]
    pub fn for_box(
        profile: LootProfile,
        memory: Memory,
        box_id: &str,
        charm: Option<String>,
    ) -> Self {
        let mut planner = Self::new(profile, memory, &[]);
        planner.boxed = Some(Boxed::new(box_id, charm));
        planner
    }

    /// What was learned, for the next room.
    #[must_use]
    pub fn memory(&self) -> &Memory {
        &self.memory
    }

    /// The next command. A hand is freed first for a step that needs one;
    /// before the visit ends, what was put away is given back, and the bags
    /// it opened are closed again when the profile keeps them closed.
    pub fn next(&mut self, state: &GameState) -> Step {
        let step = match self.pending.take() {
            Some(step) => step,
            None => self.decide(state),
        };
        let mut step = self.with_hand(state, step);
        if matches!(step, Step::Done(_)) {
            if let Some(back) = self.give_back(state) {
                step = back;
            } else if let Some(bag) = self.reclose(state) {
                step = Step::Close(bag);
            }
        }
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
        if let Some(step) = self.phase_step(state) {
            return step;
        }
        if let Some(step) = self.stow_caught(state) {
            return step;
        }
        if let Some(step) = self.box_step(state) {
            return step;
        }
        if let Some(step) = self.skin(state) {
            return step;
        }
        if self.only_skin {
            return Step::Done(Left::Nothing);
        }
        if let Some(corpse) = self.corpses.front().copied() {
            if self.leaves_corpse(state, corpse) {
                self.corpses.pop_front();
                return self.decide(state);
            }
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
        if let Some(step) = self.empty_hands(state) {
            return step;
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
            // The bags the game stows into are opened first when the profile
            // keeps them closed (`open_loot_containers`, `eloot.lic:3869-3889`).
            for (_, types) in &floor.regular {
                if let Some(bag) = self.bag_for(state, types)
                    && let Some(open) = self.open_first(state, &bag)
                {
                    return open;
                }
            }
            self.room_looted = true;
            self.gathered = floor
                .regular
                .iter()
                .map(|(item, types)| (item.id.clone(), types.clone()))
                .collect();
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

    /// eloot skins before it searches (`Loot.skin`, `eloot.lic:2504`): the
    /// phase is built on the first call from the corpses here, and stands
    /// until it has nothing left to say.
    fn skin(&mut self, state: &GameState) -> Option<Step> {
        if !self.profile.skin.enable {
            return None;
        }
        if self.skinning.is_none() {
            let corpses: Vec<i64> = self
                .corpses
                .iter()
                .copied()
                .filter(|corpse| !self.leaves_corpse(state, *corpse))
                .collect();
            let unskinnable: Vec<String> = self.memory.unskinnable.iter().cloned().collect();
            self.skinning = Some(Skinning::new(
                self.profile.skin.clone(),
                state,
                &corpses,
                &unskinnable,
            ));
        }
        let skinning = self.skinning.as_mut()?;
        if skinning.is_done() {
            return None;
        }
        skinning.next(state)
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
        let Some(bag) = self.bag_for(state, types) else {
            self.bags_full = true;
            return Some(Step::Done(Left::BagsFull));
        };
        // Before either way of taking it: `loot #id` into a shut bag is
        // refused as a drag is.
        if let Some(open) = self.open_first(state, &bag) {
            return Some(open);
        }
        if lootable_by_verb(types) {
            self.into = Some(bag);
            return Some(Step::LootItem(item.id.clone()));
        }
        if types.is("box") {
            self.may_phase(&item.id, &item.text, bags::is_disk(state, &bag));
        }
        Some(Step::Drag {
            item: item.id.clone(),
            bag,
        })
    }

    /// The floor split into eloot's two passes and what is left. A thing
    /// kept by name is a special, taken one by one (`loot_specials`,
    /// `eloot.lic:5595`), whatever was learned of its name.
    fn split(&self, state: &GameState) -> Floor {
        let mut floor = Floor::default();
        for item in &state.room.objects {
            let kept = self.profile.keeps(&item.text);
            if self.skipped.contains(&item.id)
                || (!kept
                    && (self.memory.crumbly.contains(&item.text)
                        || self.memory.unlootable.contains(&item.text)))
            {
                floor.unwanted += 1;
                continue;
            }
            match verdict(item, &self.profile) {
                Verdict::Take(types) if kept || is_special(item, &types) => {
                    floor.specials.push((item.clone(), types));
                }
                Verdict::Take(types) => floor.regular.push((item.clone(), types)),
                Verdict::Leave(_) => floor.unwanted += 1,
            }
        }
        floor
    }

    /// Whether a corpse is left unsearched and unskinned: one of the
    /// profile's `leave_creatures`, or a child.
    fn leaves_corpse(&self, state: &GameState, corpse: i64) -> bool {
        state
            .creatures()
            .get(corpse)
            .is_some_and(|creature| self.profile.leaves_creature(&creature.name))
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

    /// What the game said to the last step. `state` is as it stands after
    /// the reply: the hands, for a gem a skinning broke out.
    pub fn outcome_in(&mut self, outcome: &Outcome, state: &GameState) {
        if self.too_heavy(state, outcome) {
            return;
        }
        if let (Some(last), Some(skinning)) = (self.last.clone(), self.skinning.as_mut())
            && let Some(name) = skinning.outcome(&last, outcome, state)
            && self.memory.unskinnable.insert(name.clone())
        {
            Learned::add(&mut self.learned.unskinnable, &name);
        }
        if *outcome == Outcome::Closed
            && let Some(bag) = self.shut_bag()
        {
            self.closed_on(Some(state), &bag);
            return;
        }
        self.outcome(outcome);
    }

    /// What the game said to the last step.
    pub fn outcome(&mut self, outcome: &Outcome) {
        if let Some(boxed) = self.boxed.as_mut() {
            boxed.outcome(outcome);
        }
        if *outcome == Outcome::Closed
            && let Some(bag) = self.shut_bag()
        {
            self.closed_on(None, &bag);
            return;
        }
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
                // What landed in the hands is dragged item by item
                // (`empty_hands`); the room is looted again once they are
                // away.
                self.room_looted = false;
            }
            (Step::LootRoom, _) => self.gathered.clear(),
            (Step::Open(bag), Outcome::NotAContainer | Outcome::NotFound) => {
                // Not a bag after all: nothing to empty, take it as it is.
                self.bags.remove(&bag);
                self.memory.checked_bags.insert(bag);
            }
            (Step::Open(bag), Outcome::Crumbled) => {
                self.bags.remove(&bag);
                self.skipped.insert(bag);
            }
            (Step::Drag { bag, .. }, Outcome::WontFit) => {
                self.memory.full.insert(bag);
            }
            (Step::Drag { item, .. }, Outcome::Stored) => self.stored(&item),
            (Step::Cast(_), Outcome::Hindered) => self.hindered(),
            // `loot #id` names the item; the bag is the one it was bound for.
            (Step::LootItem(_), Outcome::WontFit) => {
                if let Some(bag) = self.into.take() {
                    self.memory.full.insert(bag);
                }
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
}

/// Is Sigil of Determination up, by the effects list?
fn sigil_up(state: &GameState) -> bool {
    let now = state.game_time_now();
    state.effects.iter().any(|(id, effect)| {
        effect.text.eq_ignore_ascii_case(SIGIL_NAME)
            && now.is_some_and(|now| state.effects.active(id, now) == Some(true))
    })
}
