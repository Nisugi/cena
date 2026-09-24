//! Is this thing on the floor worth taking?
//!
//! eloot answers in two steps (`plan/31` §2b): `reject_invalid_loot`
//! (`eloot.lic:5582-5596`) throws out what is not loot at all, then
//! `should_grab_item?` (`:5600-5609`) asks whether what is left is wanted.
//! Both are one function here, and the reason a thing is left is named, so
//! a hunt can say it.

use cena_session::RoomItem;
use cena_session::containers::StowSlot;
use cena_session::gameobj::{ObjectTypes, classify};

use super::profile::LootProfile;

/// Names of things on the floor that are not loot (`eloot.lic:633-667`):
/// deity spell effects, ranger vines, and the fixtures a room describes as
/// objects. Matched as whole words, case-insensitively.
pub const NOT_LOOT_NAMES: &[&str] = &[
    "golden light",
    "jet black scimitar",
    "midnight black flames",
    "reddish haze",
    "sourceless shadow",
    "swirling blue-green pillar of water",
    "bramble",
    "briar",
    "clutch of twisted branches",
    "creeper",
    "ivy",
    "smilax",
    "swallowwort",
    "tumbleweed",
    "vine",
    "widgeonweed",
    "child",
    "jagged crater",
    "massive icicle",
    "point of elemental instability",
    "rolton droppings",
    "rotting tree stump",
    "sealed fissure",
    "severed",
    "slender silvery thread",
    "slippery wooden chute",
    "small puddle",
    "vathor club",
];

/// Nouns of things on the floor that are not loot (`eloot.lic:669-681`).
pub const NOT_LOOT_NOUNS: &[&str] = &[
    "cloud",
    "cyclone",
    "door",
    "gangplank",
    "kitten",
    "maw",
    "mist",
    "muck",
    "puppy",
    "space",
    "staircase",
];

/// The kinds `loot #id` puts straight into the stow list's bag
/// (`eloot.lic:5237`, `loot_cmd_items`); anything else is dragged.
pub const LOOTABLE_BY_VERB: &[&str] = &[
    "clothing",
    "jewelry",
    "gem",
    "herb",
    "skin",
    "wand",
    "scroll",
    "potion",
    "reagent",
    "trinket",
    "lockpick",
    "treasure",
    "forageable",
];

/// Whether a thing is taken, and if not, why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Take it; its kinds, for choosing the bag and the verb.
    Take(ObjectTypes),
    /// Leave it, for this reason.
    Leave(&'static str),
}

/// eloot's answer for one thing on the floor.
#[must_use]
pub fn verdict(item: &RoomItem, profile: &LootProfile) -> Verdict {
    if item.id.starts_with('-') {
        return Verdict::Leave("part of the room");
    }
    let name = item.text.as_str();
    if NOT_LOOT_NAMES.iter().any(|word| has_word(name, word)) {
        return Verdict::Leave("not loot");
    }
    if NOT_LOOT_NOUNS
        .iter()
        .any(|noun| item.noun.eq_ignore_ascii_case(noun))
    {
        return Verdict::Leave("not loot");
    }
    // `Nisugi disk`: a player's disk is a capitalised name and the noun.
    if item.noun == "disk" && name.chars().next().is_some_and(char::is_uppercase) {
        return Verdict::Leave("someone's disk");
    }
    if profile.unlootable.iter().any(|known| known == name) {
        return Verdict::Leave("could not be held before");
    }
    if profile.crumbly.iter().any(|known| known == name) {
        return Verdict::Leave("crumbles when stowed");
    }
    if profile.leave.iter().any(|word| has_word(name, word)) {
        return Verdict::Leave("excluded by name");
    }
    let types = classify(&item.noun, name);
    if (types.is("weapon") || types.is("armor")) && !types.is("uncommon") && !types.is("clothing") {
        return Verdict::Leave("a weapon or armor");
    }
    if types.is("cursed") && !profile.takes("cursed") {
        return Verdict::Leave("cursed");
    }
    if types.types.iter().any(|kind| profile.takes(kind)) {
        return Verdict::Take(types);
    }
    if types
        .types
        .iter()
        .any(|kind| super::profile::ELOOT_CATEGORIES.contains(&kind.as_str()))
    {
        return Verdict::Leave("not a wanted kind");
    }
    // Of no category eloot knows: taken, as eloot does (`:5608`).
    Verdict::Take(types)
}

/// The stow list's slot for a thing of these kinds: the first kind that
/// names a slot, else the default.
#[must_use]
pub fn stow_slot(types: &ObjectTypes) -> StowSlot {
    types
        .types
        .iter()
        .find_map(|kind| StowSlot::parse(kind))
        .unwrap_or(StowSlot::Default)
}

/// Does `loot #id` put this straight into its bag?
#[must_use]
pub fn lootable_by_verb(types: &ObjectTypes) -> bool {
    types
        .types
        .iter()
        .any(|kind| LOOTABLE_BY_VERB.contains(&kind.as_str()))
}

/// `word` occurs in `text` as whole words, case-insensitively: eloot's
/// `build_word_boundary_regexes` (`eloot.lic:692`).
fn has_word(text: &str, word: &str) -> bool {
    let text = text.to_ascii_lowercase();
    let word = word.to_ascii_lowercase();
    text.match_indices(&word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
    })
}
