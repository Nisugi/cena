//! The errand planner: the next step of a selling round, given the state.
//! Pure, as the loot planner is.
//!
//! eloot's round (`Sell.sell`, `eloot.lic:7788-7820`, and `go_sell`,
//! `:7074`): what is in the selling bags decides which shops to visit; each
//! shop is the nearest room tagged for it; at the gem shop a sack of gems
//! sells in one `sell #sack`, the note is read and the sack worn again, then
//! what is left sells item by item; at the pawnshop everything sells item by
//! item, appraised first when the profile says so and kept when it appraises
//! over the limit or analyzes as a transmog. Then home.
//!
//! What the game answers arrives two ways: as the ledger's [`LootFact`]s --
//! a sale, an appraisal, a refusal, a note -- and as the few [`Reply`]s that
//! are not loot facts. The planner is fed both after every step.

use std::collections::{BTreeSet, VecDeque};

use cena_map::RoomId;
use cena_session::containers::StowSlot;
use cena_session::gameobj::{ObjectTypes, classify};
use cena_session::{GameState, LootFact, RoomItem};

use super::reply::Reply;
use super::settings::Town;

/// The shops of the first stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Shop {
    /// Gems, and what the jeweler buys: tagged `gemshop`.
    Gemshop,
    /// Everything else: tagged `pawnshop`.
    Pawnshop,
}

impl Shop {
    /// The map tag the room carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Gemshop => "gemshop",
            Self::Pawnshop => "pawnshop",
        }
    }
}

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
    /// `sell #sack`: the whole sack of gems at once.
    SellSack(String),
    /// `wear #sack`: the sack back on after its bulk sale.
    Wear(String),
    /// `read #note`: the note a bulk sale paid with.
    ReadNote(String),
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

/// One item to sell at the current shop.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Lot {
    item: RoomItem,
    /// The bag it came from, to go back to.
    bag: String,
    appraise: bool,
    analyze: bool,
}

/// Where the item in hand is in its selling.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Doing {
    Fetching,
    Analyzing,
    Appraising,
    Selling,
    /// Waiting for a stow to be confirmed.
    Stowing,
}

/// A sack sold whole at the gem shop.
#[derive(Clone, Debug, PartialEq, Eq)]
enum SackPhase {
    Fetching,
    Selling,
    Wearing,
    Reading(String),
    StowingNote(String),
}

/// The planner for one round.
#[derive(Clone, Debug)]
pub struct Seller {
    town: Town,
    home: RoomId,
    shops: VecDeque<Shop>,
    shop: Option<Shop>,
    arrived: bool,
    /// The gem sacks still to sell whole at the gem shop, by id.
    sacks: VecDeque<String>,
    sack: Option<(String, SackPhase)>,
    lots: VecDeque<Lot>,
    lot: Option<(Lot, Doing)>,
    /// Items given up on this round.
    skipped: BTreeSet<String>,
    /// The gem shop's lots are built when it is reached, after the sacks.
    lots_built: bool,
    going_home: bool,
    last: Option<Step>,
}

/// Which categories eloot analyzes for a transmog (`is_transmog?`,
/// `eloot.lic:4090`).
const TRANSMOG_KINDS: &[&str] = &["jewelry", "clothing", "armor", "weapon", "uncommon"];
/// Categories the pawnshop appraises whatever the profile says (`:7522`).
const ALWAYS_APPRAISED: &[&str] = &["uncommon", "weapon", "armor"];

impl Seller {
    /// A round for what the selling bags hold, or `None` when there is
    /// nothing to sell, the stow list is not known, or no shop has anything.
    #[must_use]
    pub fn new(town: Town, state: &GameState, home: RoomId) -> Option<Self> {
        if !state.containers.stow_checked() {
            return None;
        }
        let mut shops = BTreeSet::new();
        for (item, types, _) in Self::goods(&town, state) {
            if types.sells_to("gemshop") || (types.is("gem") && is_thorn_or_berry(&item)) {
                shops.insert(Shop::Gemshop);
            }
            if (types.sells_to("pawnshop") && !types.sells_to("gemshop")) || types.is("clothing") {
                shops.insert(Shop::Pawnshop);
            }
        }
        if shops.is_empty() {
            return None;
        }
        Some(Seller {
            town,
            home,
            shops: shops.into_iter().collect(),
            shop: None,
            arrived: false,
            sacks: VecDeque::new(),
            sack: None,
            lots: VecDeque::new(),
            lot: None,
            skipped: BTreeSet::new(),
            lots_built: false,
            going_home: false,
            last: None,
        })
    }

    /// The shops this round visits, in order.
    #[must_use]
    pub fn shops(&self) -> Vec<Shop> {
        self.shop
            .into_iter()
            .chain(self.shops.iter().copied())
            .collect()
    }

    /// The bags sold from, by the profile's `sell_container` slots, with
    /// their contents as the inventory lists them.
    fn bags(town: &Town, state: &GameState) -> Vec<(String, Vec<RoomItem>)> {
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        for word in &town.containers {
            let slot = match word.as_str() {
                "default" => Some(StowSlot::Default),
                "overflow" => None,
                other => StowSlot::parse(other),
            };
            let Some(bag) = slot.and_then(|slot| state.containers.stow(slot)) else {
                continue;
            };
            if !seen.insert(bag.id.clone()) {
                continue;
            }
            let items = state
                .inventory
                .container(&bag.id)
                .map(|c| c.items.clone())
                .unwrap_or_default();
            out.push((bag.id.clone(), items));
        }
        out
    }

    /// Everything sellable in the selling bags: the item, its types, its bag.
    fn goods(town: &Town, state: &GameState) -> Vec<(RoomItem, ObjectTypes, String)> {
        let ready: BTreeSet<String> = cena_session::containers::ReadySlot::ALL
            .iter()
            .filter_map(|slot| state.containers.ready(*slot))
            .map(|item| item.id.clone())
            .collect();
        let mut out = Vec::new();
        for (bag, items) in Self::bags(town, state) {
            for item in items {
                if ready.contains(&item.id)
                    || town.excludes(&item.text)
                    || item.text.split(' ').any(|w| w == "bound")
                    || (item.text.starts_with("shimmering ") && item.text.ends_with(" orb"))
                {
                    continue;
                }
                let types = classify(&item.noun, &item.text);
                // Boxes are the pool's (Stage 4c), not a sale.
                if types.is("box") {
                    continue;
                }
                if !types.types.iter().any(|t| town.sells(t)) {
                    continue;
                }
                out.push((item, types, bag.clone()));
            }
        }
        out
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
        if self.shop.is_none() {
            let Some(shop) = self.shops.pop_front() else {
                self.going_home = true;
                return Step::Walk(self.home);
            };
            // A shop the map does not have here is skipped, as eloot skips it.
            let Some(room) = nearest(shop.tag()) else {
                return self.decide(state, nearest);
            };
            self.shop = Some(shop);
            self.arrived = false;
            self.lots_built = false;
            return Step::Walk(room);
        }
        let shop = self.shop.unwrap_or(Shop::Pawnshop);
        if !self.lots_built {
            self.arrived = true;
            self.build(shop, state);
        }
        if let Some(sack) = self.sacks.pop_front() {
            let phase = if state.right_hand.holds(&sack) || state.left_hand.holds(&sack) {
                SackPhase::Selling
            } else {
                SackPhase::Fetching
            };
            self.sack = Some((sack, phase));
            return self.continue_sack(state).unwrap_or(Step::Done);
        }
        if let Some(lot) = self.lots.pop_front() {
            if let Some(free) = Self::free_a_hand(state) {
                self.lots.push_front(lot);
                return free;
            }
            self.lot = Some((lot.clone(), Doing::Fetching));
            return Step::Fetch(lot.item.id);
        }
        // This shop is done.
        self.shop = None;
        self.decide(state, nearest)
    }

    /// What to sell at `shop`, from the bags as they stand now.
    fn build(&mut self, shop: Shop, state: &GameState) {
        self.lots_built = true;
        let goods = Self::goods(&self.town, state);
        match shop {
            Shop::Gemshop => {
                // A sack of gems sells whole when it has gems and none is
                // excluded (`gemshop`, `eloot.lic:6938`).
                if self.town.sells("gem") {
                    for (bag, items) in Self::bags(&self.town, state) {
                        let gems: Vec<&RoomItem> = items
                            .iter()
                            .filter(|i| classify(&i.noun, &i.text).is("gem"))
                            .collect();
                        if !gems.is_empty() && !gems.iter().any(|g| self.town.excludes(&g.text)) {
                            self.sacks.push_back(bag);
                        }
                    }
                }
                for (item, types, bag) in goods {
                    if self.skipped.contains(&item.id) {
                        continue;
                    }
                    // What a bulk sale takes is left out; what it leaves is
                    // sold one by one when the shop is reached again.
                    if types.is("gem") && self.sacks.contains(&bag) {
                        continue;
                    }
                    if !(types.sells_to("gemshop") || is_thorn_or_berry(&item)) {
                        continue;
                    }
                    let appraise = types.types.iter().any(|t| self.town.appraises(t));
                    self.lots.push_back(Lot {
                        item,
                        bag,
                        appraise,
                        analyze: false,
                    });
                }
            }
            Shop::Pawnshop => {
                for (item, types, bag) in goods {
                    if self.skipped.contains(&item.id) {
                        continue;
                    }
                    let pawn_only = types.sells_to("pawnshop") && !types.sells_to("gemshop");
                    if !(pawn_only || types.is("clothing")) {
                        continue;
                    }
                    let appraise = types
                        .types
                        .iter()
                        .any(|t| self.town.appraises(t) || ALWAYS_APPRAISED.contains(&t.as_str()));
                    let analyze = self.town.keep_transmogs
                        && item.after.is_some()
                        && types
                            .types
                            .iter()
                            .any(|t| TRANSMOG_KINDS.contains(&t.as_str()));
                    self.lots.push_back(Lot {
                        item,
                        bag,
                        appraise,
                        analyze,
                    });
                }
            }
        }
    }

    /// A hand to fetch into: `None` when one is free, else the stow that
    /// frees the right hand into the default bag (`free_hands`, `:3960`).
    fn free_a_hand(state: &GameState) -> Option<Step> {
        if !(state.right_hand.is_holding() && state.left_hand.is_holding()) {
            return None;
        }
        let item = state.right_hand.id()?.to_owned();
        let bag = state.containers.stow(StowSlot::Default)?.id.clone();
        Some(Step::Stow { item, bag })
    }

    fn continue_lot(&mut self, state: &GameState) -> Option<Step> {
        let (lot, doing) = self.lot.clone()?;
        let id = lot.item.id.clone();
        let in_hand = state.right_hand.holds(&id) || state.left_hand.holds(&id);
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
            Doing::Selling => Some(Step::Sell(id)),
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

    /// The item is in hand: analyze, appraise or sell, in eloot's order.
    fn after_fetch(&mut self, lot: Lot) -> Step {
        let id = lot.item.id.clone();
        let doing = if lot.analyze {
            Doing::Analyzing
        } else if lot.appraise {
            Doing::Appraising
        } else {
            Doing::Selling
        };
        let step = match doing {
            Doing::Analyzing => Step::Analyze(id),
            Doing::Appraising => Step::Appraise(id),
            _ => Step::Sell(id),
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
        let in_hand = state.right_hand.holds(&sack) || state.left_hand.holds(&sack);
        match phase {
            SackPhase::Fetching => {
                if in_hand {
                    self.sack = Some((sack.clone(), SackPhase::Selling));
                    return Some(Step::SellSack(sack));
                }
                if let Some(free) = Self::free_a_hand(state) {
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
                if let Some(note) = note_in_hand(state) {
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
                if state.right_hand.holds(&note) || state.left_hand.holds(&note) {
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
                        Doing::Selling
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
                        Some(value) if value > 0 && value <= limit && !refused => Doing::Selling,
                        _ => Doing::Stowing,
                    };
                    self.lot = Some((lot, doing));
                }
            }
            Step::Sell(_) => {
                if let Some((lot, _)) = self.lot.clone() {
                    if sold {
                        self.lot = None;
                    } else if refused {
                        self.lot = Some((lot, Doing::Stowing));
                    } else {
                        // No answer read: the hands decide on the next turn.
                        self.lot = Some((lot, Doing::Stowing));
                    }
                }
            }
            other => self.sack_outcome(&other, sold, refused, replies, state),
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
                if sold || replies.contains(&Reply::SackInspected) {
                    self.sack = Some((sack, SackPhase::Wearing));
                } else if refused {
                    // Nothing in it the shop wants whole: item by item then.
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
                self.sack = self
                    .sack
                    .take()
                    .map(|(sack, _)| (sack, SackPhase::StowingNote(note)));
            }
            Step::Stow { item, .. } => {
                let gone = !(state.right_hand.holds(&item) || state.left_hand.holds(&item));
                if gone {
                    if self
                        .lot
                        .as_ref()
                        .is_some_and(|(lot, _)| lot.item.id == item)
                    {
                        self.lot = None;
                    }
                    if let Some((sack, SackPhase::StowingNote(_))) = &self.sack {
                        let sack = sack.clone();
                        if !(state.right_hand.holds(&sack) || state.left_hand.holds(&sack)) {
                            self.sack = None;
                            self.lots_built = false;
                        }
                    }
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

/// eloot's thorn-and-berry gems the jeweler takes (`:6963`).
fn is_thorn_or_berry(item: &RoomItem) -> bool {
    item.noun.contains("thorn") || item.noun.contains("berry")
}

/// A note, scrip or chit in either hand (`read_note`, `:2445`).
fn note_in_hand(state: &GameState) -> Option<String> {
    [&state.right_hand, &state.left_hand]
        .into_iter()
        .find(|hand| {
            hand.noun()
                .is_some_and(|noun| matches!(noun, "note" | "scrip" | "chit"))
        })
        .and_then(|hand| hand.id().map(str::to_owned))
}
