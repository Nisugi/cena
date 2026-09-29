//! The heal profile: the herb container and eherbs' switches, one file per
//! character beside the loot profile (`plan/36` Stage 2).
//!
//! eherbs keeps these in Lich's per-character settings (`load_eherbs_settings`,
//! `eherbs.lic:350-500`), which no file carries, so there is nothing to
//! import: the file is written by hand or by `;heal set`.
//!
//! ```toml
//! container = "herb pouch"
//! skip_scars = false
//! potions = false
//! yabathilium = true
//! stock = 50
//! ```

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::settings::{Key, KeyKind};

/// How a character heals with herbs.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "eherbs' switches, carried as they are; a profile is a table of switches"
)]
pub struct HealProfile {
    /// The herb container, by a word or words of its name (`herbsack`).
    pub container: String,
    /// Leave minor scars (`skip_scars`).
    pub skip_scars: bool,
    /// Prefer what is drunk to what is eaten (`use_potions`).
    pub potions: bool,
    /// Yabathilium first for blood (`use_yaba`).
    pub yabathilium: bool,
    /// Only ever treat blood (`blood_toggle`).
    pub blood_only: bool,
    /// Buy a herb the container lacks (`buy_missing`; Stage 4).
    pub buy_missing: bool,
    /// Stock to this percent of eherbs' minimum doses (`stock`; Stage 4).
    pub stock: Option<u32>,
    /// Count and stock major blood apart from minor (`split_blood`; Stage 4).
    pub split_blood: bool,
    /// Deposit coins after buying (`deposit_coins`; Stage 4).
    pub deposit_coins: bool,
    /// The kit is a Survivalist's Kit with the Liquid Extractor: point it at
    /// a dose after healing (`distiller`; Stage 3). Learned by `analyze`
    /// when unset.
    pub distiller: Option<bool>,
}

impl HealProfile {
    /// Read a profile file's text.
    ///
    /// # Errors
    ///
    /// The text is not TOML, or names a key this does not know.
    pub fn parse(text: &str) -> Result<Self, String> {
        crate::profile_file::read(text)
    }

    /// The profile as a file's text.
    ///
    /// # Errors
    ///
    /// The profile cannot be written as TOML.
    pub fn to_toml(&self) -> Result<String, String> {
        crate::profile_file::written(self)
    }
}

/// Every setting, in the struct's order: what `;heal show` lists, the ones
/// not set among them, and what the settings menu shows.
pub const TABLE: &[Key] = &[
    Key {
        name: "container",
        label: "Herb container",
        help: "The container the herbs are kept in, by a word of its name. Healing waits for it.",
        kind: KeyKind::Text,
    },
    Key {
        name: "skip_scars",
        label: "Leave minor scars",
        help: "Treat wounds, and leave the minor scars.",
        kind: KeyKind::Toggle,
    },
    Key {
        name: "potions",
        label: "Prefer potions",
        help: "Drink rather than eat where there is a choice.",
        kind: KeyKind::Toggle,
    },
    Key {
        name: "yabathilium",
        label: "Yabathilium first for blood",
        help: "Use yabathilium before anything else for lost blood.",
        kind: KeyKind::Toggle,
    },
    Key {
        name: "blood_only",
        label: "Only treat blood",
        help: "Treat lost blood and nothing else.",
        kind: KeyKind::Toggle,
    },
    Key {
        name: "buy_missing",
        label: "Buy missing herbs",
        help: "Not built yet: eherbs buys a herb the container lacks while it heals. Stock the container with heal stock.",
        kind: KeyKind::Toggle,
    },
    Key {
        name: "stock",
        label: "Stock to (percent)",
        help: "When stocking, fill each herb to this percent of its minimum doses; 100 at most, as eherbs has it.",
        kind: KeyKind::Whole { min: 0, max: 100 },
    },
    Key {
        name: "split_blood",
        label: "Split blood",
        help: "Count and stock major blood apart from minor.",
        kind: KeyKind::Toggle,
    },
    Key {
        name: "deposit_coins",
        label: "Deposit coins",
        help: "Not built yet: eherbs deposits what is left at the bank after buying.",
        kind: KeyKind::Toggle,
    },
    Key {
        name: "distiller",
        label: "Kit has a distiller",
        help: "A Survivalist's Kit with the Liquid Extractor. Learned by analyze when unset.",
        kind: KeyKind::Toggle,
    },
];

/// The character's heal profile: `<data>/hunt/heal/<instance>_<character>.toml`.
/// `None` when the names cannot be a file name.
#[must_use]
pub fn path(dir: &Path, instance: &str, character: &str) -> Option<PathBuf> {
    crate::profile_file::path(dir, "heal", instance, character)
}

#[cfg(test)]
mod tests {
    use super::{HealProfile, TABLE};
    use crate::settings::names;

    /// `TABLE` is every field, in order: a field added to the struct and not
    /// here would never be listed by `;heal show`, nor shown in the menu.
    #[test]
    fn keys_are_every_setting() {
        let every = HealProfile {
            stock: Some(50),
            distiller: Some(true),
            ..HealProfile::default()
        };
        let table = toml::Table::try_from(&every).unwrap();
        let keys: Vec<&str> = table.keys().map(String::as_str).collect();
        assert_eq!(keys, names(TABLE));
    }
}
