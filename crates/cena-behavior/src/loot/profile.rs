//! The loot profile: one TOML file per character, as eloot's `eloot.yaml`
//! is per character (`plan/31` §6; the author: *"the eloot settings profile
//! is per character yes"*).
//!
//! There is one looter, so nothing here chooses it. A character with a
//! profile loots by it; one without gets the hunt's bare `loot #id`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

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
    /// Skinning, when the profile turns it on (`skin_enable` and the
    /// `skin_*` keys; `plan/31` §5).
    #[serde(default, skip_serializing_if = "Skin::is_off")]
    pub skin: Skin,
    /// The selling, hoarding and banking keys, carried verbatim for the rest
    /// phase's errands (`plan/31` §4, Stage 4), which read them as
    /// [`crate::town::Town`].
    #[serde(skip_serializing_if = "toml::Table::is_empty")]
    pub town: toml::Table,
}

/// How corpses are skinned, eloot's Skinning tab (`eloot.lic:937-947`,
/// `:2061-2072`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "eloot's five switches, carried as they are; a profile is a table of switches"
)]
pub struct Skin {
    /// Skin at all (`skin_enable`).
    pub enable: bool,
    /// Kneel before skinning (`skin_kneel`).
    pub kneel: bool,
    /// Keep Bravery (604) up while skinning (`skin_604`).
    pub spell_604: bool,
    /// Cast Sigil of Resolve before skinning (`skin_resolve`).
    pub resolve: bool,
    /// Skin only the creature a skinning bounty names, and nothing without
    /// one (`skin_bounty_only`).
    pub bounty_only: bool,
    /// The edged skinning weapon, by a word of its name (`skin_weapon`);
    /// empty means whatever is in the right hand.
    pub weapon: String,
    /// Where the edged weapon goes back (`skin_sheath`); empty means the
    /// default bag.
    pub sheath: String,
    /// The blunt skinning weapon (`skin_weapon_blunt`); empty means the
    /// blunt-skinned creatures are left.
    pub weapon_blunt: String,
    /// Where the blunt weapon goes back (`skin_sheath_blunt`).
    pub sheath_blunt: String,
    /// Names, or words in names, never skinned (`skin_exclude`).
    pub exclude: Vec<String>,
    /// Creatures the game said cannot be skinned, learned (`unskinnable`).
    pub unskinnable: Vec<String>,
}

impl Skin {
    /// Nothing set: the default, left out of the file.
    #[must_use]
    pub fn is_off(&self) -> bool {
        *self == Self::default()
    }
}

impl LootProfile {
    /// Read a profile.
    ///
    /// # Errors
    ///
    /// Not TOML, or a key this profile does not have.
    pub fn parse(text: &str) -> Result<Self, String> {
        crate::profile_file::read(text)
    }

    /// Write the profile as TOML.
    ///
    /// # Errors
    ///
    /// A value TOML cannot hold, which no field here produces.
    pub fn to_toml(&self) -> Result<String, String> {
        crate::profile_file::written(self)
    }

    /// The profile with every setting written out: skinning's even when it
    /// is off, and selling's as the selling round reads them, beside the
    /// `[town]` keys it carries and does not read. What the settings menu
    /// shows (`plan/50` §7 step 5); never saved.
    ///
    /// # Errors
    ///
    /// A value TOML cannot hold, which no field here produces.
    pub fn to_toml_whole(&self) -> Result<String, String> {
        let mut table = toml::Table::try_from(self).map_err(|e| e.to_string())?;
        let skin = toml::Value::try_from(&self.skin).map_err(|e| e.to_string())?;
        table.insert("skin".to_owned(), skin);
        let mut town = self.town.clone();
        town.extend(crate::town::Town::from_table(&self.town).to_table());
        table.insert("town".to_owned(), toml::Value::Table(town));
        toml::to_string(&table).map_err(|e| e.to_string())
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

/// The profile file's text with these creatures added to what it has learned
/// cannot be skinned, as eloot saves its profile on *You cannot skin*
/// (`eloot.lic:5846`). `Ok(None)` when every name is already there.
///
/// # Errors
///
/// The text is not a loot profile, or cannot be written back as one.
pub fn remember_unskinnable(text: &str, names: &[String]) -> Result<Option<String>, String> {
    // The comments at the file's head are kept, as `;hunt set` keeps them
    // (`settings::split`): written back from the profile alone, the first
    // *You cannot skin* took the importer's notes of what it dropped out of
    // the file (the review of 2026-09-29).
    let (head, _) = crate::settings::split(text)?;
    let mut profile = LootProfile::parse(text)?;
    let before = profile.skin.unskinnable.len();
    for name in names {
        if !profile.skin.unskinnable.contains(name) {
            profile.skin.unskinnable.push(name.clone());
        }
    }
    if profile.skin.unskinnable.len() == before {
        return Ok(None);
    }
    Ok(Some(format!("{head}{}", profile.to_toml()?)))
}

/// The character's loot profile: `<data>/hunt/loot/<instance>_<character>.toml`,
/// beside the hunt chain's files. `None` when the names cannot be a file
/// name.
#[must_use]
pub fn path(dir: &Path, instance: &str, character: &str) -> Option<PathBuf> {
    crate::profile_file::path(dir, "loot", instance, character)
}

/// The loot profile's own settings as the settings menu shows them
/// (`plan/50` §7 step 5), in the struct's order; skinning's are
/// [`SKIN_TABLE`], selling's `crate::town::settings::TABLE`.
pub const TABLE: &[crate::settings::Key] = {
    use crate::settings::{Key, KeyKind};
    &[
        Key {
            name: "take",
            label: "Take these kinds",
            help: "Object categories worth taking; a thing of no known category is taken too.",
            kind: KeyKind::Words,
        },
        Key {
            name: "leave",
            label: "Never take",
            help: "Names, or words in names, never taken.",
            kind: KeyKind::Words,
        },
        Key {
            name: "defensive",
            label: "Go defensive to loot",
            help: "Change to defensive stance before searching.",
            kind: KeyKind::Toggle,
        },
        Key {
            name: "disk",
            label: "Use the disk",
            help: "The disk holds what the bags cannot, and its boxes go to the pool.",
            kind: KeyKind::Toggle,
        },
        Key {
            name: "sigil_on_fail",
            label: "Sigil of Determination on a failed search",
            help: "Cast it when a corpse is not in any condition to be searched.",
            kind: KeyKind::Toggle,
        },
        Key {
            name: "phase_boxes",
            label: "Phase boxes",
            help: "Phase (704) a box before stowing it.",
            kind: KeyKind::Toggle,
        },
        Key {
            name: "overflow",
            label: "Overflow containers",
            help: "Tried in order when the stow bag and the default are full.",
            kind: KeyKind::Words,
        },
        Key {
            name: "crumbly",
            label: "Crumbles when stowed",
            help: "Names learned to crumble when stowed, left where they lie.",
            kind: KeyKind::Words,
        },
        Key {
            name: "unlootable",
            label: "Cannot be held",
            help: "Names the game refused to let this character hold, left.",
            kind: KeyKind::Words,
        },
        Key {
            name: "autoclose",
            label: "Containers that close themselves",
            help: "Opened before something is put in.",
            kind: KeyKind::Words,
        },
    ]
};

/// Skinning's settings as the settings menu shows them, under `[skin]`.
pub const SKIN_TABLE: &[crate::settings::Key] = {
    use crate::settings::{Key, KeyKind};
    &[
        Key {
            name: "skin.enable",
            label: "Skin",
            help: "Skin what is killed.",
            kind: KeyKind::Toggle,
        },
        Key {
            name: "skin.kneel",
            label: "Kneel to skin",
            help: "Kneel before skinning.",
            kind: KeyKind::Toggle,
        },
        Key {
            name: "skin.spell_604",
            label: "Bravery while skinning",
            help: "Keep Bravery (604) up while skinning.",
            kind: KeyKind::Toggle,
        },
        Key {
            name: "skin.resolve",
            label: "Sigil of Resolve",
            help: "Cast Sigil of Resolve before skinning.",
            kind: KeyKind::Toggle,
        },
        Key {
            name: "skin.bounty_only",
            label: "Only for a bounty",
            help: "Skin only the creature a skinning bounty names, and nothing without one.",
            kind: KeyKind::Toggle,
        },
        Key {
            name: "skin.weapon",
            label: "Skinning weapon",
            help: "The edged weapon, by a word of its name. Empty: whatever is in the right hand.",
            kind: KeyKind::Text,
        },
        Key {
            name: "skin.sheath",
            label: "Its sheath",
            help: "Where the edged weapon goes back. Empty: the default bag.",
            kind: KeyKind::Text,
        },
        Key {
            name: "skin.weapon_blunt",
            label: "Blunt skinning weapon",
            help: "For what is skinned blunt. Empty: those creatures are left.",
            kind: KeyKind::Text,
        },
        Key {
            name: "skin.sheath_blunt",
            label: "Its sheath",
            help: "Where the blunt weapon goes back.",
            kind: KeyKind::Text,
        },
        Key {
            name: "skin.exclude",
            label: "Never skin",
            help: "Names, or words in names, never skinned.",
            kind: KeyKind::Words,
        },
        Key {
            name: "skin.unskinnable",
            label: "Cannot be skinned",
            help: "Creatures the game said cannot be skinned, learned as the hunt goes.",
            kind: KeyKind::Words,
        },
    ]
};

#[cfg(test)]
mod tests {
    use super::{LootProfile, SKIN_TABLE, Skin, TABLE};
    use crate::settings::names;

    /// `TABLE` is every field but the `[skin]` and `[town]` tables, in
    /// order, and `SKIN_TABLE` every skinning field: a field added and not
    /// listed would never be shown in the menu.
    #[test]
    fn the_tables_are_every_setting() {
        let every = LootProfile {
            skin: Skin {
                enable: true,
                ..Skin::default()
            },
            ..LootProfile::default()
        };
        let table = toml::Table::try_from(&every).unwrap();
        let keys: Vec<&str> = table
            .keys()
            .map(String::as_str)
            .filter(|key| *key != "skin" && *key != "town")
            .collect();
        assert_eq!(keys, names(TABLE));
        let skin = toml::Table::try_from(Skin::default()).unwrap();
        let keys: Vec<String> = skin.keys().map(|key| format!("skin.{key}")).collect();
        assert_eq!(keys, names(SKIN_TABLE));
    }
}
