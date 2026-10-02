//! A box in hand, emptied: eloot's `box_loot` (`eloot.lic:5086-5142`).
//!
//! The same planner that empties a critter's bag empties a box: `open`, `look
//! in`, the coins gathered -- by pointing the profile's charm at the box when
//! it names one, else `get coins from` -- then what the box holds taken by
//! the planner's own [`Planner::take`], one item at a time, into the bag its
//! kind belongs in. What the profile does not want stays in the box. The box
//! itself is the caller's: the selling round trashes it or keeps it.
//!
//! The caller is told how it came out ([`Emptied`]), and throws a box out
//! only when it is: one that says it is locked is left alone (`return
//! Inventory.single_drag(box) if line =~ /locked/`), as is one whose contents
//! were never listed (`:5096`); coins the character cannot carry stay in the
//! box, and the caller banks and empties it again (`:5109-5115`); and a
//! thing no bag would take stays in it too, the bags full or the thing a
//! gold ingot too heavy for any (`single_drag`, `:4012-4020`, `:4141-4146`).

use cena_session::gameobj::ObjectTypes;
use cena_session::{GameState, RoomItem};

use super::super::outcome::Outcome;
use super::super::worth::{Verdict, verdict};
use super::{DRAG_TRIES, Left, Planner, Step};

/// How many times the coins are asked for before they are left.
const COIN_TRIES: u8 = 3;

/// How a box came out of [`Planner::for_box`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Emptied {
    /// Nothing the profile wants is left in it.
    Out,
    /// It would not open: back in its bag (`box_loot`, `eloot.lic:5090`).
    Locked,
    /// Its coins would not all fit on the character, and the rest are still
    /// in it: eloot banks and gathers again (`:5109-5115`).
    CoinsLeft,
    /// What it holds was never listed, so nobody knows: kept, as eloot keeps
    /// it (`:5096`).
    Unseen,
    /// Something the profile wants is still in it: no bag would take it, or a
    /// drag of it never took.
    ThingsLeft,
}

/// Where the box is in being emptied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Opening,
    Looking,
    Coins,
    Taking,
}

/// One box being emptied.
#[derive(Clone, Debug, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "four ways a box can fail to come out empty, each read in `emptied`"
)]
pub(super) struct Boxed {
    id: String,
    /// The charm pointed at the box for its coins, by name.
    charm: Option<String>,
    phase: Phase,
    coin_tries: u8,
    locked: bool,
    /// The character could carry no more of its coins.
    coins_left: bool,
    /// Its contents were never listed.
    unseen: bool,
    /// A wanted thing is still in it.
    things_left: bool,
}

impl Boxed {
    pub(super) fn new(id: &str, charm: Option<String>) -> Self {
        Self {
            id: id.to_owned(),
            charm: charm.filter(|name| !name.is_empty()),
            phase: Phase::Opening,
            coin_tries: 0,
            locked: false,
            coins_left: false,
            unseen: false,
            things_left: false,
        }
    }

    /// What the game said to the last step, for the box.
    pub(super) fn outcome(&mut self, outcome: &Outcome) {
        match outcome {
            Outcome::Locked => self.locked = true,
            // Gathered, or the character can hold no more: the coins are
            // done, and what is left of them stays in the box.
            Outcome::Gathered | Outcome::CoinsFull if self.phase == Phase::Coins => {
                self.coins_left = *outcome == Outcome::CoinsFull;
                self.phase = Phase::Taking;
            }
            _ => {}
        }
    }
}

impl Planner {
    /// The box being emptied, by id.
    pub(super) fn boxed_id(&self) -> Option<&str> {
        self.boxed.as_ref().map(|boxed| boxed.id.as_str())
    }

    /// How the box emptied by [`Planner::for_box`] came out.
    #[must_use]
    pub fn emptied(&self) -> Emptied {
        match &self.boxed {
            Some(boxed) if boxed.locked => Emptied::Locked,
            Some(boxed) if boxed.coins_left => Emptied::CoinsLeft,
            Some(boxed) if boxed.unseen => Emptied::Unseen,
            Some(boxed) if boxed.things_left || self.bags_full => Emptied::ThingsLeft,
            _ => Emptied::Out,
        }
    }

    /// A gold ingot that will not fit says nothing of the bag: it is too
    /// heavy for any, and eloot leaves the bag open to the rest
    /// (`single_drag`, `eloot.lic:4141-4146`). It stays where it is, and a
    /// box holding it is not emptied. `true` when that was the answer.
    pub(super) fn too_heavy(&mut self, state: &GameState, outcome: &Outcome) -> bool {
        let (Outcome::WontFit, Some(Step::Drag { item, .. } | Step::LootItem(item))) =
            (outcome, &self.last)
        else {
            return false;
        };
        if !named(state, item).is_some_and(|name| name.contains("gold ingot")) {
            return false;
        }
        self.skipped.insert(item.clone());
        self.into = None;
        if let Some(boxed) = self.boxed.as_mut() {
            boxed.things_left = true;
        }
        true
    }

    /// The box's next step; `None` when the planner is not emptying a box.
    pub(super) fn box_step(&mut self, state: &GameState) -> Option<Step> {
        let boxed = self.boxed.as_mut()?;
        let id = boxed.id.clone();
        if boxed.locked {
            return Some(Step::Done(Left::Nothing));
        }
        match boxed.phase {
            Phase::Opening => {
                boxed.phase = Phase::Looking;
                Some(Step::Open(id))
            }
            Phase::Looking => {
                boxed.phase = Phase::Coins;
                Some(Step::LookIn(id))
            }
            Phase::Coins => {
                let coins = state
                    .inventory
                    .container(&id)
                    .is_some_and(|inside| inside.items.iter().any(|i| is_coins(&i.text)));
                if coins && boxed.coin_tries < COIN_TRIES {
                    boxed.coin_tries += 1;
                    return Some(match &boxed.charm {
                        Some(charm) => Step::Charm {
                            charm: charm.clone(),
                            box_: id,
                        },
                        None => Step::Coins(id),
                    });
                }
                boxed.phase = Phase::Taking;
                self.box_step(state)
            }
            Phase::Taking => {
                if !state.containers.stow_checked() {
                    return Some(Step::Ask("stow list"));
                }
                let Some(inside) = state.inventory.container(&id) else {
                    boxed.unseen = true;
                    return Some(Step::Done(Left::Nothing));
                };
                let wanted: Vec<(RoomItem, ObjectTypes)> = inside
                    .items
                    .iter()
                    .filter(|thing| !is_coins(&thing.text))
                    .filter_map(|thing| match verdict(thing, &self.profile) {
                        Verdict::Take(kinds) => Some((thing.clone(), kinds)),
                        Verdict::Leave(_) => None,
                    })
                    .collect();
                for (thing, kinds) in &wanted {
                    if self.skipped.contains(&thing.id) {
                        continue;
                    }
                    if let Some(step) = self.take(state, thing, kinds) {
                        return Some(step);
                    }
                }
                // A thing whose drags never took is still in the box.
                let given_up = wanted.iter().any(|(thing, _)| {
                    self.tries
                        .get(&thing.id)
                        .is_some_and(|tries| *tries >= DRAG_TRIES)
                });
                if given_up && let Some(boxed) = self.boxed.as_mut() {
                    boxed.things_left = true;
                }
                Some(Step::Done(Left::Nothing))
            }
        }
    }
}

/// The box's coins, as the contents list them.
fn is_coins(text: &str) -> bool {
    text.ends_with("coins")
}

/// The name of the thing `id`, on the floor or in a container listed.
fn named<'a>(state: &'a GameState, id: &str) -> Option<&'a str> {
    state
        .room
        .objects
        .iter()
        .chain(
            state
                .inventory
                .containers()
                .flat_map(|(_, inside)| inside.items.iter()),
        )
        .find(|thing| thing.id == id)
        .map(|thing| thing.text.as_str())
}
