//! Weapons, armor and shields: the stat tables, and the aliases that name them.
//!
//! Ports `lib/gemstone/armaments/` (2,942 lines across 14 files) as three
//! TSVs plus a lookup, extracted by `tools/extract_armaments.rb` -- the same
//! route `crit_tables.tsv` and `gameobj-data.tsv` take, and for the reason
//! `plan/13` §4a gives: static data ships as data files.
//!
//! # The aliases are the point
//!
//! MEASURED: **706 alias rows over 96 weapons, 20 armor sub-groups and 4
//! shields.** `broadsword` alone carries fifteen -- `flyssa`, `katzbalger`,
//! `spatha`, `xiphos` and eleven more -- and they are how a client turns
//! `a flyssa` on the wire into "this is a broadsword, DF 0.4 against cloth".
//!
//! That knowledge lives only in this table. It is the same argument
//! `SKILL_NAME_MAP` makes for the 46 skill names, and the reason this is a
//! port rather than a rewrite.
//!
//! # Two values are DERIVED, and live here rather than in the data
//!
//! `find_crit_divisor` and `find_coverage` (`armor_stats.rb:352-390`) are pure
//! functions of the armor group, not stored fields. An earlier version of the
//! extractor wrote empty columns for them, having guessed at field names the
//! data does not have -- see [`Armor::crit_divisor`] and [`Armor::coverage`].
//!
//! # Positional arrays
//!
//! Three columns hold `|`-joined arrays whose **position carries meaning**:
//!
//! | Column | Indexed by |
//! |---|---|
//! | `damage_factor_by_ag_0_to_5` | armor group, index 0 unused |
//! | `avd_by_asg_1_to_20` | armor sub-group |
//! | `hindrances_0_to_19`, `training_reqs_0_to_19` | spell circle |
//!
//! An empty element is **not zero**. A spell circle that does not exist
//! (index 8, 15, 18) is `None`, because a circle with no hindrance and a
//! circle that is not a circle are different facts -- the same rule
//! `plan/12` §5.2 applies to game state.

use std::collections::BTreeMap;
use std::sync::OnceLock;

const WEAPONS_TSV: &str = include_str!("../../data/weapons.tsv");
const ARMOR_TSV: &str = include_str!("../../data/armor.tsv");
const SHIELDS_TSV: &str = include_str!("../../data/shields.tsv");
const ALIASES_TSV: &str = include_str!("../../data/armament_aliases.tsv");

/// Which table an alias resolves into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArmamentKind {
    /// `weapon` in the alias table's `kind` column; resolves into `weapons.tsv`.
    Weapon,
    /// `armor` in the alias table's `kind` column; resolves into `armor.tsv`.
    Armor,
    /// `shield` in the alias table's `kind` column; resolves into `shields.tsv`.
    Shield,
}

/// How much of the body a suit of armor covers (`armor_stats.rb:370-389`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Coverage {
    /// Armor sub-groups 1-4.
    Torso,
    /// Armor sub-groups 5-8.
    TorsoAndArms,
    /// Armor sub-groups 9-12.
    TorsoArmsAndLegs,
    /// Armor sub-groups 13-20.
    TorsoArmsLegsAndHead,
}

/// One weapon's stats.
#[derive(Debug, Clone, PartialEq)]
pub struct Weapon {
    /// The skill category: `edged`, `blunt`, `polearm`, ...
    pub category: String,
    /// The canonical id, e.g. `broadsword`.
    pub id: String,
    /// The display name.
    pub base_name: String,
    /// Damage factor **by armor group**, index 0 unused.
    ///
    /// `[None, 0.4, 0.3, 0.23, 0.26, 0.18]` reads: 0.4 against cloth, 0.3
    /// against leather, and so on. `None` where the table has no value.
    pub damage_factor: Vec<Option<f32>>,
    /// Base roundtime in seconds.
    pub base_rt: Option<u8>,
    /// The floor roundtime, whatever the character's speed.
    pub min_rt: Option<u8>,
    /// Percentage of each damage type, where the table states it.
    pub slash: Option<f32>,
    /// See [`Self::slash`].
    pub crush: Option<f32>,
    /// See [`Self::slash`].
    pub puncture: Option<f32>,
    /// Attack-versus-defense **by armor sub-group 1..=20**.
    pub avd: Vec<Option<i16>>,
}

impl Weapon {
    /// Damage factor against one armor group (1..=5).
    ///
    /// `None` for a group the table does not state, which is different from
    /// a damage factor of zero.
    #[must_use]
    pub fn damage_factor_vs(&self, armor_group: u8) -> Option<f32> {
        self.damage_factor
            .get(usize::from(armor_group))
            .copied()
            .flatten()
    }

    /// `AvD` against one armor sub-group (1..=20).
    #[must_use]
    pub fn avd_vs(&self, asg: u8) -> Option<i16> {
        let index = usize::from(asg).checked_sub(1)?;
        self.avd.get(index).copied().flatten()
    }
}

/// One armor sub-group's stats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Armor {
    /// `cloth`, `leather`, `scale`, `chain`, `plate`.
    pub kind: String,
    /// The canonical name, e.g. `full_plate`.
    pub base_name: String,
    /// Armor group, 1..=5.
    pub armor_group: u8,
    /// Armor sub-group, 1..=20.
    pub armor_sub_group: u8,
    /// Weight in pounds.
    pub base_weight: Option<u16>,
    /// Minimum roundtime this armor imposes.
    pub min_rt: Option<u8>,
    /// Penalty to combat maneuvers.
    pub action_penalty: Option<i16>,
    /// Defence versus non-magical attacks.
    pub normal_cva: Option<i16>,
    /// Defence versus magical attacks.
    pub magical_cva: Option<i16>,
    /// Spell hindrance **by circle**, 0..=19. See the module docs.
    pub hindrances: Vec<Option<i16>>,
    /// The worst hindrance across all circles.
    pub hindrance_max: Option<i16>,
    /// Ranks of Armor Use that remove the hindrance, by circle.
    pub training_reqs: Vec<Option<i16>>,
}

impl Armor {
    /// The crit divisor for this armor (`armor_stats.rb:352-357`).
    ///
    /// **Derived from the armor group, not stored.** A higher divisor means
    /// criticals are reduced further, which is why plate (11) protects better
    /// than cloth (5).
    #[must_use]
    pub const fn crit_divisor(&self) -> Option<u8> {
        match self.armor_group {
            1 => Some(5),
            2 => Some(6),
            3 => Some(7),
            4 => Some(9),
            5 => Some(11),
            _ => None,
        }
    }

    /// What this armor covers (`armor_stats.rb:370-389`).
    ///
    /// Also derived: sub-groups bucket into four ranges of four.
    #[must_use]
    pub const fn coverage(&self) -> Option<Coverage> {
        match self.armor_sub_group {
            1..=4 => Some(Coverage::Torso),
            5..=8 => Some(Coverage::TorsoAndArms),
            9..=12 => Some(Coverage::TorsoArmsAndLegs),
            13..=20 => Some(Coverage::TorsoArmsLegsAndHead),
            _ => None,
        }
    }

    /// Hindrance to one spell circle, by its index in the positional array.
    ///
    /// `None` for a circle the table leaves empty -- which includes indices
    /// 8, 15 and 18, positions that correspond to no live circle.
    #[must_use]
    pub fn hindrance_for_circle(&self, index: usize) -> Option<i16> {
        self.hindrances.get(index).copied().flatten()
    }
}

/// One shield's stats.
#[derive(Debug, Clone, PartialEq)]
pub struct Shield {
    /// The canonical id, e.g. `tower_shield`.
    pub id: String,
    /// The size category.
    pub category: String,
    /// The display name.
    pub base_name: String,
    /// Multiplier applied for the shield's size.
    pub size_modifier: Option<f32>,
    /// Multiplier applied to evasion.
    pub evade_modifier: Option<f32>,
    /// Weight in pounds.
    pub base_weight: Option<u16>,
}

/// Everything the tables hold.
struct Tables {
    /// Keyed by `(category, id)`, **not by id alone**.
    ///
    /// MEASURED: eleven weapons appear in two categories -- a bastard sword is
    /// `edged` at DF 0.45 and `two_handed` at 0.55, a handaxe is both `edged`
    /// and `thrown`, a cestus both `brawling` and `unarmed`. Same weapon, two
    /// skills, two stat lines. Keying by id alone silently kept 85 of 96.
    weapons: BTreeMap<(String, String), Weapon>,
    armor: BTreeMap<String, Armor>,
    shields: BTreeMap<String, Shield>,
    /// `(kind, alias) -> id`.
    ///
    /// **Keyed by kind as well as name**, because an alias can resolve
    /// differently per table -- and because a caller asking "what weapon is
    /// this" should not get an armor answer.
    aliases: BTreeMap<(ArmamentKind, String), Vec<String>>,
}

fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(build)
}

/// Split a `|`-joined positional column, preserving empties as `None`.
fn positional<T: std::str::FromStr>(field: &str) -> Vec<Option<T>> {
    field
        .split('|')
        .map(|v| {
            let v = v.trim();
            if v.is_empty() { None } else { v.parse().ok() }
        })
        .collect()
}

/// Parse a scalar column, empty meaning absent.
fn scalar<T: std::str::FromStr>(field: &str) -> Option<T> {
    let field = field.trim();
    if field.is_empty() {
        return None;
    }
    field.parse().ok()
}

/// Rows of a TSV, header skipped, split on tabs.
fn rows(tsv: &'static str) -> impl Iterator<Item = Vec<&'static str>> {
    tsv.lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split('\t').collect())
}

fn build() -> Tables {
    let mut weapons = BTreeMap::new();
    for r in rows(WEAPONS_TSV) {
        let (Some(category), Some(id), Some(base_name)) = (r.first(), r.get(1), r.get(2)) else {
            continue;
        };
        weapons.insert(
            ((*category).to_owned(), (*id).to_owned()),
            Weapon {
                category: (*category).to_owned(),
                id: (*id).to_owned(),
                base_name: (*base_name).to_owned(),
                damage_factor: positional(r.get(3).copied().unwrap_or_default()),
                base_rt: scalar(r.get(4).copied().unwrap_or_default()),
                min_rt: scalar(r.get(5).copied().unwrap_or_default()),
                slash: scalar(r.get(6).copied().unwrap_or_default()),
                crush: scalar(r.get(7).copied().unwrap_or_default()),
                puncture: scalar(r.get(8).copied().unwrap_or_default()),
                avd: positional(r.get(10).copied().unwrap_or_default()),
            },
        );
    }

    let mut armor = BTreeMap::new();
    for r in rows(ARMOR_TSV) {
        let (Some(kind), Some(base_name)) = (r.first(), r.get(1)) else {
            continue;
        };
        // The two declared-and-empty sub-groups carry `-` as a base name; they
        // are real gaps in the ASG numbering and are not armor.
        if *base_name == "-" {
            continue;
        }
        let (Some(ag), Some(asg)) = (
            scalar::<u8>(r.get(2).copied().unwrap_or_default()),
            scalar::<u8>(r.get(3).copied().unwrap_or_default()),
        ) else {
            continue;
        };
        armor.insert(
            (*base_name).to_owned(),
            Armor {
                kind: (*kind).to_owned(),
                base_name: (*base_name).to_owned(),
                armor_group: ag,
                armor_sub_group: asg,
                base_weight: scalar(r.get(4).copied().unwrap_or_default()),
                min_rt: scalar(r.get(5).copied().unwrap_or_default()),
                action_penalty: scalar(r.get(6).copied().unwrap_or_default()),
                normal_cva: scalar(r.get(7).copied().unwrap_or_default()),
                magical_cva: scalar(r.get(8).copied().unwrap_or_default()),
                hindrances: positional(r.get(9).copied().unwrap_or_default()),
                hindrance_max: scalar(r.get(10).copied().unwrap_or_default()),
                training_reqs: positional(r.get(11).copied().unwrap_or_default()),
            },
        );
    }

    let mut shields = BTreeMap::new();
    for r in rows(SHIELDS_TSV) {
        let (Some(id), Some(category), Some(base_name)) = (r.first(), r.get(1), r.get(2)) else {
            continue;
        };
        shields.insert(
            (*id).to_owned(),
            Shield {
                id: (*id).to_owned(),
                category: (*category).to_owned(),
                base_name: (*base_name).to_owned(),
                size_modifier: scalar(r.get(3).copied().unwrap_or_default()),
                evade_modifier: scalar(r.get(4).copied().unwrap_or_default()),
                base_weight: scalar(r.get(5).copied().unwrap_or_default()),
            },
        );
    }

    let mut aliases: BTreeMap<(ArmamentKind, String), Vec<String>> = BTreeMap::new();
    for r in rows(ALIASES_TSV) {
        let (Some(kind), Some(name), Some(id)) = (r.first(), r.get(1), r.get(2)) else {
            continue;
        };
        let kind = match *kind {
            "weapon" => ArmamentKind::Weapon,
            "armor" => ArmamentKind::Armor,
            "shield" => ArmamentKind::Shield,
            _ => continue,
        };
        aliases
            .entry((kind, name.to_ascii_lowercase()))
            .or_default()
            .push((*id).to_owned());
    }

    Tables {
        weapons,
        armor,
        shields,
        aliases,
    }
}

/// Resolve a name -- canonical or alias -- to the ids it could mean.
///
/// **Returns every match, not the best one.** MEASURED: `aketon` names both
/// `double_leather` and `reinforced_leather`, so an alias is not always
/// unique. Guessing would attribute the wrong stats to a real item, which is
/// the same reasoning `resolve_noun` records for room objects.
#[must_use]
pub fn resolve(kind: ArmamentKind, name: &str) -> &'static [String] {
    tables()
        .aliases
        .get(&(kind, name.trim().to_ascii_lowercase()))
        .map_or(&[], Vec::as_slice)
}

/// A weapon by its category and canonical id.
///
/// **Both are needed.** Eleven weapons appear in two categories with
/// different stats -- see [`weapons_named`] for the lookup that does not know
/// the category.
#[must_use]
pub fn weapon(category: &str, id: &str) -> Option<&'static Weapon> {
    tables().weapons.get(&(category.to_owned(), id.to_owned()))
}

/// Every stat line for one weapon id, across categories.
///
/// A bastard sword returns two: `edged` and `two_handed`. A caller that knows
/// which skill is being used picks; one that does not has to decide, and
/// returning both is what lets it.
pub fn weapons_named(id: &str) -> impl Iterator<Item = &'static Weapon> {
    tables()
        .weapons
        .iter()
        .filter(move |((_, weapon_id), _)| weapon_id == id)
        .map(|(_, w)| w)
}

/// An armor sub-group by its canonical name.
#[must_use]
pub fn armor(base_name: &str) -> Option<&'static Armor> {
    tables().armor.get(base_name)
}

/// A shield by its canonical id.
#[must_use]
pub fn shield(id: &str) -> Option<&'static Shield> {
    tables().shields.get(id)
}

/// Every weapon, in id order.
pub fn weapons() -> impl Iterator<Item = &'static Weapon> {
    tables().weapons.values()
}

/// Every armor sub-group, in name order.
pub fn armors() -> impl Iterator<Item = &'static Armor> {
    tables().armor.values()
}

/// Every shield.
pub fn shields() -> impl Iterator<Item = &'static Shield> {
    tables().shields.values()
}

/// How many aliases the tables hold, across all three kinds.
#[must_use]
pub fn alias_count() -> usize {
    tables().aliases.values().map(Vec::len).sum()
}
