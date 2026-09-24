//! The typed shape of one critical-hit table entry.
//!
//! Split from `crit.rs` under Rule 4.1 (`plan/05:352-353`) -- move code down,
//! do not raise the cap. This file is the vocabulary; `crit/load.rs` is the
//! parser and `crit.rs` is the table and its lookup.
//!
//! Integer widths are **measured, not guessed** (`plan/05` §-2). Every range
//! quoted below came from a census over all 2,394 entries, reproducible with:
//!
//! ```text
//! $ ruby crates/cena-model/tools/extract_crit_tables.rb \
//!        reference/lich-5/lib/<game>/critranks /tmp/crit.tsv
//! 21 table files -> 2394 entries -> /tmp/crit.tsv
//! ```

use std::fmt;

/// The damage type of a critical hit: one Lich table file each.
///
/// 21 variants, which is every `*critical_table.rb` in
/// `reference/lich-5/lib/<game>/critranks/`. `Generic` is the odd one --
/// its table holds a single entry, the "is stunned" message that carries no
/// location or damage type of its own
/// (`reference/lich-5/lib/<game>/critranks/generic_critical_table.rb:7-11`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DamageType {
    /// `acid_critical_table.rb`.
    Acid,
    /// `cold_critical_table.rb`.
    Cold,
    /// `crush_critical_table.rb`.
    Crush,
    /// `disintegrate_critical_table.rb`.
    Disintegrate,
    /// `disruption_critical_table.rb`.
    Disruption,
    /// `fire_critical_table.rb`.
    Fire,
    /// `generic_critical_table.rb`. One entry, at `Unspecified` rank 0: the bare "is stunned" message.
    Generic,
    /// `grapple_critical_table.rb`.
    Grapple,
    /// `impact_critical_table.rb`.
    Impact,
    /// `lightning_critical_table.rb`.
    Lightning,
    /// `non_corporeal_critical_table.rb`. Spelled `non-corporeal` in Lich's `:type` field.
    NonCorporeal,
    /// `plasma_critical_table.rb`.
    Plasma,
    /// `puncture_critical_table.rb`.
    Puncture,
    /// `slash_critical_table.rb`.
    Slash,
    /// `steam_critical_table.rb`.
    Steam,
    /// `ucs_grapple_critical_table.rb`. Unarmed-combat grapple.
    UcsGrapple,
    /// `ucs_jab_critical_table.rb`. Unarmed-combat jab.
    UcsJab,
    /// `ucs_kick_critical_table.rb`. Unarmed-combat kick.
    UcsKick,
    /// `ucs_punch_critical_table.rb`. Unarmed-combat punch.
    UcsPunch,
    /// `unbalance_critical_table.rb`.
    Unbalance,
    /// `vacuum_critical_table.rb`.
    Vacuum,
}

impl DamageType {
    /// Every damage type, in the order the TSV is sorted.
    ///
    /// An associated `const`, not a `static`. That is deliberate: a `const` is
    /// inlined at each use site and has no address, so it cannot become a
    /// process global however it is written, and this port therefore needs no
    /// entry in `ALLOWED_STATICS` (plan/05 Rule 5.2, :400-408) -- the rule
    /// multi-session depends on stays untouched.
    pub const ALL: [Self; 21] = [
        Self::Acid,
        Self::Cold,
        Self::Crush,
        Self::Disintegrate,
        Self::Disruption,
        Self::Fire,
        Self::Generic,
        Self::Grapple,
        Self::Impact,
        Self::Lightning,
        Self::NonCorporeal,
        Self::Plasma,
        Self::Puncture,
        Self::Slash,
        Self::Steam,
        Self::UcsGrapple,
        Self::UcsJab,
        Self::UcsKick,
        Self::UcsPunch,
        Self::Unbalance,
        Self::Vacuum,
    ];

    /// The spelling used as a hash key in Lich and as column 1 of the TSV.
    ///
    /// Note this is the *key* spelling (`non_corporeal`), not the `:type`
    /// field's spelling (`"non-corporeal"`). The extractor asserts the two
    /// agree under Lich's own `clean_key`, so there is one spelling here.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Acid => "acid",
            Self::Cold => "cold",
            Self::Crush => "crush",
            Self::Disintegrate => "disintegrate",
            Self::Disruption => "disruption",
            Self::Fire => "fire",
            Self::Generic => "generic",
            Self::Grapple => "grapple",
            Self::Impact => "impact",
            Self::Lightning => "lightning",
            Self::NonCorporeal => "non_corporeal",
            Self::Plasma => "plasma",
            Self::Puncture => "puncture",
            Self::Slash => "slash",
            Self::Steam => "steam",
            Self::UcsGrapple => "ucs_grapple",
            Self::UcsJab => "ucs_jab",
            Self::UcsKick => "ucs_kick",
            Self::UcsPunch => "ucs_punch",
            Self::Unbalance => "unbalance",
            Self::Vacuum => "vacuum",
        }
    }

    /// Parse the TSV / Lich key spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.as_str() == text)
    }
}

impl fmt::Display for DamageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where a critical hit landed.
///
/// 15 variants, measured as the union of the level-2 hash keys of every table.
/// `Nerves` and `Unspecified` are not body parts in the anatomical sense but
/// are locations in the table's sense, which is the one that matters here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Location {
    /// Key `head`.
    Head,
    /// Key `neck`.
    Neck,
    /// Key `left_eye`.
    LeftEye,
    /// Key `right_eye`.
    RightEye,
    /// Key `chest`.
    Chest,
    /// Key `abdomen`.
    Abdomen,
    /// Key `back`.
    Back,
    /// Key `left_arm`.
    LeftArm,
    /// Key `right_arm`.
    RightArm,
    /// Key `left_hand`.
    LeftHand,
    /// Key `right_hand`.
    RightHand,
    /// Key `left_leg`.
    LeftLeg,
    /// Key `right_leg`.
    RightLeg,
    /// Key `nerves`. The nervous system rather than a limb; only the `lightning` table has it as a primary location.
    Nerves,
    /// Key `unspecified`. No location at all; used only by the single `Generic` entry.
    Unspecified,
}

impl Location {
    /// Every location. See `DamageType::ALL` for why this is a `const`.
    pub const ALL: [Self; 15] = [
        Self::Head,
        Self::Neck,
        Self::LeftEye,
        Self::RightEye,
        Self::Chest,
        Self::Abdomen,
        Self::Back,
        Self::LeftArm,
        Self::RightArm,
        Self::LeftHand,
        Self::RightHand,
        Self::LeftLeg,
        Self::RightLeg,
        Self::Nerves,
        Self::Unspecified,
    ];

    /// The spelling used as a Lich hash key and in the TSV; `parse` inverts it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Head => "head",
            Self::Neck => "neck",
            Self::LeftEye => "left_eye",
            Self::RightEye => "right_eye",
            Self::Chest => "chest",
            Self::Abdomen => "abdomen",
            Self::Back => "back",
            Self::LeftArm => "left_arm",
            Self::RightArm => "right_arm",
            Self::LeftHand => "left_hand",
            Self::RightHand => "right_hand",
            Self::LeftLeg => "left_leg",
            Self::RightLeg => "right_leg",
            Self::Nerves => "nerves",
            Self::Unspecified => "unspecified",
        }
    }

    /// Parse the spelling `as_str` produces; `None` for anything else.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.as_str() == text)
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where a *secondary* wound landed.
///
/// **This is not `Location`, and the difference is a measured data fact.**
/// `both eyes` appears as a secondary wound location -- once, in
/// `impact_critical_table.rb` -- and is not a level-2 key in any table.
/// Reusing `Location` would mean either inventing a `BothEyes` primary
/// location no table has, or dropping the entry.
///
/// The other seven spellings are a subset of `Location`'s, so `as_location`
/// converts where it can.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WoundLocation {
    /// Key `head`.
    Head,
    /// Key `neck`.
    Neck,
    /// Key `chest`.
    Chest,
    /// Key `abdomen`.
    Abdomen,
    /// Key `back`.
    Back,
    /// Key `nerves`.
    Nerves,
    /// Key `right_leg`.
    RightLeg,
    /// Key `both_eyes`. Once, in `impact_critical_table.rb`; no primary `Location` matches it.
    BothEyes,
}

impl WoundLocation {
    /// Every secondary-wound location. See `DamageType::ALL` for the `const`.
    pub const ALL: [Self; 8] = [
        Self::Head,
        Self::Neck,
        Self::Chest,
        Self::Abdomen,
        Self::Back,
        Self::Nerves,
        Self::RightLeg,
        Self::BothEyes,
    ];

    /// The spelling in the TSV's `secondary_location` column; `parse` inverts it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Head => "head",
            Self::Neck => "neck",
            Self::Chest => "chest",
            Self::Abdomen => "abdomen",
            Self::Back => "back",
            Self::Nerves => "nerves",
            Self::RightLeg => "right_leg",
            Self::BothEyes => "both_eyes",
        }
    }

    /// Parse the spelling `as_str` produces; `None` for anything else.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.as_str() == text)
    }

    /// The primary `Location` this names, where one exists.
    ///
    /// `BothEyes` has none, which is the whole reason this enum is separate.
    #[must_use]
    pub const fn as_location(self) -> Option<Location> {
        match self {
            Self::Head => Some(Location::Head),
            Self::Neck => Some(Location::Neck),
            Self::Chest => Some(Location::Chest),
            Self::Abdomen => Some(Location::Abdomen),
            Self::Back => Some(Location::Back),
            Self::Nerves => Some(Location::Nerves),
            Self::RightLeg => Some(Location::RightLeg),
            Self::BothEyes => None,
        }
    }
}

impl fmt::Display for WoundLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The posture a critical hit forces on its target.
///
/// Measured domain over 2,394 entries: absent 2,051, `PRONE` 319,
/// `KNEELING` 13, `SITTING` 11. Absent is the common case, which is why the
/// field is an `Option` rather than carrying a `Standing` variant the data
/// never states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Position {
    /// Lich's `KNEELING`; 13 entries.
    Kneeling,
    /// Lich's `PRONE`; 319 entries, the common knockdown.
    Prone,
    /// Lich's `SITTING`; 11 entries.
    Sitting,
}

impl Position {
    /// Every position. See `DamageType::ALL` for the `const`.
    pub const ALL: [Self; 3] = [Self::Kneeling, Self::Prone, Self::Sitting];

    /// The spelling in the TSV's `position` column; `parse` inverts it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Kneeling => "kneeling",
            Self::Prone => "prone",
            Self::Sitting => "sitting",
        }
    }

    /// Parse the spelling `as_str` produces; `None` for anything else.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.as_str() == text)
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A second wound, at a different location from the hit itself.
///
/// 156 of 2,394 entries carry one, in 16 distinct shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SecondaryWound {
    /// Where the second wound lands; TSV column `secondary_location`.
    pub location: WoundLocation,
    /// Measured 1..=3 among the entries that have one.
    pub wound_rank: u8,
}
