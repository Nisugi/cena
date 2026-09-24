//! The loot profile: one TOML file per character, as eloot's `eloot.yaml`
//! is per character (`plan/31` §6; the author: *"the eloot settings profile
//! is per character yes"*).
//!
//! There is one looter, so nothing here chooses it. A character with a
//! profile loots by it; one without gets the hunt's bare `loot #id`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::hunt::chain;

/// eloot's own category names (`eloot.lic:683`, `all_loot_categories`),
/// the words `take` may use beside the object-type table's. `breakable` and
/// `lm trap` are in Nisugi's profile and in the type table, not in this list.
pub const ELOOT_CATEGORIES: &[&str] = &[
    "alchemy",
    "armor",
    "box",
    "clothing",
    "collectible",
    "cursed",
    "food",
    "gem",
    "herb",
    "jewelry",
    "junk",
    "lockpick",
    "lm trap",
    "magic",
    "reagent",
    "scroll",
    "skin",
    "uncommon",
    "valuable",
    "wand",
    "weapon",
];

/// What a character takes, leaves and falls back on when looting.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is one eloot switch a player sets; an enum per switch would name nothing the file does not"
)]
pub struct LootProfile {
    /// Object categories worth taking (`loot_types`). A thing of a category
    /// not here is left; a thing of no known category is taken.
    pub take: Vec<String>,
    /// Names, or words in names, never taken (`loot_exclude`).
    pub leave: Vec<String>,
    /// Go defensive to loot (`loot_defensive`).
    pub defensive: bool,
    /// The disk is a container when the bags are full (`use_disk`).
    pub disk: bool,
    /// Cast Sigil of Determination when a corpse is *not in any condition*
    /// to be searched (`sigil_determination_on_fail`).
    pub sigil_on_fail: bool,
    /// Phase (704) a box before stowing it (`loot_phase`).
    pub phase_boxes: bool,
    /// Containers tried, by name and in order, when the stow list's bag and
    /// the default are full (`overflow_containers`).
    pub overflow: Vec<String>,
    /// Names learned to crumble when stowed; left where they lie.
    pub crumbly: Vec<String>,
    /// Names the game refused to let this character hold; left.
    pub unlootable: Vec<String>,
    /// Containers that close themselves; opened before a drag (`auto_close`).
    pub autoclose: Vec<String>,
    /// The selling, hoarding and banking keys, carried verbatim for the rest
    /// phase's errands (`plan/31` §4, Stage 4). Nothing reads them yet.
    #[serde(skip_serializing_if = "toml::Table::is_empty")]
    pub town: toml::Table,
}

impl LootProfile {
    /// Read a profile.
    ///
    /// # Errors
    ///
    /// Not TOML, or a key this profile does not have.
    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    /// Write the profile as TOML.
    ///
    /// # Errors
    ///
    /// A value TOML cannot hold, which no field here produces.
    pub fn to_toml(&self) -> Result<String, String> {
        toml::to_string_pretty(self).map_err(|e| e.to_string())
    }

    /// What is wrong with it: a `take` word that names no category the
    /// object-type table or eloot knows. Empty when nothing is.
    #[must_use]
    pub fn problems(&self) -> Vec<String> {
        let known = cena_session::gameobj::categories(cena_session::gameobj::Classification::Type);
        self.take
            .iter()
            .filter(|word| {
                !known.contains(word.as_str()) && !ELOOT_CATEGORIES.contains(&word.as_str())
            })
            .map(|word| format!("take: {word:?} names no kind of object the game types"))
            .collect()
    }

    /// Is this category wanted?
    #[must_use]
    pub fn takes(&self, category: &str) -> bool {
        self.take.iter().any(|word| word == category)
    }
}

/// The character's loot profile: `<data>/hunt/loot/<instance>_<character>.toml`,
/// beside the hunt chain's files. `None` when the names cannot be a file
/// name.
#[must_use]
pub fn path(dir: &Path, instance: &str, character: &str) -> Option<PathBuf> {
    let file = chain::file_name(&format!("{instance}_{character}"))?;
    Some(dir.join("hunt").join("loot").join(format!("{file}.toml")))
}
