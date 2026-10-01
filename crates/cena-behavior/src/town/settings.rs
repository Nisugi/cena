//! The selling settings, typed out of the loot profile's `[town]` table.
//!
//! The importer carries eloot's town keys verbatim (`loot/import.rs`,
//! `TOWN_PREFIXES`), so the table holds `sell_loot_types`, `sell_container`
//! and the rest under eloot's own names. This reads the ones Stage 4 acts on
//! into fields named for what they do; a key it does not know stays in the
//! table for the stage that will.

use toml::Table;

/// How the character sells (`eloot.lic:2027-2072`, the `sell_*` keys).
#[derive(Clone, Debug, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "eloot's switches, carried as they are; a profile is a table of switches"
)]
pub struct Town {
    /// Object categories to sell (`sell_loot_types`).
    pub sell_types: Vec<String>,
    /// Stow slots whose bags are sold from (`sell_container`): `default`,
    /// `overflow`, and stow slot names.
    pub containers: Vec<String>,
    /// Names, or words in names, never sold (`sell_exclude`).
    pub exclude: Vec<String>,
    /// Categories appraised before selling (`sell_appraise_types`).
    pub appraise_types: Vec<String>,
    /// Sell at the gem shop only up to this appraisal (`sell_appraise_gemshop`).
    pub appraise_gemshop: u64,
    /// Sell at the pawnshop only up to this appraisal (`sell_appraise_pawnshop`).
    pub appraise_pawnshop: u64,
    /// Hand collectibles in (`sell_collectibles`).
    pub collectibles: bool,
    /// Give gold rings to the Chronomage (`sell_gold_rings`).
    pub gold_rings: bool,
    /// Analyze before selling what could be a transmog, and keep it (`keep_transmogs`).
    pub keep_transmogs: bool,
    /// Silver kept in hand when depositing (`sell_keep_silver`).
    pub keep_silver: u64,
    /// Where an item over the limit goes, by a word of the bag's name
    /// (`appraisal_container`); empty means back where it came from.
    pub appraisal_container: String,
    /// Drop boxes in the locksmith pool (`sell_locksmith_pool`).
    pub pool: bool,
    /// The standard tip per box (`sell_locksmith_pool_tip`).
    pub pool_tip: u64,
    /// The tip is a percent of the box's value (`sell_locksmith_pool_tip_percent`).
    pub pool_tip_percent: bool,
    /// A charm that gathers a box's coins, by its name (`charm_name`).
    pub charm: String,
    /// Appraise, never sell, at the pawnshop what the gem shop found too
    /// valuable (`sell_pawn_recheck`).
    pub pawn_recheck: bool,
    /// Scrolls kept for these spells (`sell_keep_scrolls`): `215` keeps a
    /// scroll holding 215 that is not vibrant, `215v` one that is.
    pub keep_scrolls: Vec<String>,
    /// Sell on the Isle of Four Winds, wherever the round begins
    /// (`sell_fwi`, `town/route.rs`).
    pub fwi: bool,
    /// Boxes on the character's disk go to the pool too: the loot
    /// profile's `use_disk`, set by the driver.
    pub disk: bool,
    /// The loot profile's overflow containers, by name: sold from when
    /// `containers` names `overflow`.
    pub overflow: Vec<String>,
    /// The loot profile's `keep_closed`: the bags opened to sell from are
    /// closed again after the round.
    pub keep_closed: bool,
}

impl Default for Town {
    fn default() -> Self {
        Self {
            sell_types: Vec::new(),
            containers: vec!["default".to_owned()],
            exclude: Vec::new(),
            appraise_types: Vec::new(),
            appraise_gemshop: u64::MAX,
            appraise_pawnshop: u64::MAX,
            collectibles: false,
            gold_rings: false,
            keep_transmogs: false,
            keep_silver: 0,
            appraisal_container: String::new(),
            pool: false,
            pool_tip: 0,
            pool_tip_percent: false,
            charm: String::new(),
            pawn_recheck: false,
            keep_scrolls: Vec::new(),
            fwi: false,
            disk: false,
            overflow: Vec::new(),
            keep_closed: false,
        }
    }
}

fn list(table: &Table, key: &str) -> Vec<String> {
    match table.get(key) {
        Some(toml::Value::Array(items)) => items
            .iter()
            .filter_map(|v| match v {
                toml::Value::String(s) => Some(s.clone()),
                toml::Value::Integer(n) => Some(n.to_string()),
                _ => None,
            })
            .collect(),
        Some(toml::Value::String(one)) if !one.is_empty() => vec![one.clone()],
        _ => Vec::new(),
    }
}

fn flag(table: &Table, key: &str) -> bool {
    table.get(key).and_then(toml::Value::as_bool) == Some(true)
}

fn number(table: &Table, key: &str, default: u64) -> u64 {
    match table.get(key) {
        Some(toml::Value::Integer(n)) => u64::try_from(*n).unwrap_or(default),
        Some(toml::Value::String(s)) => s.parse().unwrap_or(default),
        _ => default,
    }
}

fn text(table: &Table, key: &str) -> String {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned()
}

impl Town {
    /// The settings in a profile's `[town]` table; a key absent takes
    /// eloot's default.
    #[must_use]
    pub fn from_table(table: &Table) -> Self {
        let defaults = Self::default();
        let containers = list(table, "sell_container");
        Self {
            sell_types: list(table, "sell_loot_types"),
            containers: if containers.is_empty() {
                defaults.containers
            } else {
                containers
            },
            exclude: list(table, "sell_exclude"),
            appraise_types: list(table, "sell_appraise_types"),
            appraise_gemshop: number(table, "sell_appraise_gemshop", u64::MAX),
            appraise_pawnshop: number(table, "sell_appraise_pawnshop", u64::MAX),
            collectibles: flag(table, "sell_collectibles"),
            gold_rings: flag(table, "sell_gold_rings"),
            keep_transmogs: flag(table, "keep_transmogs"),
            keep_silver: number(table, "sell_keep_silver", 0),
            appraisal_container: text(table, "appraisal_container"),
            pool: flag(table, "sell_locksmith_pool"),
            pool_tip: number(table, "sell_locksmith_pool_tip", 0),
            pool_tip_percent: flag(table, "sell_locksmith_pool_tip_percent"),
            charm: text(table, "charm_name"),
            pawn_recheck: flag(table, "sell_pawn_recheck"),
            keep_scrolls: list(table, "sell_keep_scrolls"),
            fwi: flag(table, "sell_fwi"),
            disk: flag(table, "use_disk"),
            overflow: Vec::new(),
            keep_closed: false,
        }
    }

    /// These settings as the `[town]` table writes them, under eloot's
    /// names: [`Self::from_table`]'s inverse, what the settings menu shows
    /// (`plan/50` §7 step 5). A limit that is none (`u64::MAX`) is left out,
    /// as is the disk, which is the loot profile's own `disk`.
    #[must_use]
    pub fn to_table(&self) -> Table {
        use toml::Value;
        let list =
            |items: &[String]| Value::Array(items.iter().cloned().map(Value::String).collect());
        let number = |n: u64| i64::try_from(n).ok().map(Value::Integer);
        let entries = [
            ("sell_loot_types", Some(list(&self.sell_types))),
            ("sell_container", Some(list(&self.containers))),
            ("sell_exclude", Some(list(&self.exclude))),
            ("sell_appraise_types", Some(list(&self.appraise_types))),
            ("sell_appraise_gemshop", number(self.appraise_gemshop)),
            ("sell_appraise_pawnshop", number(self.appraise_pawnshop)),
            ("sell_collectibles", Some(Value::Boolean(self.collectibles))),
            ("sell_gold_rings", Some(Value::Boolean(self.gold_rings))),
            ("keep_transmogs", Some(Value::Boolean(self.keep_transmogs))),
            ("sell_keep_silver", number(self.keep_silver)),
            (
                "appraisal_container",
                Some(Value::String(self.appraisal_container.clone())),
            ),
            ("sell_locksmith_pool", Some(Value::Boolean(self.pool))),
            ("sell_locksmith_pool_tip", number(self.pool_tip)),
            (
                "sell_locksmith_pool_tip_percent",
                Some(Value::Boolean(self.pool_tip_percent)),
            ),
            ("charm_name", Some(Value::String(self.charm.clone()))),
            ("sell_pawn_recheck", Some(Value::Boolean(self.pawn_recheck))),
            ("sell_keep_scrolls", Some(list(&self.keep_scrolls))),
            ("sell_fwi", Some(Value::Boolean(self.fwi))),
        ];
        entries
            .into_iter()
            .filter_map(|(key, value)| Some((key.to_owned(), value?)))
            .collect()
    }

    /// The settings for a loot profile: its `[town]` table, and the disk
    /// the loot side uses.
    #[must_use]
    pub fn for_profile(profile: &crate::loot::LootProfile) -> Self {
        let mut town = Self::from_table(&profile.town);
        town.disk |= profile.disk;
        town.overflow.clone_from(&profile.overflow);
        town.keep_closed = profile.keep_closed;
        town
    }

    /// Is a scroll that holds `spell` kept? `vibrant` says whether the line
    /// naming it said so (`no_vib_regex`, `vib_scrolls_regex`,
    /// `eloot.lic:511-515`).
    #[must_use]
    pub fn keeps_scroll(&self, spell: u16, vibrant: bool) -> bool {
        self.keep_scrolls.iter().any(|entry| {
            let wants_vibrant = entry.to_ascii_lowercase().contains('v');
            let number: String = entry.chars().filter(char::is_ascii_digit).collect();
            number.parse::<u16>() == Ok(spell) && wants_vibrant == vibrant
        })
    }

    /// Is this category sold at all?
    #[must_use]
    pub fn sells(&self, category: &str) -> bool {
        self.sell_types.iter().any(|t| t == category)
    }

    /// Is this category appraised before it is sold?
    #[must_use]
    pub fn appraises(&self, category: &str) -> bool {
        self.appraise_types.iter().any(|t| t == category)
    }

    /// Is this name, or a word in it, excluded from selling?
    #[must_use]
    pub fn excludes(&self, name: &str) -> bool {
        self.exclude
            .iter()
            .any(|word| !word.is_empty() && name.contains(word.as_str()))
    }
}

/// Every selling setting as the settings menu shows it (`plan/50` §7 step
/// 5), under the `[town]` table's own names, which are eloot's. The disk is
/// not here: it is the loot profile's `disk`.
pub const TABLE: &[crate::settings::Key] = {
    use crate::settings::{Key, KeyKind};
    const SILVER: KeyKind = KeyKind::Whole {
        min: 0,
        max: 1_000_000_000,
    };
    &[
        Key {
            name: "town.sell_loot_types",
            label: "Sell these kinds",
            help: "Object categories sold at the rest: gem, skin, box, ...",
            kind: KeyKind::Words,
        },
        Key {
            name: "town.sell_container",
            label: "Sell from",
            help: "Stow slots whose bags are sold from: default, overflow, or a slot's name.",
            kind: KeyKind::Words,
        },
        Key {
            name: "town.sell_exclude",
            label: "Never sell",
            help: "Names, or words in names, never sold.",
            kind: KeyKind::Words,
        },
        Key {
            name: "town.sell_appraise_types",
            label: "Appraise these kinds first",
            help: "Categories appraised before they are sold.",
            kind: KeyKind::Words,
        },
        Key {
            name: "town.sell_appraise_gemshop",
            label: "Gem shop limit",
            help: "Sell at the gem shop only what appraises at or under this. Empty: no limit.",
            kind: SILVER,
        },
        Key {
            name: "town.sell_appraise_pawnshop",
            label: "Pawnshop limit",
            help: "Sell at the pawnshop only what appraises at or under this. Empty: no limit.",
            kind: SILVER,
        },
        Key {
            name: "town.sell_collectibles",
            label: "Hand in collectibles",
            help: "Hand collectibles in at the collectibles shop.",
            kind: KeyKind::Toggle,
        },
        Key {
            name: "town.sell_gold_rings",
            label: "Gold rings to the Chronomage",
            help: "Give gold rings to the Chronomage.",
            kind: KeyKind::Toggle,
        },
        Key {
            name: "town.keep_transmogs",
            label: "Keep transmogs",
            help: "Analyze what could be a transmog before selling it, and keep it.",
            kind: KeyKind::Toggle,
        },
        Key {
            name: "town.sell_keep_silver",
            label: "Silver kept in hand",
            help: "Silver kept on the character when depositing.",
            kind: SILVER,
        },
        Key {
            name: "town.appraisal_container",
            label: "Bag for what is over the limit",
            help: "A word of the bag's name. Empty: it goes back where it came from.",
            kind: KeyKind::Text,
        },
        Key {
            name: "town.sell_locksmith_pool",
            label: "Use the locksmith pool",
            help: "Drop boxes in the locksmith pool.",
            kind: KeyKind::Toggle,
        },
        Key {
            name: "town.sell_locksmith_pool_tip",
            label: "Pool tip",
            help: "The tip per box.",
            kind: SILVER,
        },
        Key {
            name: "town.sell_locksmith_pool_tip_percent",
            label: "Tip as a percent",
            help: "The tip is a percent of the box's value, not silver.",
            kind: KeyKind::Toggle,
        },
        Key {
            name: "town.charm_name",
            label: "Coin charm",
            help: "A charm that gathers a box's coins, by its name.",
            kind: KeyKind::Text,
        },
        Key {
            name: "town.sell_pawn_recheck",
            label: "Pawnshop recheck",
            help: "Appraise, never sell, at the pawnshop what the gem shop found too valuable.",
            kind: KeyKind::Toggle,
        },
        Key {
            name: "town.sell_keep_scrolls",
            label: "Scrolls kept",
            help: "Scrolls kept for these spells: 215 keeps one that is not vibrant, 215v one that is.",
            kind: KeyKind::Words,
        },
        Key {
            name: "town.sell_fwi",
            label: "Sell in Mist Harbor",
            help: "Sell on the Isle of Four Winds wherever the round begins, there and back by the trinket Travel's settings name.",
            kind: KeyKind::Toggle,
        },
    ]
};

#[cfg(test)]
mod tests {
    use super::{TABLE, Town};

    /// A town of every setting, none at its default but the disk.
    fn every() -> Town {
        Town {
            sell_types: vec!["gem".to_owned()],
            containers: vec!["overflow".to_owned()],
            exclude: vec!["heirloom".to_owned()],
            appraise_types: vec!["jewelry".to_owned()],
            appraise_gemshop: 50_000,
            appraise_pawnshop: 20_000,
            collectibles: true,
            gold_rings: true,
            keep_transmogs: true,
            keep_silver: 5000,
            appraisal_container: "cloak".to_owned(),
            pool: true,
            pool_tip: 25,
            pool_tip_percent: true,
            charm: "silver charm".to_owned(),
            pawn_recheck: true,
            keep_scrolls: vec!["215v".to_owned()],
            fwi: true,
            disk: false,
            overflow: Vec::new(),
            keep_closed: false,
        }
    }

    /// `TABLE` is every setting `to_table` writes, in order, and what it
    /// writes reads back the same: the menu shows what the round acts on.
    #[test]
    fn the_table_is_every_setting_and_reads_back() {
        let written = every().to_table();
        let keys: Vec<String> = written.keys().map(|key| format!("town.{key}")).collect();
        let names: Vec<String> = crate::settings::names(TABLE)
            .into_iter()
            .map(str::to_owned)
            .collect();
        assert_eq!(keys, names);
        assert_eq!(Town::from_table(&written), every());
        // Switches that differ, so no two can be written under each other's
        // names and still read back.
        let alternate = Town {
            gold_rings: false,
            pool: false,
            pawn_recheck: false,
            ..every()
        };
        assert_eq!(Town::from_table(&alternate.to_table()), alternate);
        let other = Town {
            collectibles: false,
            keep_transmogs: false,
            pool_tip_percent: false,
            ..every()
        };
        assert_eq!(Town::from_table(&other.to_table()), other);
        let none = Town::default().to_table();
        assert!(
            !none.contains_key("sell_appraise_gemshop"),
            "no limit is left out"
        );
        assert_eq!(Town::from_table(&none), Town::default());
    }
}
