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
    /// A charm that gathers a box's coins, by a word of its name (`charm_name`).
    pub charm: String,
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
        }
    }
}

fn list(table: &Table, key: &str) -> Vec<String> {
    match table.get(key) {
        Some(toml::Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str())
            .map(str::to_owned)
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
        }
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
