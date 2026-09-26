//! Stocking the herb container: eherbs' `stock` and `fill` (`eherbs.lic:
//! 1642-1896`, `:2214-2302`; `plan/36` Stage 4). Pure, as the healer is.
//!
//! eherbs counts the doses it holds of each kind against a minimum, buys
//! what is short at the nearest herbalist, and goes home:
//!
//! - **The minimums** are eherbs' own table, scaled by the profile's stock
//!   percent; in a Survivalist's Kit every kind's minimum is the kit's
//!   capacity, `tier * 25 + 25`, scaled the same way (`:3008-3022`).
//! - **The count** is each herb's doses as the dose monitor measured them; a
//!   herb never measured is fetched, measured and put back first. A kit's
//!   listing states its counts, and a kit is stocked once for what it drinks
//!   and once for what it eats.
//! - **What is bought** for a kind is the first herb the table says this
//!   town's herbalist sells for it, in purchases of its store doses:
//!   `(minimum - held) / store doses`, rounded down, eherbs' arithmetic.
//! - **Buying** is `order <n> <number>` from the herbalist's menu, ten at a
//!   time, then `buy`; a package is opened, emptied into the container and
//!   thrown away. Too little silver sends the round to the bank for the
//!   price and a tenth again, and back.
//!
//! `fill` is the same round with a minimum of one purchase of each kind the
//! container has none of.
//!
//! Not ported: bundling the bought herbs together (eherbs' `bundle_all`);
//! they sit in the container as they came, which the healer does not mind.

use std::collections::VecDeque;

use cena_map::RoomId;
use cena_session::herbs::{self, Herb, HerbKind};
use cena_session::{GameState, RoomItem};

use super::profile::HealProfile;
use super::reply::Reply;

/// Purchases per `order`.
const BATCH: u32 = 10;
/// Silver fetched when the price is not known (`withdraw_amount`, `:376`).
const WITHDRAW: u64 = 8_000;

/// eherbs' minimum doses by kind (`@@min_stock_doses`, `:146-167`).
fn base_minimum(kind: &str) -> u32 {
    match kind {
        "major head scar" | "major organ scar" => 6,
        "minor head wound" | "major nerve wound" | "minor organ scar" => 4,
        "missing eye" => 7,
        "blood" | "minor blood" => 50,
        "major blood" => 20,
        _ => 25,
    }
}

/// A kind as eherbs counts it: a herb kind, with blood split in two when
/// the profile splits it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bucket {
    /// Any kind but blood, or all blood.
    Kind(HerbKind),
    /// Yabathilium-class blood.
    MajorBlood,
    /// Acantha-class blood.
    MinorBlood,
}

impl Bucket {
    fn word(self) -> String {
        match self {
            Self::Kind(kind) => kind.to_string(),
            Self::MajorBlood => "major blood".to_owned(),
            Self::MinorBlood => "minor blood".to_owned(),
        }
    }

    fn holds(self, herb: &Herb) -> bool {
        match self {
            Self::Kind(kind) => herb.kind == kind,
            Self::MajorBlood => herb.is_major_blood(),
            Self::MinorBlood => herb.kind == HerbKind::Blood && !herb.is_major_blood(),
        }
    }
}

/// What to buy of one kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Want {
    /// The kind.
    pub bucket: Bucket,
    /// The herb this town sells for it.
    pub herb: &'static Herb,
    /// Purchases.
    pub count: u32,
}

/// One command for the driver, or the end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Walk there.
    Walk(RoomId),
    /// `get #id`.
    Fetch(String),
    /// `measure #id`.
    Measure(String),
    /// Put one thing in one bag.
    Stow {
        /// The item's id.
        item: String,
        /// The bag's id.
        bag: String,
    },
    /// `order`: the herbalist's menu.
    Menu,
    /// `order <count> <number>`.
    Order {
        /// How many.
        count: u32,
        /// The menu number.
        number: u32,
    },
    /// `buy`.
    Buy,
    /// `open #id`: a package.
    Open(String),
    /// `empty #from in #into`: a package into the container.
    Empty {
        /// The package.
        from: String,
        /// The container.
        into: String,
    },
    /// `throw #id`: the empty package.
    Throw(String),
    /// `withdraw N silver`.
    Withdraw(u64),
    /// The round is over.
    Done(Stocked),
}

/// How a stocking round ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stocked {
    /// Everything short was bought, or nothing was.
    Done {
        /// Purchases made.
        bought: u32,
    },
    /// The map has no herbalist to walk to.
    NoHerbalist,
    /// The herbalist's menu does not list a herb the table says it sells.
    NotOnMenu(&'static str),
    /// The bank would not cover it.
    NoSilver,
}

/// Where the round is.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Phase {
    Measuring,
    ToShop,
    Menu,
    Buying,
    Unpacking(String),
    /// A herb bought loose, into the container.
    Storing(String),
    ToBank,
    Withdrawing,
    Home,
    Over(Stocked),
}

/// The stocker for one round.
#[derive(Clone, Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "four independent facts about the round, each read in one place"
)]
pub struct Stocker {
    profile: HealProfile,
    sack: String,
    home: RoomId,
    shop: Option<RoomId>,
    bank: Option<RoomId>,
    location: String,
    fill: bool,
    phase: Phase,
    measure: VecDeque<String>,
    measuring: Option<(String, u8)>,
    wants: VecDeque<Want>,
    /// Purchases still to make of the want at the front.
    left: u32,
    price: Option<u64>,
    bought: u32,
    withdrew: bool,
    menu_asked: bool,
    /// The wants are counted once, before the first walk to the shop.
    planned: bool,
    last: Option<Step>,
}

impl Stocker {
    /// A round for `sack` (the herb container's id), at the herbalist in
    /// `shop` whose town is `location`, the bank in `bank`, home at `home`.
    /// `fill` buys one of each kind the container lacks, as eherbs' `fill`.
    #[must_use]
    pub fn new(
        profile: HealProfile,
        state: &GameState,
        sack: String,
        places: (Option<RoomId>, Option<RoomId>, RoomId),
        location: String,
        fill: bool,
    ) -> Self {
        let (shop, bank, home) = places;
        let kit = state.kits.analysis(&sack).is_some_and(|a| a.is_kit);
        let measure = if kit {
            VecDeque::new()
        } else {
            held(state, &sack)
                .into_iter()
                .filter(|item| {
                    herbs::herb_for(&item.text).is_some() && state.doses.left(&item.id).is_none()
                })
                .map(|item| item.id)
                .collect()
        };
        Self {
            profile,
            sack,
            home,
            shop,
            bank,
            location,
            fill,
            phase: Phase::Measuring,
            measure,
            measuring: None,
            wants: VecDeque::new(),
            left: 0,
            price: None,
            bought: 0,
            withdrew: false,
            menu_asked: false,
            planned: false,
            last: None,
        }
    }

    /// What this round would buy, as things stand: the kinds short, the herb
    /// for each, the purchases.
    #[must_use]
    pub fn wants(&self, state: &GameState) -> Vec<Want> {
        shopping(&self.profile, state, &self.sack, &self.location, self.fill)
    }

    /// The next command.
    pub fn next(&mut self, state: &GameState) -> Step {
        let step = self.decide(state);
        self.last = Some(step.clone());
        step
    }

    fn decide(&mut self, state: &GameState) -> Step {
        match self.phase.clone() {
            Phase::Measuring => self.measuring(state),
            Phase::ToShop => {
                let Some(shop) = self.shop else {
                    self.phase = Phase::Over(Stocked::NoHerbalist);
                    return self.decide(state);
                };
                if !self.planned {
                    self.planned = true;
                    self.wants = self.wants(state).into();
                    if self.wants.is_empty() {
                        self.phase = Phase::Home;
                        return self.decide(state);
                    }
                    self.left = self.wants.front().map_or(0, |w| w.count);
                }
                self.phase = Phase::Menu;
                Step::Walk(shop)
            }
            Phase::Menu => {
                if state.order_menu.is_empty() && !self.menu_asked {
                    self.menu_asked = true;
                    return Step::Menu;
                }
                self.phase = Phase::Buying;
                self.decide(state)
            }
            Phase::Buying => self.buying(state),
            Phase::Unpacking(package) => self.unpacking(state, &package),
            Phase::Storing(item) => {
                self.phase = Phase::Buying;
                if state.right_hand.holds(&item) || state.left_hand.holds(&item) {
                    return Step::Stow {
                        item,
                        bag: self.sack.clone(),
                    };
                }
                self.decide(state)
            }
            Phase::ToBank => {
                let Some(bank) = self.bank else {
                    self.phase = Phase::Over(Stocked::NoSilver);
                    return self.decide(state);
                };
                self.phase = Phase::Withdrawing;
                Step::Walk(bank)
            }
            Phase::Withdrawing => {
                self.withdrew = true;
                let amount = self.price.map_or(WITHDRAW, |price| {
                    let batch = u64::from(self.left.min(BATCH));
                    price * batch + price * batch / 10
                });
                // Back to the shop, for the same wants.
                self.phase = Phase::ToShop;
                Step::Withdraw(amount)
            }
            Phase::Home => {
                self.phase = Phase::Over(Stocked::Done {
                    bought: self.bought,
                });
                Step::Walk(self.home)
            }
            Phase::Over(how) => Step::Done(how),
        }
    }

    /// Every unmeasured herb fetched, measured and put back.
    fn measuring(&mut self, state: &GameState) -> Step {
        if let Some((id, stage)) = self.measuring.clone() {
            let in_hand = state.right_hand.holds(&id) || state.left_hand.holds(&id);
            match stage {
                0 if in_hand => {
                    self.measuring = Some((id.clone(), 1));
                    return Step::Measure(id);
                }
                1 if in_hand => {
                    self.measuring = Some((id.clone(), 2));
                    return Step::Stow {
                        item: id,
                        bag: self.sack.clone(),
                    };
                }
                _ => self.measuring = None,
            }
        }
        if let Some(id) = self.measure.pop_front() {
            self.measuring = Some((id.clone(), 0));
            return Step::Fetch(id);
        }
        self.phase = Phase::ToShop;
        self.decide(state)
    }

    /// The want at the front: ordered and bought in tens.
    fn buying(&mut self, state: &GameState) -> Step {
        if self.left == 0 {
            self.wants.pop_front();
            let Some(next) = self.wants.front() else {
                self.phase = Phase::Home;
                return self.decide(state);
            };
            self.left = next.count;
            self.price = None;
        }
        let Some(want) = self.wants.front() else {
            self.phase = Phase::Home;
            return self.decide(state);
        };
        let number = state
            .order_menu
            .number(want.herb.name)
            .or_else(|| state.order_menu.number(want.herb.short_name));
        let Some(number) = number else {
            self.phase = Phase::Over(Stocked::NotOnMenu(want.herb.name));
            return Step::Walk(self.home);
        };
        match &self.last {
            Some(Step::Order { .. }) => Step::Buy,
            _ => Step::Order {
                count: self.left.min(BATCH),
                number,
            },
        }
    }

    /// A package opened, emptied into the container, thrown away.
    fn unpacking(&mut self, state: &GameState, package: &str) -> Step {
        let in_hand = state.right_hand.holds(package) || state.left_hand.holds(package);
        match &self.last {
            Some(Step::Buy) if in_hand => Step::Open(package.to_owned()),
            Some(Step::Open(_)) if in_hand => Step::Empty {
                from: package.to_owned(),
                into: self.sack.clone(),
            },
            Some(Step::Empty { .. }) if in_hand => Step::Throw(package.to_owned()),
            _ => {
                self.phase = Phase::Buying;
                self.buying(state)
            }
        }
    }

    /// What the game said to the last step.
    pub fn outcome(&mut self, replies: &[Reply], state: &GameState) {
        let Some(last) = self.last.clone() else {
            return;
        };
        match last {
            Step::Order { .. } => {
                if let Some(price) = replies.iter().find_map(|r| match r {
                    Reply::Price(n) => Some(*n),
                    _ => None,
                }) {
                    self.price = Some(price);
                }
            }
            Step::Buy => {
                if replies.contains(&Reply::NotEnough) {
                    if self.withdrew {
                        self.phase = Phase::Over(Stocked::NoSilver);
                    } else {
                        self.phase = Phase::ToBank;
                    }
                    return;
                }
                if replies.contains(&Reply::Sold) {
                    let batch = self.left.min(BATCH);
                    self.left -= batch;
                    self.bought += batch;
                    self.withdrew = false;
                }
                // A package in hand is unpacked; a herb in hand is stored.
                if let Some(id) = held_package(state) {
                    self.phase = Phase::Unpacking(id);
                } else if let Some(id) = held_herb(state) {
                    self.phase = Phase::Storing(id);
                }
            }
            _ => {}
        }
    }
}

/// What the container holds, as the inventory lists it.
fn held(state: &GameState, sack: &str) -> Vec<RoomItem> {
    state
        .inventory
        .container(sack)
        .map(|c| c.items.clone())
        .unwrap_or_default()
}

/// A package in a hand, by id.
fn held_package(state: &GameState) -> Option<String> {
    [&state.right_hand, &state.left_hand]
        .into_iter()
        .find(|hand| hand.noun() == Some("package"))
        .and_then(|hand| hand.id().map(str::to_owned))
}

/// A herb eherbs knows in a hand, by id.
fn held_herb(state: &GameState) -> Option<String> {
    [&state.right_hand, &state.left_hand]
        .into_iter()
        .find(|hand| hand.name().is_some_and(|n| herbs::herb_for(n).is_some()))
        .and_then(|hand| hand.id().map(str::to_owned))
}

/// The kinds eherbs stocks, in its order (`:1717-1719`): blood, split or
/// not, then every wound and scar, the severed limb and the missing eye.
fn buckets(split_blood: bool) -> Vec<Bucket> {
    let mut out = if split_blood {
        vec![Bucket::MinorBlood, Bucket::MajorBlood]
    } else {
        vec![Bucket::Kind(HerbKind::Blood)]
    };
    out.extend(
        HerbKind::all()
            .into_iter()
            .filter(|k| {
                matches!(
                    k,
                    HerbKind::Injury { .. } | HerbKind::SeveredLimb | HerbKind::MissingEye
                )
            })
            .map(Bucket::Kind),
    );
    out
}

/// The minimum doses of a kind: eherbs' table, or a kit's capacity, scaled
/// by the profile's stock percent when it has one (`:3005-3022`).
fn minimum(profile: &HealProfile, bucket: Bucket, kit_tier: Option<u32>) -> u32 {
    let base = kit_tier.map_or_else(|| base_minimum(&bucket.word()), |tier| tier * 25 + 25);
    match profile.stock {
        Some(percent) if percent > 0 => base * percent.min(100) / 100,
        _ => base,
    }
}

/// What is short, and what this town sells for it (`get_current_stock`,
/// `:2214-2302`).
fn shopping(
    profile: &HealProfile,
    state: &GameState,
    sack: &str,
    location: &str,
    fill: bool,
) -> Vec<Want> {
    let kit = state.kits.analysis(sack).filter(|a| a.is_kit);
    let tier = kit.and_then(|a| a.tier);
    // Each herb held, with its doses and whether it is drunk.
    let holding: Vec<(&'static Herb, u32, bool)> = match (kit, state.kits.listing(sack)) {
        (Some(_), Some(listing)) => listing
            .iter()
            .filter_map(|h| herbs::herb_for(&h.item.text).map(|herb| (herb, h.count, h.liquid)))
            .collect(),
        _ => held(state, sack)
            .iter()
            .filter_map(|item| {
                let herb = herbs::herb_for(&item.text)?;
                let doses = state.doses.left(&item.id).unwrap_or(0);
                Some((herb, doses, herbs::is_drinkable(&item.text)))
            })
            .collect(),
    };
    // A kit is stocked for each form; anything else, once.
    let forms: &[Option<bool>] = if kit.is_some() {
        &[Some(true), Some(false)]
    } else {
        &[None]
    };
    let mut out = Vec::new();
    for form in forms {
        for bucket in buckets(profile.split_blood) {
            let sold = herbs::sold_in(location).find(|herb| {
                bucket.holds(herb) && form.is_none_or(|liquid| herb.is_drinkable() == liquid)
            });
            let Some(herb) = sold else { continue };
            let have: u32 = holding
                .iter()
                .filter(|(h, _, liquid)| bucket.holds(h) && form.is_none_or(|f| *liquid == f))
                .map(|(_, doses, _)| *doses)
                .sum();
            let count = if fill {
                u32::from(!holding.iter().any(|(h, _, _)| bucket.holds(h)))
            } else {
                minimum(profile, bucket, tier).saturating_sub(have) / herb.store_doses.max(1)
            };
            if count > 0 {
                out.push(Want {
                    bucket,
                    herb,
                    count,
                });
            }
        }
    }
    out
}
