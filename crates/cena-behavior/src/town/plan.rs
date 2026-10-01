//! The errand planner: the next step of a selling round, given the state.
//! Pure, as the loot planner is.
//!
//! eloot's round (`Sell.sell`, `eloot.lic:7812-7847`, and `go_sell`,
//! `:7098`): what is in the selling bags decides which shops to visit; each
//! shop is the nearest room tagged for it. Boxes go to the locksmith pool
//! first, and what it has ready comes back and is emptied (`super::pool`).
//! The Chronomage's clerk is given
//! the gold rings; at the furrier and the gem shop a sack sells whole, the
//! note is read and the sack worn again (`plan/sack.rs`), then what is left sells
//! item by item, a bundle of skins a skin at a time; at the pawnshop
//! everything sells item by item, appraised first when the profile says so
//! and kept when it appraises over the limit or analyzes as a transmog;
//! collectibles are deposited at their counter. Last the bank, when the
//! round earned anything or a note waits in the default bag
//! (`finish_sell_run`, `:6109`); and the bank first, whenever the character
//! is over 80% encumbered on the way to a shop (`go_sell`, `:7132`), and
//! between, when the pool will not hand a box over to a character carrying
//! so much (`plan/bank.rs`). Then home.
//!
//! What the game answers arrives two ways: as the ledger's [`LootFact`]s --
//! a sale, an appraisal, a refusal, a note, a deposit -- and as the few
//! [`Reply`]s that are not loot facts. The planner is fed both after every
//! step.
//!
//! What the jeweler calls *not my field* is sold at the pawnshop instead, and
//! what it calls too valuable is appraised there when the profile asks; the
//! pawnshop is added to the round for either. Last, what the hands held when
//! the round began is fetched back (`return_hands`, `eloot.lic:3923`), so a
//! weapon stowed to free a hand is in hand again for the hunt.

use std::collections::{BTreeSet, VecDeque};

use cena_map::RoomId;
use cena_session::containers::StowSlot;
use cena_session::{GameState, LootFact};

use super::goods::{self, How, Lot, Onward, Shop};
use super::pool::{self, Pool};
use super::reply::Reply;
use super::settings::Town;
pub use super::step::{Round, Step};

mod bank;
mod sack;

use bank::Bank;
use sack::SackPhase;

/// Over this encumbrance, the bank comes before the next shop.
const HEAVY: u32 = 80;

/// How many times running the same command is sent before what it is for
/// is given up: the loot planner's `DRAG_TRIES`, and the pool's own count.
/// A reply no classifier knows (a closed bag, full hands, a shopkeeper's line
/// nobody has seen) otherwise sent the same `get`, `sell` or stow again to
/// the driver's cap of 400 steps (the review of 2026-09-29).
const SAME_TRIES: u8 = 5;

/// Where the item in hand is in its selling.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Doing {
    Fetching,
    /// A scroll read for the spells it holds.
    Reading,
    Analyzing,
    Appraising,
    /// Selling, depositing or giving, by the lot's `how`.
    Parting,
    /// A bundle in hand, taken apart a skin at a time.
    Unbundling,
    /// Waiting for a stow to be confirmed.
    Stowing,
}

/// The planner for one round.
#[derive(Clone, Debug)]
pub struct Seller {
    town: Town,
    home: RoomId,
    shops: VecDeque<Shop>,
    shop: Option<Shop>,
    /// The bags still to sell whole at this shop, by id.
    sacks: VecDeque<String>,
    sack: Option<(String, SackPhase)>,
    /// Bags already sold whole this round.
    sold_whole: BTreeSet<String>,
    lots: VecDeque<Lot>,
    lot: Option<(Lot, Doing)>,
    /// Items given up on this round.
    skipped: BTreeSet<String>,
    /// This shop's lots are built on arrival, after the sacks.
    lots_built: bool,
    /// The bank's part: the visit, the silver kept, when it is wanted.
    bank: Bank,
    /// The visit to the locksmith pool, while it lasts.
    pool: Option<Pool>,
    /// The pool's worker as the map names it for the room the character
    /// stands in.
    worker: Option<String>,
    /// What the gem shop sent on to the pawnshop.
    onward: Onward,
    /// What the hands held when the round began, to fetch back at its end.
    restore: Vec<String>,
    going_home: bool,
    last: Option<Step>,
    /// How many times running [`Seller::next`] has answered `last`.
    same: u8,
    /// Which part of the round this is.
    round: Round,
    /// What the round has come to so far.
    breakdown: super::Breakdown,
}

impl Seller {
    /// A round for what the selling bags hold, or `None` when there is
    /// nothing to sell and no note to deposit, or the stow list is not known.
    #[must_use]
    pub fn new(town: Town, state: &GameState, home: RoomId) -> Option<Self> {
        Self::for_round(town, state, home, Round::All)
    }

    /// [`Self::new`], for one part of the round. The pool alone goes even
    /// with no box to give when it is to collect; the bank alone always goes.
    #[must_use]
    pub fn for_round(town: Town, state: &GameState, home: RoomId, round: Round) -> Option<Self> {
        if !state.containers.stow_checked() {
            return None;
        }
        let mut shops = BTreeSet::new();
        let mut note = false;
        match round {
            Round::All => {
                for (item, types, _) in goods::goods(&town, state) {
                    if let Some(shop) = goods::shop_for(&town, &item, &types) {
                        shops.insert(shop);
                    }
                    // Clothing both shops buy is offered at the pawnshop too.
                    if types.is("clothing") {
                        shops.insert(Shop::Pawnshop);
                    }
                }
                // The pool for the boxes carried, or for its returns alone
                // every round (`process_boxes`, `eloot.lic:7696`, `:7714`).
                if (town.pool && !pool::boxes(&town, state).is_empty()) || town.always_check_pool {
                    shops.insert(Shop::Pool);
                }
                note = goods::note_in_bag(state);
                if shops.is_empty() && !note {
                    return None;
                }
            }
            Round::Pool { drop, collect } => {
                if !collect && (!drop || pool::boxes(&town, state).is_empty()) {
                    return None;
                }
                shops.insert(Shop::Pool);
            }
            // The bank is the round's last stop whenever it earned.
            Round::Bank => note = true,
        }
        // A box in hand goes to the pool; anything else comes back.
        let restore = [&state.right_hand, &state.left_hand]
            .into_iter()
            .filter(|hand| {
                !hand.noun().zip(hand.name()).is_some_and(|(noun, name)| {
                    cena_session::gameobj::classify(noun, name).is("box")
                })
            })
            .filter_map(|hand| hand.id().map(str::to_owned))
            .collect();
        let shops = goods::in_order(shops, town.fwi);
        let bank = Bank::for_round(round, state, town.keep_silver, note);
        Some(Seller {
            town,
            home,
            shops,
            shop: None,
            sacks: VecDeque::new(),
            sack: None,
            sold_whole: BTreeSet::new(),
            lots: VecDeque::new(),
            lot: None,
            skipped: BTreeSet::new(),
            lots_built: false,
            bank,
            pool: None,
            worker: None,
            onward: Onward::default(),
            restore,
            going_home: false,
            last: None,
            same: 0,
            round,
            breakdown: super::Breakdown::default(),
        })
    }

    /// The shops this round visits, in order, before the bank.
    #[must_use]
    pub fn shops(&self) -> Vec<Shop> {
        self.shop
            .into_iter()
            .chain(self.shops.iter().copied())
            .collect()
    }

    /// The pool's worker as the map names it for the room the character
    /// stands in (`cena_map::Room::pool_worker`), for a visit begun there.
    /// The driver says it before each step; a round never told finds the
    /// worker by eloot's words.
    pub fn worker_here(&mut self, name: Option<&str>) {
        self.worker = name.map(str::to_owned);
    }

    /// The next step.
    pub fn next(&mut self, state: &GameState, nearest: &dyn Fn(&str) -> Option<RoomId>) -> Step {
        let mut step = self.decide(state, nearest);
        if self.last.as_ref() == Some(&step) {
            self.same += 1;
        } else {
            self.same = 0;
        }
        if self.same >= SAME_TRIES {
            self.same = 0;
            self.give_up(&step);
            step = self.decide(state, nearest);
        }
        self.last = Some(step.clone());
        step
    }

    /// `step` was sent [`SAME_TRIES`] times and nothing came of it: what it
    /// was for is left, and the round goes on with the rest. With nothing
    /// to leave, the round is over.
    fn give_up(&mut self, step: &Step) {
        if let Some((lot, _)) = self.lot.take() {
            self.skipped.insert(lot.item.id);
        } else if let Some((sack, _)) = self.sack.take() {
            self.skipped.insert(sack);
        } else if self.bank.visit.take().is_some() {
            self.close_bank();
        } else if matches!(step, Step::Walk(_)) {
            self.going_home = true;
        } else if self.shop == Some(Shop::Pool) && self.pool.take().is_some() {
            // The visit is over, and what is in hand stays there.
            self.shop = None;
        } else {
            // A hand that will not be freed: no more shops, only home.
            self.shops.clear();
            self.shop = None;
            self.restore.clear();
            self.bank.banked = true;
        }
    }

    fn decide(&mut self, state: &GameState, nearest: &dyn Fn(&str) -> Option<RoomId>) -> Step {
        if self.going_home {
            return Step::Done;
        }
        if let Some(step) = self.continue_lot(state) {
            return step;
        }
        if let Some(step) = self.continue_sack(state) {
            return step;
        }
        if let Some(step) = self.continue_bank() {
            return step;
        }
        if self.shop.is_none() {
            let Some(shop) = self.pick_shop(state) else {
                if let Some(step) = self.restore_hands(state) {
                    return step;
                }
                self.going_home = true;
                return Step::Walk(self.home);
            };
            // A shop the map does not have here is skipped, as eloot skips it.
            let Some(room) = shop.tags().iter().find_map(|tag| nearest(tag)) else {
                if shop == Shop::Bank {
                    self.bank.banked = true;
                    self.bank.earned = false;
                }
                return self.decide(state, nearest);
            };
            self.shop = Some(shop);
            self.lots_built = false;
            return Step::Walk(room);
        }
        let shop = self.shop.unwrap_or(Shop::Pawnshop);
        if shop == Shop::Bank {
            self.bank.visit = Some(bank::Banking::Depositing);
            return Step::DepositAll;
        }
        if shop == Shop::Pool {
            return self.at_pool(state, nearest);
        }
        if !self.lots_built {
            self.lots_built = true;
            self.sacks = goods::sacks(shop, &self.town, state, &self.sold_whole).into();
            let whole: Vec<String> = self.sacks.iter().cloned().collect();
            self.lots =
                goods::lots(shop, &self.town, state, &self.skipped, &whole, &self.onward).into();
        }
        if let Some(sack) = self.sacks.pop_front() {
            self.sold_whole.insert(sack.clone());
            let phase = if holds(state, &sack) {
                SackPhase::Selling
            } else {
                SackPhase::Fetching
            };
            self.sack = Some((sack, phase));
            return self.continue_sack(state).unwrap_or(Step::Done);
        }
        if let Some(lot) = self.lots.pop_front() {
            if let Some(free) = free_a_hand(state) {
                self.lots.push_front(lot);
                return free;
            }
            self.lot = Some((lot.clone(), Doing::Fetching));
            return Step::Fetch(lot.item.id);
        }
        // This shop is done.
        self.shop = None;
        self.bank.banked = false;
        self.decide(state, nearest)
    }

    /// At the pool: the visit begun on arrival, gone on with, sent to the
    /// bank and back when it asks, and over when it says so.
    fn at_pool(&mut self, state: &GameState, nearest: &dyn Fn(&str) -> Option<RoomId>) -> Step {
        if self.pool.is_none() {
            let (drop, collect) = match self.round {
                Round::Pool { drop, collect } => (drop, collect),
                // A round gives the pool boxes when the profile uses it, and
                // asks for its returns always (`always_check_pool`).
                Round::All | Round::Bank => (self.town.pool, true),
            };
            // No worker in the room: the pool is passed by.
            self.pool = Pool::new(&self.town, state, self.worker.as_deref())
                .map(|pool| pool.only(drop, collect));
        }
        if self.pool.as_mut().is_some_and(Pool::wants_bank) {
            return self.bank_between(state, nearest);
        }
        if let Some(step) = self.pool.as_mut().and_then(|pool| pool.next(state)) {
            return step;
        }
        self.pool = None;
        self.shop = None;
        self.decide(state, nearest)
    }

    fn continue_lot(&mut self, state: &GameState) -> Option<Step> {
        let (lot, doing) = self.lot.clone()?;
        let id = lot.item.id.clone();
        let in_hand = holds(state, &id);
        match doing {
            Doing::Fetching => {
                if !in_hand {
                    // Not in hand yet: the driver sends the fetch and comes
                    // back with what the game said.
                    return Some(Step::Fetch(id));
                }
                Some(self.after_fetch(lot))
            }
            Doing::Reading => Some(Step::ReadScroll(id)),
            Doing::Analyzing => Some(Step::Analyze(id)),
            Doing::Appraising => Some(Step::Appraise(id)),
            Doing::Parting => Some(part(&lot)),
            Doing::Unbundling => {
                // A skin out of the bundle: sell it, or back to the bag when
                // the furrier would not have it.
                if let Some(skin) = other_hand(state, &id) {
                    if self.skipped.contains(&skin) {
                        return Some(Step::Stow {
                            item: skin,
                            bag: lot.bag,
                        });
                    }
                    return Some(Step::Sell(skin));
                }
                if in_hand {
                    return Some(Step::Unbundle);
                }
                self.lot = None;
                None
            }
            Doing::Stowing => {
                if in_hand {
                    return Some(Step::Stow {
                        item: id,
                        bag: self.keep_bag(&lot, state),
                    });
                }
                self.lot = None;
                None
            }
        }
    }

    /// The item is in hand: analyze, appraise, part with it or take it
    /// apart, in eloot's order.
    fn after_fetch(&mut self, lot: Lot) -> Step {
        let id = lot.item.id.clone();
        let (doing, step) = if lot.how == How::Unbundle {
            (Doing::Unbundling, Step::Unbundle)
        } else if lot.how == How::Appraise {
            (Doing::Appraising, Step::Appraise(id))
        } else if lot.read {
            (Doing::Reading, Step::ReadScroll(id))
        } else {
            return self.after_read(lot);
        };
        self.lot = Some((lot, doing));
        step
    }

    /// Analyze, appraise or part with it, in eloot's order.
    fn after_read(&mut self, lot: Lot) -> Step {
        let id = lot.item.id.clone();
        let (doing, step) = if lot.analyze {
            (Doing::Analyzing, Step::Analyze(id))
        } else if lot.appraise {
            (Doing::Appraising, Step::Appraise(id))
        } else {
            (Doing::Parting, part(&lot))
        };
        self.lot = Some((lot, doing));
        step
    }

    /// The hands as they were: each thing held when the round began and not
    /// held now is fetched, once; a thing the round left in both hands is
    /// stowed first.
    fn restore_hands(&mut self, state: &GameState) -> Option<Step> {
        self.restore.retain(|id| !holds(state, id));
        let id = self.restore.first()?.clone();
        if let Some(free) = free_a_hand(state) {
            return Some(free);
        }
        self.restore.remove(0);
        Some(Step::Fetch(id))
    }

    /// The gem shop sent something on: the pawnshop is in the round.
    fn pawnshop_too(&mut self) {
        if self.shop == Some(Shop::Pawnshop) || self.shops.contains(&Shop::Pawnshop) {
            return;
        }
        let at = self
            .shops
            .iter()
            .position(|shop| *shop > Shop::Pawnshop)
            .unwrap_or(self.shops.len());
        self.shops.insert(at, Shop::Pawnshop);
    }

    /// Where a kept item goes: the appraisal container by name, else the
    /// bag it came from.
    fn keep_bag(&self, lot: &Lot, state: &GameState) -> String {
        if !self.town.appraisal_container.is_empty() {
            for (id, container) in state.inventory.containers() {
                if container.title.as_deref().is_some_and(|title| {
                    title
                        .split(|c: char| !c.is_alphanumeric())
                        .any(|w| w.eq_ignore_ascii_case(&self.town.appraisal_container))
                }) {
                    return id.to_owned();
                }
            }
        }
        lot.bag.clone()
    }

    /// What the game said to the last step: the ledger's facts for the
    /// prompt, and the replies that are not facts.
    pub fn outcome(&mut self, facts: &[LootFact], replies: &[Reply], state: &GameState) {
        self.breakdown.take(facts);
        let Some(last) = self.last.clone() else {
            return;
        };
        let sold = facts.iter().any(|f| matches!(f, LootFact::Sold { .. }));
        // A tip paid changes the silver as a sale does: the bank evens it
        // out at the end, as eloot's `silver_deposit` does.
        self.bank.earned |= sold
            || facts
                .iter()
                .any(|f| matches!(f, LootFact::BoxOpened { .. } | LootFact::PoolDropped { .. }));
        if self.shop == Some(Shop::Pool)
            && let Some(pool) = self.pool.as_mut()
        {
            pool.outcome(&last, facts, replies, state);
            return;
        }
        let refused = facts
            .iter()
            .any(|f| matches!(f, LootFact::Worthless { .. } | LootFact::TooValuable { .. }))
            || replies.contains(&Reply::WrongShop);
        let appraised = facts.iter().find_map(|f| match f {
            LootFact::Appraised { value: Some(v), .. } => Some(*v),
            _ => None,
        });
        match last {
            Step::Fetch(id) => {
                if replies.contains(&Reply::CannotFetch) {
                    self.skipped.insert(id.clone());
                    if self.lot.as_ref().is_some_and(|(lot, _)| lot.item.id == id) {
                        self.lot = None;
                    }
                    if self.sack.as_ref().is_some_and(|(sack, _)| *sack == id) {
                        self.sack = None;
                    }
                }
            }
            Step::ReadScroll(_) | Step::Analyze(_) => self.examined(&last, replies),
            Step::Appraise(_) => {
                if let Some((lot, _)) = self.lot.clone() {
                    let limit = match self.shop {
                        Some(Shop::Gemshop) => self.town.appraise_gemshop,
                        _ => self.town.appraise_pawnshop,
                    };
                    let doing = match appraised {
                        _ if lot.how == How::Appraise => Doing::Stowing,
                        Some(value) if value > 0 && value <= limit && !refused => Doing::Parting,
                        _ => Doing::Stowing,
                    };
                    if doing == Doing::Stowing
                        && self.shop == Some(Shop::Gemshop)
                        && self.town.pawn_recheck
                    {
                        self.onward.recheck.insert(lot.item.id.clone());
                        self.pawnshop_too();
                    }
                    self.lot = Some((lot, doing));
                }
            }
            Step::Sell(id) => {
                if self.shop == Some(Shop::Gemshop) && self.lot.is_some() {
                    if replies.contains(&Reply::WrongShop) {
                        self.onward.retry.insert(id.clone());
                        self.pawnshop_too();
                    } else if self.town.pawn_recheck
                        && facts
                            .iter()
                            .any(|f| matches!(f, LootFact::TooValuable { .. }))
                    {
                        self.onward.recheck.insert(id.clone());
                        self.pawnshop_too();
                    }
                }
                if replies.contains(&Reply::Again) {
                    // Sell it again to be sure: the lot stays where it is.
                    return;
                }
                self.sold(&id, sold);
            }
            Step::Deposit(id) | Step::Give { item: id, .. } => {
                // Handed over when it has left the hands; else back it goes.
                if let Some((lot, _)) = self.lot.clone() {
                    self.lot = holds(state, &id).then_some((lot, Doing::Stowing));
                }
            }
            Step::Unbundle => {
                // Nothing came out: the bundle goes back to its bag.
                if let Some((lot, Doing::Unbundling)) = self.lot.clone()
                    && other_hand(state, &lot.item.id).is_none()
                    && !replies.contains(&Reply::LastTwo)
                {
                    self.lot = Some((lot, Doing::Stowing));
                }
            }
            Step::DepositAll => self.deposited(),
            Step::Withdraw(_) => {
                self.bank.visit = None;
                self.close_bank();
            }
            other => self.sack_outcome(&other, sold, refused, replies, state),
        }
    }

    /// A scroll read, or an item analyzed: kept, or on to its sale.
    fn examined(&mut self, last: &Step, replies: &[Reply]) {
        let Some((lot, _)) = self.lot.clone() else {
            return;
        };
        if matches!(last, Step::ReadScroll(_)) {
            let keep = replies.iter().any(|reply| match reply {
                Reply::ScrollSpell { spell, vibrant } => self.town.keeps_scroll(*spell, *vibrant),
                _ => false,
            });
            if keep {
                self.lot = Some((lot, Doing::Stowing));
            } else {
                self.after_read(lot);
            }
            return;
        }
        // ALTER 41 is always kept; a transmog when the profile keeps them
        // (`pawnshop`, `eloot.lic:7568-7575`).
        let keep = replies.contains(&Reply::Alter41)
            || (self.town.keep_transmogs && replies.contains(&Reply::Transmog));
        let doing = if keep {
            Doing::Stowing
        } else if lot.appraise {
            Doing::Appraising
        } else {
            Doing::Parting
        };
        self.lot = Some((lot, doing));
    }

    /// A `sell`'s answer: the lot is done, or it is kept; a skin out of a
    /// bundle the furrier would not take is put back.
    fn sold(&mut self, id: &str, sold: bool) {
        let Some((lot, doing)) = self.lot.clone() else {
            return;
        };
        if doing == Doing::Unbundling {
            if !sold {
                self.skipped.insert(id.to_owned());
            }
            return;
        }
        // Refused, or no answer read: kept, and the hands decide next turn.
        self.lot = (!sold).then_some((lot, Doing::Stowing));
    }

    /// Items given up on this round, for the driver's notes.
    #[must_use]
    pub fn skipped(&self) -> &BTreeSet<String> {
        &self.skipped
    }

    /// What the round came to, as lines for the player (`breakdown.rs`).
    #[must_use]
    pub fn breakdown(&self) -> Vec<String> {
        self.breakdown.lines(self.skipped.len())
    }
}

/// The command that parts with a lot in hand.
fn part(lot: &Lot) -> Step {
    let id = lot.item.id.clone();
    match &lot.how {
        How::Deposit => Step::Deposit(id),
        How::Give(to) => Step::Give {
            item: id,
            to: to.clone(),
        },
        How::Appraise => Step::Appraise(id),
        How::Sell | How::Unbundle => Step::Sell(id),
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

/// A hand to fetch into: `None` when one is free, else the stow that frees
/// the right hand into the default bag (`free_hands`, `:3839`).
fn free_a_hand(state: &GameState) -> Option<Step> {
    if !(state.right_hand.is_holding() && state.left_hand.is_holding()) {
        return None;
    }
    let item = state.right_hand.id()?.to_owned();
    let bag = state.containers.stow(StowSlot::Default)?.id.clone();
    Some(Step::Stow { item, bag })
}
