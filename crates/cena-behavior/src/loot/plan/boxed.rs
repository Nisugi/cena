//! A box in hand, emptied: eloot's `box_loot` (`eloot.lic:5071-5127`).
//!
//! The same planner that empties a critter's bag empties a box: `open`, `look
//! in`, the coins gathered -- by pointing the profile's charm at the box when
//! it names one, else `get coins from` -- then what the box holds taken by
//! the planner's own [`Planner::take`], one item at a time, into the bag its
//! kind belongs in. What the profile does not want stays in the box. The box
//! itself is the caller's: the selling round trashes it or keeps it.
//!
//! A box that says it is locked is left alone and reported, so the caller
//! puts it back in its bag (`return Inventory.single_drag(box) if line =~
//! /locked/`). So are coins the character cannot carry: the rest stay in the
//! box, and the caller banks and empties it again (`:5109-5115`).

use cena_session::GameState;
use cena_session::gameobj::ObjectTypes;

use super::super::outcome::Outcome;
use super::super::worth::{Verdict, verdict};
use super::{Left, Planner, Step};

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
pub(super) struct Boxed {
    id: String,
    /// The charm pointed at the box for its coins, by name.
    charm: Option<String>,
    phase: Phase,
    coin_tries: u8,
    locked: bool,
    /// The character could carry no more of its coins.
    coins_left: bool,
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
            _ => Emptied::Out,
        }
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
                let inside: Vec<(cena_session::RoomItem, ObjectTypes)> = state
                    .inventory
                    .container(&id)
                    .map(|inside| inside.items.clone())
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|thing| !is_coins(&thing.text) && !self.skipped.contains(&thing.id))
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
                Some(Step::Done(Left::Nothing))
            }
        }
    }
}

/// The box's coins, as the contents list them.
fn is_coins(text: &str) -> bool {
    text.ends_with("coins")
}
