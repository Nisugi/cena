//! What the selling bags hold, and what each shop takes of it.
//!
//! eloot's `check_items` (`eloot.lic:6472`) decides which shops a round
//! visits; each shop's own method (`gemshop`, `furrier`, `pawnshop`,
//! `collectibles`, `gold_rings`) then walks the bags again for what it takes.
//! Both readings are here, over the state as it stands when the shop is
//! reached, so what an earlier shop sold is not offered twice.

use std::collections::BTreeSet;

use cena_session::containers::{ReadySlot, StowSlot};
use cena_session::gameobj::{ObjectTypes, classify};
use cena_session::{GameState, RoomItem};

use super::settings::Town;

/// A shop a round visits, in the order it visits them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Shop {
    /// The locksmith pool, first (`process_boxes` before `go_sell`):
    /// tagged `locksmith pool`.
    Pool,
    /// The Chronomage's office: gold rings given to its clerk.
    Chronomage,
    /// Skins and reagents: tagged `furrier`.
    Furrier,
    /// Gems, and what the jeweler buys: tagged `gemshop`.
    Gemshop,
    /// Everything else: tagged `pawnshop`.
    Pawnshop,
    /// The collectibles counter: tagged `collectibles` or `collectible`.
    Collectibles,
    /// The bank, last: the round's silver deposited (`silver_deposit`).
    Bank,
}

impl Shop {
    /// The map tag the room carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        self.tags()[0]
    }

    /// Every tag a room for this shop may carry, eloot's preferred first.
    #[must_use]
    pub const fn tags(self) -> &'static [&'static str] {
        match self {
            Self::Pool => &["locksmith pool"],
            Self::Chronomage => &["chronomage"],
            Self::Furrier => &["furrier"],
            Self::Gemshop => &["gemshop"],
            Self::Pawnshop => &["pawnshop"],
            Self::Collectibles => &["collectibles", "collectible"],
            Self::Bank => &["bank"],
        }
    }
}

/// How a lot leaves the character.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum How {
    /// `sell #id`, appraised or analyzed first as the lot says.
    Sell,
    /// `deposit #id`, at the collectibles counter.
    Deposit,
    /// `give #id to #npc`, the Chronomage's clerk.
    Give(String),
    /// A bundle of skins: `bundle remove` and sell each skin.
    Unbundle,
}

/// One item to part with at the current shop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Lot {
    pub(super) item: RoomItem,
    /// The bag it came from, to go back to.
    pub(super) bag: String,
    pub(super) appraise: bool,
    pub(super) analyze: bool,
    pub(super) how: How,
}

/// Which categories eloot analyzes for a transmog (`is_transmog?`,
/// `eloot.lic:4090`).
const TRANSMOG_KINDS: &[&str] = &["jewelry", "clothing", "armor", "weapon", "uncommon"];
/// Categories the pawnshop appraises whatever the profile says (`:7522`).
const ALWAYS_APPRAISED: &[&str] = &["uncommon", "weapon", "armor"];
/// The nouns a Chronomage's clerk goes by (`gold_rings`, `:7130`).
const CLERKS: &[&str] = &[
    "clerk",
    "agent",
    "halfling",
    "scallywag",
    "dwarf",
    "woman",
    "attendant",
    "guard",
];
/// eloot's gold rings (`regex_gold_rings`, `:638`): a plain `gold ring`, or
/// one of these words before it.
const RING_WORDS: &[&str] = &[
    "dingy",
    "plain",
    "braided",
    "twisted",
    "intricate",
    "large",
    "thin",
    "wide",
    "polished",
    "scratched",
    "thick",
    "dull",
    "faded",
    "small",
    "flawless",
    "inlaid",
    "dirt-caked",
    "ornate",
    "exquisite",
    "shiny",
    "bright",
    "narrow",
];

/// A gold ring the Chronomage takes, by eloot's exact names.
#[must_use]
pub fn is_gold_ring(name: &str) -> bool {
    match name.strip_suffix("gold ring") {
        Some("") => true,
        Some(rest) => rest
            .strip_suffix(' ')
            .is_some_and(|word| RING_WORDS.contains(&word)),
        None => false,
    }
}

/// eloot's thorn-and-berry gems the jeweler takes (`:6963`).
fn is_thorn_or_berry(item: &RoomItem) -> bool {
    item.noun.contains("thorn") || item.noun.contains("berry")
}

/// The bags sold from, by the profile's `sell_container` slots, with their
/// contents as the inventory lists them.
pub(super) fn bags(town: &Town, state: &GameState) -> Vec<(String, Vec<RoomItem>)> {
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

/// Whether the profile parts with this at some shop: a sold category, a
/// collectible when collectibles are handed in, a gold ring when the
/// Chronomage takes them.
fn wanted(town: &Town, item: &RoomItem, types: &ObjectTypes) -> bool {
    (town.gold_rings && is_gold_ring(&item.text))
        || (town.collectibles && types.is("collectible"))
        || types.types.iter().any(|t| town.sells(t))
        || (types.sells_to("furrier") && (town.sells("skin") || town.sells("reagent")))
}

/// Everything the round may part with: the item, its types, its bag.
pub(super) fn goods(town: &Town, state: &GameState) -> Vec<(RoomItem, ObjectTypes, String)> {
    let ready: BTreeSet<String> = ReadySlot::ALL
        .iter()
        .filter_map(|slot| state.containers.ready(*slot))
        .map(|item| item.id.clone())
        .collect();
    let mut out = Vec::new();
    for (bag, items) in bags(town, state) {
        for item in items {
            if ready.contains(&item.id)
                || town.excludes(&item.text)
                || item.text.split(' ').any(|w| w == "bound")
                || (item.text.starts_with("shimmering ") && item.text.ends_with(" orb"))
            {
                continue;
            }
            let types = classify(&item.noun, &item.text);
            // Boxes are the pool's, and sold only when the profile sells
            // them (`check_items`, `:6503`).
            if (types.is("box") && !town.sells("box")) || !wanted(town, &item, &types) {
                continue;
            }
            out.push((item, types, bag.clone()));
        }
    }
    out
}

/// The shop one item goes to, by `check_items`' order: a gold ring to the
/// Chronomage, a collectible to its counter, then the shop the object data
/// says buys it. `None` when no shop this stage visits takes it.
pub(super) fn shop_for(town: &Town, item: &RoomItem, types: &ObjectTypes) -> Option<Shop> {
    if town.gold_rings && is_gold_ring(&item.text) {
        return Some(Shop::Chronomage);
    }
    if types.is("collectible") {
        return town.collectibles.then_some(Shop::Collectibles);
    }
    if types.is("box") {
        return town.sells("box").then_some(Shop::Pawnshop);
    }
    if types.sells_to("furrier") {
        return Some(Shop::Furrier);
    }
    if types.sells_to("gemshop") || (types.is("gem") && is_thorn_or_berry(item)) {
        return Some(Shop::Gemshop);
    }
    if types.sells_to("pawnshop") || types.is("clothing") {
        return Some(Shop::Pawnshop);
    }
    None
}

/// The room's clerk for the Chronomage, by eloot's nouns: the NPCs first,
/// then the room's objects.
pub(super) fn clerk(state: &GameState) -> Option<String> {
    state
        .room
        .creatures
        .iter()
        .chain(&state.room.objects)
        .find(|thing| CLERKS.contains(&thing.noun.as_str()))
        .map(|thing| thing.id.clone())
}

/// The bags that sell whole at `shop`: at the gem shop a bag with gems and
/// no excluded gem (`gemshop`, `:6938`); at the furrier a bag with furrier
/// goods and none excluded (`furrier`, `:6858`). A bag already sold whole
/// this round is not offered again.
pub(super) fn sacks(
    shop: Shop,
    town: &Town,
    state: &GameState,
    sold: &BTreeSet<String>,
) -> Vec<String> {
    let takes = |item: &RoomItem| {
        let types = classify(&item.noun, &item.text);
        match shop {
            Shop::Gemshop => town.sells("gem") && types.is("gem"),
            Shop::Furrier => {
                (town.sells("skin") || town.sells("reagent")) && types.sells_to("furrier")
            }
            _ => false,
        }
    };
    bags(town, state)
        .into_iter()
        .filter(|(bag, items)| {
            !sold.contains(bag)
                && items.iter().any(takes)
                && !items.iter().any(|i| takes(i) && town.excludes(&i.text))
        })
        .map(|(bag, _)| bag)
        .collect()
}

/// Whether `shop` takes this item: its own shop by [`shop_for`], or, at the
/// pawnshop, clothing the gem shop has passed by.
fn takes(shop: Shop, town: &Town, item: &RoomItem, types: &ObjectTypes) -> bool {
    let home = shop_for(town, item, types);
    home == Some(shop)
        || (shop == Shop::Pawnshop && types.is("clothing") && home == Some(Shop::Gemshop))
}

/// What to part with at `shop`, one lot at a time, leaving out what the
/// round has given up on and the bags about to sell whole.
pub(super) fn lots(
    shop: Shop,
    town: &Town,
    state: &GameState,
    skipped: &BTreeSet<String>,
    whole: &[String],
) -> Vec<Lot> {
    let clerk = clerk(state);
    let mut out = Vec::new();
    for (item, types, bag) in goods(town, state) {
        if skipped.contains(&item.id) || !takes(shop, town, &item, &types) {
            continue;
        }
        // What a bulk sale takes is left out; what it leaves is sold one by
        // one when the shop is built again.
        if whole.contains(&bag) && matches!(shop, Shop::Gemshop | Shop::Furrier) {
            continue;
        }
        let how = match shop {
            Shop::Collectibles => How::Deposit,
            Shop::Chronomage => match &clerk {
                Some(npc) => How::Give(npc.clone()),
                None => continue,
            },
            Shop::Furrier if item.text.contains("bundle") => How::Unbundle,
            _ => How::Sell,
        };
        let sells = how == How::Sell;
        let appraise = sells
            && types.types.iter().any(|t| {
                town.appraises(t)
                    || (shop == Shop::Pawnshop && ALWAYS_APPRAISED.contains(&t.as_str()))
            });
        let analyze = sells
            && shop == Shop::Pawnshop
            && town.keep_transmogs
            && item.after.is_some()
            && types
                .types
                .iter()
                .any(|t| TRANSMOG_KINDS.contains(&t.as_str()));
        out.push(Lot {
            item,
            bag,
            appraise,
            analyze,
            how,
        });
    }
    out
}

/// This character's own disk in the room, by id: a `disk` whose name begins
/// with the character's name.
pub(super) fn own_disk(state: &GameState) -> Option<String> {
    let name = state.character.name.as_deref()?;
    state
        .room
        .objects
        .iter()
        .find(|item| item.noun == "disk" && item.text.starts_with(name))
        .map(|item| item.id.clone())
}

/// A note, scrip or chit in either hand (`read_note`, `:2445`).
pub(super) fn note_in_hand(state: &GameState) -> Option<String> {
    [&state.right_hand, &state.left_hand]
        .into_iter()
        .find(|hand| hand.noun().is_some_and(is_note))
        .and_then(|hand| hand.id().map(str::to_owned))
}

/// A note, scrip or chit in the default bag (`silver_deposit`, `:3441`).
pub(super) fn note_in_bag(state: &GameState) -> bool {
    state
        .containers
        .stow(StowSlot::Default)
        .and_then(|bag| state.inventory.container(&bag.id))
        .is_some_and(|bag| bag.items.iter().any(|i| is_note(&i.noun)))
}

fn is_note(noun: &str) -> bool {
    matches!(noun, "note" | "scrip" | "chit")
}
