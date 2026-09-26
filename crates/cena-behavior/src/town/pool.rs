//! The locksmith pool (`plan/31` Stage 4c): eloot's `locksmith_pool`,
//! `pool_return` and `save_trash_box` (`eloot.lic:7310-7455`, `:7749`).
//!
//! Every box in the selling bags -- and on the disk, and in a hand -- is
//! given to the pool's worker with the profile's standard tip, the quote
//! confirmed; a full pool or too little silver stops the drop-off and the
//! box goes back to its bag. Then the worker is asked for what is ready,
//! box after box until nothing is: each returned box is emptied by the loot
//! planner (the driver's `EmptyBox`), then kept when it is a valuable empty
//! box the profile sells, else trashed, else dropped. A box the worker
//! calls already open is emptied on the spot.
//!
//! The answers are the ledger's facts where the ledger reads them (the
//! quote, the drop, the return) and [`Reply`]s where it does not.

use std::collections::VecDeque;

use cena_session::containers::StowSlot;
use cena_session::gameobj::classify;
use cena_session::{GameState, LootFact};

use super::goods;
use super::plan::Step;
use super::reply::Reply;
use super::settings::Town;

/// The names a pool's worker goes by (`find_worker`, `eloot.lic:3196`),
/// matched against the words of an NPC's name.
const WORKERS: &[&str] = &[
    "worker",
    "trickster",
    "Jahck",
    "woman",
    "attendant",
    "gnome",
    "merchant",
    "dwarf",
];
/// Boxes worth keeping empty when the profile sells boxes (`save_trash_box`,
/// `:7754`).
const VALUABLE: &[&str] = &["gold", "mithril", "silver"];

/// Where the pool visit is.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Doing {
    /// Between boxes.
    Idle,
    /// A box in the right hand, offered; `confirm` once the quote came.
    Tipping { id: String, confirm: bool },
    /// A box going back to its bag.
    Back { id: String, bag: String },
    /// `ask #worker for return` sent.
    Asking,
    /// A box handed to the loot planner.
    Emptying { id: String, bag: String },
    /// An emptied box on its way out: trash, then drop, then back to a bag.
    Tossing { id: String, tries: u8 },
}

/// One visit to the pool.
#[derive(Clone, Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "two profile switches and two facts about the visit, each read in one place"
)]
pub(super) struct Pool {
    worker: String,
    /// Boxes to drop off, with the bag each came from.
    boxes: VecDeque<(String, String)>,
    doing: Doing,
    tip: u64,
    percent: bool,
    /// Keep an emptied box of gold, mithril or silver.
    keep_valuable: bool,
    /// The pool is full or the silver ran out: no more drop-offs.
    stop_dropping: bool,
    /// The worker has nothing more ready.
    returns_over: bool,
    default_bag: Option<String>,
}

/// The boxes the round takes to the pool: in a hand first, then the selling
/// bags, then the disk when the profile uses one.
pub(super) fn boxes(town: &Town, state: &GameState) -> Vec<(String, String)> {
    let default_bag = state
        .containers
        .stow(StowSlot::Default)
        .map(|b| b.id.clone())
        .unwrap_or_default();
    let mut out: Vec<(String, String)> = [&state.right_hand, &state.left_hand]
        .into_iter()
        .filter(|hand| {
            hand.noun()
                .zip(hand.name())
                .is_some_and(|(noun, name)| classify(noun, name).is("box"))
        })
        .filter_map(|hand| hand.id().map(|id| (id.to_owned(), default_bag.clone())))
        .collect();
    let mut bags = goods::bags(town, state);
    if town.disk
        && let Some(disk) = goods::own_disk(state)
    {
        let items = state
            .inventory
            .container(&disk)
            .map(|c| c.items.clone())
            .unwrap_or_default();
        bags.push((disk, items));
    }
    for (bag, items) in bags {
        for item in items {
            if classify(&item.noun, &item.text).is("box")
                && !out.iter().any(|(id, _)| *id == item.id)
            {
                out.push((item.id, bag.clone()));
            }
        }
    }
    out
}

/// The pool's worker in the room, by eloot's names.
pub(super) fn worker(state: &GameState) -> Option<String> {
    state
        .room
        .creatures
        .iter()
        .find(|npc| {
            npc.text
                .split(|c: char| !c.is_alphanumeric())
                .chain([npc.noun.as_str()])
                .any(|word| WORKERS.contains(&word))
        })
        .map(|npc| npc.id.clone())
}

impl Pool {
    /// A visit, or `None` when no worker is in the room.
    pub(super) fn new(town: &Town, state: &GameState) -> Option<Self> {
        Some(Self {
            worker: worker(state)?,
            boxes: boxes(town, state).into(),
            doing: Doing::Idle,
            tip: town.pool_tip,
            percent: town.pool_tip_percent,
            keep_valuable: town.sells("box"),
            stop_dropping: false,
            returns_over: false,
            default_bag: state
                .containers
                .stow(StowSlot::Default)
                .map(|b| b.id.clone()),
        })
    }

    /// The next step; `None` when the visit is over.
    pub(super) fn next(&mut self, state: &GameState) -> Option<Step> {
        match self.doing.clone() {
            Doing::Idle => self.idle(state),
            Doing::Tipping { confirm, .. } => Some(Step::Tip {
                to: self.worker.clone(),
                amount: self.tip,
                percent: self.percent,
                confirm,
            }),
            Doing::Back { id, bag } => {
                if holds(state, &id) {
                    return Some(Step::Stow { item: id, bag });
                }
                self.doing = Doing::Idle;
                self.next(state)
            }
            Doing::Asking => Some(Step::AskReturn(self.worker.clone())),
            Doing::Emptying { id, .. } => Some(Step::EmptyBox(id)),
            Doing::Tossing { id, tries } => {
                if !holds(state, &id) {
                    self.doing = Doing::Idle;
                    return self.next(state);
                }
                match tries {
                    0 => Some(Step::Trash(id)),
                    1 => Some(Step::Drop(id)),
                    _ => {
                        let bag = self.default_bag.clone()?;
                        self.doing = Doing::Back { id, bag };
                        self.next(state)
                    }
                }
            }
        }
    }

    fn idle(&mut self, state: &GameState) -> Option<Step> {
        if !self.stop_dropping
            && let Some((id, _)) = self.boxes.front().cloned()
        {
            if state.right_hand.holds(&id) {
                self.doing = Doing::Tipping { id, confirm: false };
                return self.next(state);
            }
            if state.left_hand.holds(&id) {
                return Some(if state.right_hand.is_holding() {
                    // Both hands full: the right hand's thing away first.
                    let item = state.right_hand.id()?.to_owned();
                    Step::Stow {
                        item,
                        bag: self.default_bag.clone()?,
                    }
                } else {
                    Step::Swap
                });
            }
            if state.right_hand.is_holding() && state.left_hand.is_holding() {
                let item = state.right_hand.id()?.to_owned();
                return Some(Step::Stow {
                    item,
                    bag: self.default_bag.clone()?,
                });
            }
            return Some(Step::Fetch(id));
        }
        if !self.returns_over {
            self.doing = Doing::Asking;
            return self.next(state);
        }
        None
    }

    /// What the game said to the last step.
    pub(super) fn outcome(
        &mut self,
        last: &Step,
        facts: &[LootFact],
        replies: &[Reply],
        state: &GameState,
    ) {
        match (last, self.doing.clone()) {
            (Step::Fetch(id), _) => {
                if replies.contains(&Reply::CannotFetch) {
                    self.boxes.retain(|(b, _)| b != id);
                }
            }
            (Step::Tip { .. }, Doing::Tipping { id, confirm }) => {
                self.tipped(id, confirm, facts, replies, state);
            }
            (Step::AskReturn(_), Doing::Asking) => {
                let back = facts
                    .iter()
                    .any(|f| matches!(f, LootFact::BoxReturned { .. }));
                let in_hand = box_in_hand(state);
                match in_hand {
                    Some(id) if back || !replies.contains(&Reply::NoneReady) => {
                        let bag = self.default_bag.clone().unwrap_or_default();
                        self.doing = Doing::Emptying { id, bag };
                    }
                    _ => {
                        self.returns_over = true;
                        self.doing = Doing::Idle;
                    }
                }
            }
            (Step::EmptyBox(_), Doing::Emptying { id, bag }) => {
                // Locked, or a valuable box the profile sells: back in the
                // bag. Else out it goes.
                self.doing = if replies.contains(&Reply::BoxLocked)
                    || (self.keep_valuable && is_valuable(state, &id))
                {
                    Doing::Back { id, bag }
                } else {
                    Doing::Tossing { id, tries: 0 }
                };
            }
            (Step::Trash(_) | Step::Drop(_), Doing::Tossing { id, tries }) => {
                // Asked to throw it again to be sure: the same step, once more.
                let again = replies.contains(&Reply::Again) && tries < 2;
                self.doing = Doing::Tossing {
                    id,
                    tries: if again { tries } else { tries + 1 },
                };
            }
            _ => {}
        }
    }

    /// The worker's answer to a tip.
    fn tipped(
        &mut self,
        id: String,
        confirm: bool,
        facts: &[LootFact],
        replies: &[Reply],
        state: &GameState,
    ) {
        let bag = self
            .boxes
            .iter()
            .find(|(b, _)| *b == id)
            .map(|(_, bag)| bag.clone())
            .or_else(|| self.default_bag.clone())
            .unwrap_or_default();
        let dropped = facts
            .iter()
            .any(|f| matches!(f, LootFact::PoolDropped { .. }))
            || !holds(state, &id);
        let quoted = facts
            .iter()
            .any(|f| matches!(f, LootFact::PoolQuoted { .. }));
        if dropped {
            self.boxes.retain(|(b, _)| *b != id);
            self.doing = Doing::Idle;
        } else if replies.contains(&Reply::PoolFull) || replies.contains(&Reply::NoSilver) {
            // Nothing more goes in this visit; what is left waits for the
            // next rest.
            self.stop_dropping = true;
            self.boxes.retain(|(b, _)| *b != id);
            self.doing = Doing::Back { id, bag };
        } else if replies.contains(&Reply::AlreadyOpen) {
            self.boxes.retain(|(b, _)| *b != id);
            self.doing = Doing::Emptying { id, bag };
        } else if !confirm && (quoted || replies.is_empty()) {
            self.doing = Doing::Tipping { id, confirm: true };
        } else {
            self.boxes.retain(|(b, _)| *b != id);
            self.doing = Doing::Back { id, bag };
        }
    }
}

/// Either hand holds this id.
fn holds(state: &GameState, id: &str) -> bool {
    state.right_hand.holds(id) || state.left_hand.holds(id)
}

/// A box in either hand, by id.
fn box_in_hand(state: &GameState) -> Option<String> {
    [&state.right_hand, &state.left_hand]
        .into_iter()
        .find(|hand| {
            hand.noun()
                .zip(hand.name())
                .is_some_and(|(noun, name)| classify(noun, name).is("box"))
        })
        .and_then(|hand| hand.id().map(str::to_owned))
}

/// A box of gold, mithril or silver, by its name in hand.
fn is_valuable(state: &GameState, id: &str) -> bool {
    [&state.right_hand, &state.left_hand]
        .into_iter()
        .find(|hand| hand.holds(id))
        .and_then(|hand| hand.name())
        .is_some_and(|name| {
            name.split(' ').any(|word| VALUABLE.contains(&word)) || name.contains("reliquary")
        })
}
