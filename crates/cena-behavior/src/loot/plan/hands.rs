//! The hands: what `loot room` left in them, a hand freed before a step
//! that needs one, and what was put away given back when the visit ends.
//! Moved down from `plan.rs` at its cap.
//!
//! **Left by `loot room`** (`loot_all`, `eloot.lic:5263`): the game answers
//! *too much* when the hands filled before the floor emptied, and eloot drags
//! a hand's thing away when it is one of those it was looting.
//!
//! **A hand freed** (`free_hand`, `:3816-3837`): before `loot room`, a thing
//! taken one by one, the skinner wielded, eloot makes sure one hand is free
//! and fit to use, a hand being unfit with a wound or a scar of rank 3 on its
//! arm or hand. When neither is free it frees the left when the profile
//! favors it (`favor_left`) and the left is fit, or when the right is not;
//! else the right. An armament is stored where the player set it to go, as
//! eloot stores a readied weapon (`stow_ready_list`, `:4175-4190`); anything
//! else goes into its bag. eloot's own branch for neither hand fit cannot be
//! reached (a right hand not fit is always *damaged*, so it frees the left);
//! here that ends the visit, a reason to rest.
//!
//! **Given back** (`return_hands`, `:3923-3944`): what was put away to free
//! a hand goes back into it when the visit ends.
//!
//! **A creature that hands its loot over** (`search`, `:5709-5765`): a
//! bramble's kind puts it in the left hand, a skayl's kind in whichever is
//! free; the hand is freed before the search and what lands in it put in its
//! bag after.

use cena_session::GameState;
use cena_session::body::{Body, Track};
use cena_session::gameobj::classify;
use cena_session::hands::Hand;

use super::{DRAG_TRIES, Left, Planner, Step};
use crate::travel::hands::{Stored, is_armament, take_back};

/// Creatures whose search puts a thing in the left hand (`search`,
/// `eloot.lic:5709`).
const INTO_LEFT: [&str; 6] = ["tumbleweed", "plant", "shrub", "creeper", "vine", "bush"];
/// Creatures whose search puts a thing in a free hand (`:5710`).
const INTO_HAND: [&str; 5] = ["skayl", "glacei", "caedera", "golem", "elemental"];
/// How many times a hand is tried at being freed before the step that wanted
/// it is sent anyway.
const FREE_TRIES: u8 = 3;

/// One hand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Side {
    Right,
    Left,
}

impl Side {
    const fn word(self) -> &'static str {
        match self {
            Self::Right => "right",
            Self::Left => "left",
        }
    }

    const fn hand(self, state: &GameState) -> &Hand {
        match self {
            Self::Right => &state.right_hand,
            Self::Left => &state.left_hand,
        }
    }

    /// Fit to use: no wound or scar of rank 3 on its arm or hand
    /// (`free_hand`, `eloot.lic:3817-3821`).
    fn fit(self, state: &GameState) -> bool {
        let parts = match self {
            Self::Right => ["rightArm", "rightHand"],
            Self::Left => ["leftArm", "leftHand"],
        };
        let body = Body::new(&state.character.injuries);
        body.worst(&parts, Track::Wound)
            .max(body.worst(&parts, Track::Scar))
            < 3
    }

    /// Holding nothing, as far as the game has said, and fit to use.
    fn free(self, state: &GameState) -> bool {
        !self.hand(state).is_holding() && self.fit(state)
    }
}

/// A hand's thing put away to free it, given back when the visit ends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PutAway {
    side: Side,
    id: String,
    name: String,
    armament: bool,
}

/// Which hand a step needs free.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Needs {
    Either,
    Left,
}

impl Planner {
    /// What `loot room` left in a hand, dragged to its bag: the room is
    /// looted again only once the hands are clear of it, or the game answers
    /// *too much* to every `loot room` there is.
    pub(super) fn empty_hands(&mut self, state: &GameState) -> Option<Step> {
        let held = [&state.right_hand, &state.left_hand]
            .into_iter()
            .filter_map(|hand| hand.id())
            .find_map(|id| self.gathered.iter().find(|(item, _)| item == id))?
            .clone();
        if !state.containers.stow_checked() {
            return Some(Step::Ask("stow list"));
        }
        let (id, types) = held;
        if *self.tries.get(&id).unwrap_or(&0) >= DRAG_TRIES {
            // It will not go away: no more of the room by `loot room`.
            self.gathered.clear();
            self.room_looted = true;
            return None;
        }
        *self.tries.entry(id.clone()).or_insert(0) += 1;
        let Some(bag) = self.bag_for(state, &types) else {
            self.bags_full = true;
            return Some(Step::Done(Left::BagsFull));
        };
        if let Some(open) = self.open_first(state, &bag) {
            return Some(open);
        }
        Some(Step::Drag { item: id, bag })
    }

    /// `step`, or first what frees a hand for it. The step waits in
    /// `pending` and is sent once the hand is free, or after
    /// [`FREE_TRIES`] tries at freeing one.
    pub(super) fn with_hand(&mut self, state: &GameState, step: Step) -> Step {
        let Some(needs) = Self::needs(state, &step) else {
            return step;
        };
        let free = match needs {
            Needs::Either => Side::Right.free(state) || Side::Left.free(state),
            Needs::Left => !Side::Left.hand(state).is_holding(),
        };
        if free || self.free_tries >= FREE_TRIES {
            self.free_tries = 0;
            if let Step::Search(corpse) = step {
                self.catch = Self::catching(state, needs).map(|side| (side, corpse));
            }
            return step;
        }
        let side = match needs {
            Needs::Left => Side::Left,
            Needs::Either => match self.side_to_free(state) {
                Some(side) => side,
                None => return Step::Done(Left::NoHand),
            },
        };
        self.free_tries += 1;
        match self.put_away(state, side) {
            Ok(Some(free)) => {
                self.pending = Some(step);
                free
            }
            Ok(None) => step,
            Err(left) => {
                self.bags_full = true;
                Step::Done(left)
            }
        }
    }

    /// Which hand `step` needs free, if any: the room looted, a thing taken
    /// or the skinner wielded, either; a creature that hands its loot over,
    /// the one it hands it to.
    fn needs(state: &GameState, step: &Step) -> Option<Needs> {
        match step {
            Step::LootRoom | Step::LootItem(_) | Step::Wield(_) => Some(Needs::Either),
            Step::Drag { item, .. }
                if !state.right_hand.holds(item) && !state.left_hand.holds(item) =>
            {
                Some(Needs::Either)
            }
            Step::Search(corpse) => {
                let name = state.creatures().get(*corpse)?.name.to_ascii_lowercase();
                let words: Vec<&str> = name.split(|c: char| !c.is_alphanumeric()).collect();
                if INTO_LEFT.iter().any(|w| words.contains(w)) {
                    Some(Needs::Left)
                } else if INTO_HAND.iter().any(|w| words.contains(w)) {
                    Some(Needs::Either)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// The hand a creature's loot will land in: the left for a bramble's
    /// kind, or when both are free; else the free one (`search`,
    /// `eloot.lic:5726-5730`).
    fn catching(state: &GameState, needs: Needs) -> Option<Side> {
        let both_free = !state.right_hand.is_holding() && !state.left_hand.is_holding();
        if needs == Needs::Left || both_free {
            return Some(Side::Left);
        }
        [Side::Right, Side::Left]
            .into_iter()
            .find(|side| !side.hand(state).is_holding())
    }

    /// The hand to free, by eloot's rule; never one holding the box being
    /// emptied. `None` when neither is fit.
    fn side_to_free(&self, state: &GameState) -> Option<Side> {
        let fit = |side: Side| {
            side.fit(state) && !self.boxed_id().is_some_and(|id| side.hand(state).holds(id))
        };
        let (right, left) = (fit(Side::Right), fit(Side::Left));
        if !right && !left {
            return None;
        }
        Some(if (self.profile.favor_left && left) || !right {
            Side::Left
        } else {
            Side::Right
        })
    }

    /// The step that empties `side`: an armament stored, unless storing it
    /// was just tried; anything else dragged into its bag. `None` when the
    /// hand holds nothing after all.
    fn put_away(&mut self, state: &GameState, side: Side) -> Result<Option<Step>, Left> {
        let hand = side.hand(state);
        let (Some(name), noun) = (hand.name(), hand.noun().unwrap_or_default()) else {
            return Ok(None);
        };
        let store = format!("store {}", side.word());
        let tried = self.last.as_ref() == Some(&Step::Hand(store.clone()));
        let armament = is_armament(noun, name) && !tried;
        let Some(id) = hand.id() else {
            // Nothing to give back by: the game puts it where it goes.
            let verb = if armament { "store" } else { "stow" };
            return Ok(Some(Step::Hand(format!("{verb} {}", side.word()))));
        };
        let put = PutAway {
            side,
            id: id.to_owned(),
            name: name.to_owned(),
            armament,
        };
        if armament {
            self.keep_put(put);
            return Ok(Some(Step::Hand(store)));
        }
        let bag = self
            .bag_for(state, &classify(noun, name))
            .ok_or(Left::BagsFull)?;
        if let Some(open) = self.open_first(state, &bag) {
            return Ok(Some(open));
        }
        self.keep_put(put);
        Ok(Some(Step::Drag {
            item: id.to_owned(),
            bag,
        }))
    }

    fn keep_put(&mut self, put: PutAway) {
        self.put_away.retain(|held| held.id != put.id);
        self.put_away.push(put);
    }

    /// What a creature put in the catching hand, into its bag.
    pub(super) fn stow_caught(&mut self, state: &GameState) -> Option<Step> {
        let (side, corpse) = self.catch?;
        let hand = side.hand(state);
        let Some(id) = hand.id() else {
            if !self.corpses.contains(&corpse) {
                self.catch = None;
            }
            return None;
        };
        self.catch = None;
        let types = classify(
            hand.noun().unwrap_or_default(),
            hand.name().unwrap_or_default(),
        );
        let Some(bag) = self.bag_for(state, &types) else {
            self.bags_full = true;
            return Some(Step::Done(Left::BagsFull));
        };
        if let Some(open) = self.open_first(state, &bag) {
            self.catch = Some((side, corpse));
            return Some(open);
        }
        Some(Step::Drag {
            item: id.to_owned(),
            bag,
        })
    }

    /// What was put away to free a hand, given back, the last first; `None`
    /// when everything is back.
    pub(super) fn give_back(&mut self, state: &GameState) -> Option<Step> {
        while let Some(put) = self.put_away.pop() {
            if state.right_hand.holds(&put.id) || state.left_hand.holds(&put.id) {
                continue;
            }
            let line = if put.armament {
                take_back(
                    state,
                    &Stored {
                        id: put.id,
                        name: put.name,
                    },
                )
            } else {
                // Into the hand it came out of (`Inventory.drag`, `:3764`).
                format!("_drag #{} {}", put.id, put.side.word())
            };
            return Some(Step::Hand(line));
        }
        None
    }
}
