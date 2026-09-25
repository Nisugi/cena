//! The errand planner: the next step of a selling round, given the state.
//! Pure, as the loot planner is.
//!
//! eloot's round (`Sell.sell`, `eloot.lic:7788-7820`, and `go_sell`,
//! `:7074`): what is in the selling bags decides which shops to visit; each
//! shop is the nearest room tagged for it. Boxes go to the locksmith pool
//! first, and what it has ready comes back and is emptied (`super::pool`).
//! The Chronomage's clerk is given
//! the gold rings; at the furrier and the gem shop a sack sells whole, the
//! note is read and the sack worn again, then what is left sells item by
//! item, a bundle of skins a skin at a time; at the pawnshop everything sells
//! item by item, appraised first when the profile says so and kept when it
//! appraises over the limit or analyzes as a transmog; collectibles are
//! deposited at their counter. Last the bank, when the round earned anything
//! or a note waits in the default bag (`finish_sell_run`, `:6094`); and the
//! bank first, whenever the character is over 80% encumbered on the way to
//! a shop (`go_sell`, `:7106`). Then home.
//!
//! What the game answers arrives two ways: as the ledger's [`LootFact`]s --
//! a sale, an appraisal, a refusal, a note, a deposit -- and as the few
//! [`Reply`]s that are not loot facts. The planner is fed both after every
//! step.

use std::collections::{BTreeSet, VecDeque};

use cena_map::RoomId;
use cena_session::containers::StowSlot;
use cena_session::{GameState, LootFact};

use super::goods::{self, How, Lot, Shop};
use super::pool::{self, Pool};
use super::reply::Reply;
use super::settings::Town;

/// Over this encumbrance, the bank comes before the next shop.
const HEAVY: u32 = 80;

/// One command for the driver to send, or the end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Walk there.
    Walk(RoomId),
    /// `get #id`: into a hand.
    Fetch(String),
    /// `sell #id`.
    Sell(String),
    /// `appraise #id`.
    Appraise(String),
    /// `analyze #id`, for a possible transmog.
    Analyze(String),
    /// `sell #sack`: the whole sack at once.
    SellSack(String),
    /// `wear #sack`: the sack back on after its bulk sale.
    Wear(String),
    /// `read #note`: the note a bulk sale paid with.
    ReadNote(String),
    /// `deposit #id`: a collectible handed in.
    Deposit(String),
    /// `give #item to #to`: a gold ring to the Chronomage's clerk.
    Give {
        /// The item's id.
        item: String,
        /// The clerk's id.
        to: String,
    },
    /// `bundle remove`: one skin out of the bundle in hand.
    Unbundle,
    /// `deposit all`: the silver and notes carried, into the account.
    DepositAll,
    /// `withdraw N silver`: what the profile keeps in hand.
    Withdraw(u64),
    /// `swap`: the box into the right hand, where the worker takes it.
    Swap,
    /// `give #to <amount>[ PERCENT][ confirm]`: a box and its tip to the
    /// pool's worker.
    Tip {
        /// The worker's id.
        to: String,
        /// The tip in silver, or a percent of the box's value.
        amount: u64,
        /// The tip is a percent.
        percent: bool,
        /// The second give, accepting the worker's quote.
        confirm: bool,
    },
    /// `ask #worker for return`: a box the pool has finished.
    AskReturn(String),
    /// The box in hand, emptied by the loot planner (`box_loot`); the driver
    /// runs it and says [`Reply::BoxLocked`] when the box would not open.
    EmptyBox(String),
    /// `trash #id`: an emptied box into the room's receptacle.
    Trash(String),
    /// `drop #id`: an emptied box, where there is no receptacle.
    Drop(String),
    /// Put one thing in one bag.
    Stow {
        /// The item's id.
        item: String,
        /// The bag's id.
        bag: String,
    },
    /// The round is over; the driver is home.
    Done,
}

/// Where the item in hand is in its selling.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Doing {
    Fetching,
    Analyzing,
    Appraising,
    /// Selling, depositing or giving, by the lot's `how`.
    Parting,
    /// A bundle in hand, taken apart a skin at a time.
    Unbundling,
    /// Waiting for a stow to be confirmed.
    Stowing,
}

/// A sack sold whole.
#[derive(Clone, Debug, PartialEq, Eq)]
enum SackPhase {
    Fetching,
    Selling,
    Wearing,
    Reading(String),
    StowingNote(String),
}

/// Where the bank visit is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Banking {
    Depositing,
    Withdrawing,
}

/// The planner for one round.
#[derive(Clone, Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "four independent facts about the round, each read in one place"
)]
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
    bank: Option<Banking>,
    /// The visit to the locksmith pool, while it lasts.
    pool: Option<Pool>,
    /// A sale or a note this round: the bank is wanted at the end.
    earned: bool,
    /// The bank was visited since the last shop: no second trip for weight.
    banked: bool,
    going_home: bool,
    last: Option<Step>,
}

impl Seller {
    /// A round for what the selling bags hold, or `None` when there is
    /// nothing to sell and no note to deposit, or the stow list is not known.
    #[must_use]
    pub fn new(town: Town, state: &GameState, home: RoomId) -> Option<Self> {
        if !state.containers.stow_checked() {
            return None;
        }
        let mut shops = BTreeSet::new();
        for (item, types, _) in goods::goods(&town, state) {
            if let Some(shop) = goods::shop_for(&town, &item, &types) {
                shops.insert(shop);
            }
            // Clothing both shops buy is offered at the pawnshop too.
            if types.is("clothing") {
                shops.insert(Shop::Pawnshop);
            }
        }
        if town.pool && !pool::boxes(&town, state).is_empty() {
            shops.insert(Shop::Pool);
        }
        let note = goods::note_in_bag(state);
        if shops.is_empty() && !note {
            return None;
        }
        Some(Seller {
            town,
            home,
            shops: shops.into_iter().collect(),
            shop: None,
            sacks: VecDeque::new(),
            sack: None,
            sold_whole: BTreeSet::new(),
            lots: VecDeque::new(),
            lot: None,
            skipped: BTreeSet::new(),
            lots_built: false,
            bank: None,
            pool: None,
            earned: note,
            banked: false,
            going_home: false,
            last: None,
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

    /// The next step.
    pub fn next(&mut self, state: &GameState, nearest: &dyn Fn(&str) -> Option<RoomId>) -> Step {
        let step = self.decide(state, nearest);
        self.last = Some(step.clone());
        step
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
                self.going_home = true;
                return Step::Walk(self.home);
            };
            // A shop the map does not have here is skipped, as eloot skips it.
            let Some(room) = shop.tags().iter().find_map(|tag| nearest(tag)) else {
                if shop == Shop::Bank {
                    self.banked = true;
                    self.earned = false;
                }
                return self.decide(state, nearest);
            };
            self.shop = Some(shop);
            self.lots_built = false;
            return Step::Walk(room);
        }
        let shop = self.shop.unwrap_or(Shop::Pawnshop);
        if shop == Shop::Bank {
            self.bank = Some(Banking::Depositing);
            return Step::DepositAll;
        }
        if shop == Shop::Pool {
            if self.pool.is_none() {
                // No worker in the room: the pool is passed by.
                self.pool = Pool::new(&self.town, state);
            }
            if let Some(step) = self.pool.as_mut().and_then(|pool| pool.next(state)) {
                return step;
            }
            self.pool = None;
            self.shop = None;
            return self.decide(state, nearest);
        }
        if !self.lots_built {
            self.lots_built = true;
            self.sacks = goods::sacks(shop, &self.town, state, &self.sold_whole).into();
            let whole: Vec<String> = self.sacks.iter().cloned().collect();
            self.lots = goods::lots(shop, &self.town, state, &self.skipped, &whole).into();
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
        self.banked = false;
        self.decide(state, nearest)
    }

    /// The next shop: the bank first when heavy, the queue, the bank last
    /// when the round earned anything.
    fn pick_shop(&mut self, state: &GameState) -> Option<Shop> {
        let heavy = state
            .character
            .encumbrance_percent
            .is_some_and(|now| now > HEAVY);
        if heavy && !self.banked && !self.shops.is_empty() {
            return Some(Shop::Bank);
        }
        if let Some(shop) = self.shops.pop_front() {
            return Some(shop);
        }
        (self.earned && !self.banked).then_some(Shop::Bank)
    }

    fn continue_bank(&mut self) -> Option<Step> {
        match self.bank? {
            Banking::Depositing => Some(Step::DepositAll),
            Banking::Withdrawing => Some(Step::Withdraw(self.town.keep_silver)),
        }
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
        } else if lot.analyze {
            (Doing::Analyzing, Step::Analyze(id))
        } else if lot.appraise {
            (Doing::Appraising, Step::Appraise(id))
        } else {
            (Doing::Parting, part(&lot))
        };
        self.lot = Some((lot, doing));
        step
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

    fn continue_sack(&mut self, state: &GameState) -> Option<Step> {
        let (sack, phase) = self.sack.clone()?;
        let in_hand = holds(state, &sack);
        match phase {
            SackPhase::Fetching => {
                if in_hand {
                    self.sack = Some((sack.clone(), SackPhase::Selling));
                    return Some(Step::SellSack(sack));
                }
                if let Some(free) = free_a_hand(state) {
                    return Some(free);
                }
                Some(Step::Fetch(sack))
            }
            SackPhase::Selling => Some(Step::SellSack(sack)),
            SackPhase::Wearing => {
                if in_hand {
                    return Some(Step::Wear(sack));
                }
                // Worn again. A note in a hand is the bulk sale's payment.
                if let Some(note) = goods::note_in_hand(state) {
                    self.sack = Some((sack, SackPhase::Reading(note.clone())));
                    return Some(Step::ReadNote(note));
                }
                self.sack = None;
                // The shop is visited again for what the sale left.
                self.lots_built = false;
                None
            }
            SackPhase::Reading(note) => Some(Step::ReadNote(note)),
            SackPhase::StowingNote(note) => {
                if holds(state, &note) {
                    let bag = state
                        .containers
                        .stow(StowSlot::Default)
                        .map(|b| b.id.clone())?;
                    return Some(Step::Stow { item: note, bag });
                }
                self.sack = None;
                self.lots_built = false;
                None
            }
        }
    }

    /// What the game said to the last step: the ledger's facts for the
    /// prompt, and the replies that are not facts.
    pub fn outcome(&mut self, facts: &[LootFact], replies: &[Reply], state: &GameState) {
        let Some(last) = self.last.clone() else {
            return;
        };
        let sold = facts.iter().any(|f| matches!(f, LootFact::Sold { .. }));
        self.earned |= sold
            || facts
                .iter()
                .any(|f| matches!(f, LootFact::BoxOpened { .. }));
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
            Step::Analyze(_) => {
                if let Some((lot, _)) = self.lot.clone() {
                    let keep =
                        replies.contains(&Reply::Transmog) || replies.contains(&Reply::Alter41);
                    let doing = if keep {
                        Doing::Stowing
                    } else if lot.appraise {
                        Doing::Appraising
                    } else {
                        Doing::Parting
                    };
                    self.lot = Some((lot, doing));
                }
            }
            Step::Appraise(_) => {
                if let Some((lot, _)) = self.lot.clone() {
                    let limit = match self.shop {
                        Some(Shop::Gemshop) => self.town.appraise_gemshop,
                        _ => self.town.appraise_pawnshop,
                    };
                    let doing = match appraised {
                        Some(value) if value > 0 && value <= limit && !refused => Doing::Parting,
                        _ => Doing::Stowing,
                    };
                    self.lot = Some((lot, doing));
                }
            }
            Step::Sell(id) => self.sold(&id, sold),
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
            Step::DepositAll => {
                self.bank = (self.town.keep_silver > 0).then_some(Banking::Withdrawing);
                self.close_bank();
            }
            Step::Withdraw(_) => {
                self.bank = None;
                self.close_bank();
            }
            other => self.sack_outcome(&other, sold, refused, replies, state),
        }
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

    /// The bank visit is over when no withdrawal is left.
    fn close_bank(&mut self) {
        if self.bank.is_none() {
            self.shop = None;
            self.banked = true;
            self.earned = false;
        }
    }

    /// The sack's, the note's and the stow's answers.
    fn sack_outcome(
        &mut self,
        last: &Step,
        sold: bool,
        refused: bool,
        replies: &[Reply],
        state: &GameState,
    ) {
        match last.clone() {
            Step::SellSack(sack) => {
                // Sold, or nothing in it the shop wants whole: item by item
                // then. Either way the sack goes back on.
                if sold || refused || replies.contains(&Reply::SackInspected) {
                    self.sack = Some((sack, SackPhase::Wearing));
                }
            }
            Step::Wear(sack) => {
                if replies.contains(&Reply::CannotWear) {
                    // Back in the default bag instead.
                    self.sack = Some((sack, SackPhase::StowingNote(String::new())));
                }
            }
            Step::ReadNote(note) => {
                self.earned = true;
                self.sack = self
                    .sack
                    .take()
                    .map(|(sack, _)| (sack, SackPhase::StowingNote(note)));
            }
            Step::Stow { item, .. } if !holds(state, &item) => {
                if self
                    .lot
                    .as_ref()
                    .is_some_and(|(lot, _)| lot.item.id == item)
                {
                    self.lot = None;
                }
                if let Some((sack, SackPhase::StowingNote(_))) = &self.sack
                    && !holds(state, sack)
                {
                    self.sack = None;
                    self.lots_built = false;
                }
            }
            _ => {}
        }
    }

    /// Items given up on this round, for the driver's notes.
    #[must_use]
    pub fn skipped(&self) -> &BTreeSet<String> {
        &self.skipped
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
/// the right hand into the default bag (`free_hands`, `:3960`).
fn free_a_hand(state: &GameState) -> Option<Step> {
    if !(state.right_hand.is_holding() && state.left_hand.is_holding()) {
        return None;
    }
    let item = state.right_hand.id()?.to_owned();
    let bag = state.containers.stow(StowSlot::Default)?.id.clone();
    Some(Step::Stow { item, bag })
}
