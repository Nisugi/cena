//! The locksmith pool (`plan/31` Stage 4c): eloot's `locksmith_pool`,
//! `pool_return` and `save_trash_box` (`eloot.lic:7334-7478`, `:7773`).
//!
//! Every box in the selling bags -- and on the disk, and in a hand -- is
//! given to the pool's worker with the profile's standard tip, the quote
//! confirmed. Then the worker is asked for what is ready, box after box until
//! nothing is: each returned box is emptied by the loot planner (the driver's
//! `EmptyBox`), then kept when it is a valuable empty box the profile sells,
//! else trashed, else dropped. A box the worker calls already open is emptied
//! on the spot, and a plinite it hands back is plucked and what came of it put
//! away (`box_loot`, `:5138-5140`).
//!
//! When the pool says no (`plan/61` step 5):
//!
//! - **The pool is full** (`handle_full_pool`, `:7420-7427`): the box goes
//!   back to its bag and what is ready is collected; when a box came back
//!   there is room again, and the drop-offs go on.
//! - **Too little silver for the tip**: no more drop-offs this visit.
//! - ***You need to lighten your load first*** (`pool_return`, `:7443-7452`),
//!   or a box's coins that would not all fit (`box_loot`, `:5109-5115`): the
//!   bank, then back to ask again ([`Pool::wants_bank`]); refused again
//!   straight after, what was refused is given up.
//!
//! The worker is the one the map names for the room (`Room::pool_worker`;
//! `find_worker`, `:3204`), else one of eloot's words for a worker.
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

/// The names a pool's worker goes by when the map names none
/// (`find_worker`, `eloot.lic:3207`), matched against the words of an NPC's
/// name.
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
/// `:7778`).
const VALUABLE: &[&str] = &["gold", "mithril", "silver"];

/// Where the pool visit is.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Doing {
    /// Between boxes.
    Idle,
    /// A box in the right hand, offered; `confirm` once the quote came.
    Tipping { id: String, confirm: bool },
    /// A box in the right hand looked at first, when the profile phases
    /// boxes: a phased one, `shifting`, is dropped and comes back to the
    /// hand whole (`box_unphase`, `eloot.lic:2986-2996`).
    Unphasing { id: String, shifting: bool },
    /// A box going back to its bag.
    Back { id: String, bag: String },
    /// `ask #worker for return` sent.
    Asking,
    /// A box handed to the loot planner.
    Emptying { id: String, bag: String },
    /// An emptied box on its way out: trash, then drop, then back to a bag.
    Tossing { id: String, tries: u8 },
    /// A plinite handed back, plucked; `other` is what the other hand held
    /// already, which stays there.
    Plucking { id: String, other: Option<String> },
    /// What the plucking left in the hands, into the default bag, as eloot
    /// frees both hands after it (`box_loot`, `eloot.lic:5140`).
    PuttingAway { other: Option<String> },
}

/// One visit to the pool.
#[derive(Clone, Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "three profile switches and four facts about the visit, each read in one place"
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
    /// Look at each box before it is given, to unphase a phased one.
    unphase: bool,
    /// Boxes looked at and whole, by id.
    whole: Vec<String>,
    /// No more drop-offs this visit: the silver ran out, or this part of
    /// the visit gives none.
    stop_dropping: bool,
    /// The pool is full, and how many boxes have come back since: room
    /// made for the rest.
    full: Option<u32>,
    /// The worker has nothing more ready.
    returns_over: bool,
    /// The bank is wanted before the next step.
    unload: bool,
    /// The last refusal sent the round to the bank: another straight after
    /// gives up what was refused.
    unloaded: bool,
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

/// The pool's worker in the room: the one the map names for it, when it
/// names one (`find_worker`, `eloot.lic:3204-3211`), else one by eloot's
/// words. A worker the map names and the room lacks is no worker: a word
/// could find somebody else, and the box and its tip would go to them.
pub(super) fn worker(state: &GameState, named: Option<&str>) -> Option<String> {
    let npcs = &state.room.creatures;
    let found = match named {
        Some(name) => npcs.iter().find(|npc| npc.text.contains(name)),
        None => npcs.iter().find(|npc| {
            npc.text
                .split(|c: char| !c.is_alphanumeric())
                .chain([npc.noun.as_str()])
                .any(|word| WORKERS.contains(&word))
        }),
    };
    found.map(|npc| npc.id.clone())
}

impl Pool {
    /// A visit, or `None` when no worker is in the room. `named` is the
    /// worker the map names for the room.
    pub(super) fn new(town: &Town, state: &GameState, named: Option<&str>) -> Option<Self> {
        Some(Self {
            worker: worker(state, named)?,
            boxes: boxes(town, state).into(),
            doing: Doing::Idle,
            tip: town.pool_tip,
            percent: town.pool_tip_percent,
            keep_valuable: town.sells("box"),
            unphase: town.phase_boxes,
            whole: Vec::new(),
            stop_dropping: false,
            full: None,
            returns_over: false,
            unload: false,
            unloaded: false,
            default_bag: state
                .containers
                .stow(StowSlot::Default)
                .map(|b| b.id.clone()),
        })
    }

    /// Only part of the visit (`;eloot pool deposit`, `pool return`,
    /// `eloot.lic:8011-8032`; a round's returns alone with
    /// `always_check_pool`): without `drop` nothing is given to the worker,
    /// without `collect` nothing is asked back.
    pub(super) fn only(mut self, drop: bool, collect: bool) -> Self {
        if !drop {
            self.boxes.clear();
            self.stop_dropping = true;
        }
        self.returns_over = !collect;
        self
    }

    /// Whether the bank is wanted before the next step: the worker will not
    /// hand a box over to a character carrying this much, or a box's coins
    /// would not all fit. Said once; the round goes, comes back, and the
    /// visit goes on where it was.
    pub(super) fn wants_bank(&mut self) -> bool {
        std::mem::take(&mut self.unload)
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
            Doing::Unphasing { id, shifting } => Some(if shifting {
                Step::Drop(id)
            } else {
                Step::LookAt(id)
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
            Doing::Plucking { id, .. } => Some(Step::Pluck(id)),
            Doing::PuttingAway { other } => {
                let held = [&state.right_hand, &state.left_hand]
                    .into_iter()
                    .filter_map(|hand| hand.id())
                    .find(|id| other.as_deref() != Some(*id));
                if let (Some(item), Some(bag)) = (held, self.default_bag.clone()) {
                    return Some(Step::Stow {
                        item: item.to_owned(),
                        bag,
                    });
                }
                self.doing = Doing::Idle;
                self.next(state)
            }
        }
    }

    fn idle(&mut self, state: &GameState) -> Option<Step> {
        if !self.stop_dropping
            && self.full.is_none()
            && let Some((id, _)) = self.boxes.front().cloned()
        {
            if state.right_hand.holds(&id) {
                self.doing = if self.unphase && !self.whole.contains(&id) {
                    Doing::Unphasing {
                        id,
                        shifting: false,
                    }
                } else {
                    Doing::Tipping { id, confirm: false }
                };
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
            (Step::LookAt(_), Doing::Unphasing { id, .. }) => {
                if replies.contains(&Reply::Shifting) {
                    self.doing = Doing::Unphasing { id, shifting: true };
                } else {
                    self.whole.push(id);
                    self.doing = Doing::Idle;
                }
            }
            (Step::Drop(_), Doing::Unphasing { id, .. }) => {
                // Back in hand whole, perhaps by another id: eloot finds the
                // box in hand again (`box_unphase`, `eloot.lic:2993-2995`).
                let now = box_in_hand(state).unwrap_or(id.clone());
                for (held, _) in &mut self.boxes {
                    if *held == id {
                        held.clone_from(&now);
                    }
                }
                self.whole.push(now);
                self.doing = Doing::Idle;
            }
            (Step::AskReturn(_), Doing::Asking) => self.asked(facts, replies, state),
            (Step::EmptyBox(_), Doing::Emptying { id, bag }) => {
                self.emptied(id, bag, replies, state);
            }
            (Step::Pluck(_), Doing::Plucking { other, .. }) => {
                self.doing = Doing::PuttingAway { other };
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

    /// The worker's answer to `ask for return`.
    fn asked(&mut self, facts: &[LootFact], replies: &[Reply], state: &GameState) {
        if replies.contains(&Reply::Lighten) {
            if self.unloaded {
                // Refused again straight after the bank: eloot gives the
                // returns up (`pool_return`, `eloot.lic:7444-7446`).
                self.returns_over = true;
                self.doing = Doing::Idle;
            } else {
                self.unload = true;
                self.unloaded = true;
            }
            return;
        }
        let back = facts
            .iter()
            .any(|f| matches!(f, LootFact::BoxReturned { .. }));
        match returned(state) {
            Some((id, plinite)) if back || !replies.contains(&Reply::NoneReady) => {
                self.unloaded = false;
                if let Some(since) = self.full.as_mut() {
                    *since += 1;
                }
                self.doing = if plinite {
                    Doing::Plucking {
                        other: other_hand(state, &id),
                        id,
                    }
                } else {
                    let bag = self.default_bag.clone().unwrap_or_default();
                    Doing::Emptying { id, bag }
                };
            }
            _ => {
                if self.full.is_some_and(|since| since > 0) {
                    // Boxes came back since the pool was full: room for the
                    // rest (`handle_full_pool`, `eloot.lic:7420-7427`).
                    self.full = None;
                } else {
                    self.returns_over = true;
                }
                self.doing = Doing::Idle;
            }
        }
    }

    /// A returned box, emptied by the loot planner: kept, or out it goes.
    fn emptied(&mut self, id: String, bag: String, replies: &[Reply], state: &GameState) {
        let coins_left = replies.contains(&Reply::CoinsLeft);
        if coins_left && !self.unloaded {
            // Its coins would not all fit: the bank, then the box again
            // (`box_loot`, `eloot.lic:5109-5115`).
            self.unload = true;
            self.unloaded = true;
            return;
        }
        self.unloaded = false;
        // Locked, coins still in it after the bank, or a valuable box the
        // profile sells: back in the bag. Else out it goes.
        self.doing = if replies.contains(&Reply::BoxLocked)
            || coins_left
            || (self.keep_valuable && is_valuable(state, &id))
        {
            Doing::Back { id, bag }
        } else {
            Doing::Tossing { id, tries: 0 }
        };
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
        } else if replies.contains(&Reply::PoolFull) {
            // Back in its bag, and given again once returns have made room
            // (`locksmith_pool`, `eloot.lic:7404-7407`).
            self.full = Some(0);
            self.doing = Doing::Back { id, bag };
        } else if replies.contains(&Reply::NoSilver) {
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

/// What the other hand holds, when it is not `id`.
fn other_hand(state: &GameState, id: &str) -> Option<String> {
    [&state.right_hand, &state.left_hand]
        .into_iter()
        .filter_map(|hand| hand.id())
        .find(|held| *held != id)
        .map(str::to_owned)
}

/// What the worker handed back, in either hand: a box, or a plinite, `true`
/// (`pool_return`, `eloot.lic:7455-7464`).
fn returned(state: &GameState) -> Option<(String, bool)> {
    [&state.right_hand, &state.left_hand]
        .into_iter()
        .find_map(|hand| {
            let types = classify(hand.noun()?, hand.name()?);
            let plinite = types.is("plinite");
            if !plinite && !types.is("box") {
                return None;
            }
            Some((hand.id()?.to_owned(), plinite))
        })
}

/// Whether the emptied box `id`, in a hand, is kept rather than thrown out:
/// one of gold, mithril or silver, or a reliquary, when the profile sells
/// boxes (`save_trash_box`, `eloot.lic:7773`).
#[must_use]
pub fn keeps_box(town: &Town, state: &GameState, id: &str) -> bool {
    town.sells("box") && is_valuable(state, id)
}

/// A box in either hand, by id.
#[must_use]
pub fn box_in_hand(state: &GameState) -> Option<String> {
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
