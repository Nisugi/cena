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

use crate::hunt::chain;

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
        toml::from_str(text).map_err(|e| e.to_string())
    }

    /// The profile as a file's text.
    ///
    /// # Errors
    ///
    /// The profile cannot be written as TOML.
    pub fn to_toml(&self) -> Result<String, String> {
        toml::to_string_pretty(self).map_err(|e| e.to_string())
    }
}

/// The character's heal profile: `<data>/hunt/heal/<instance>_<character>.toml`.
/// `None` when the names cannot be a file name.
#[must_use]
pub fn path(dir: &Path, instance: &str, character: &str) -> Option<PathBuf> {
    let file = chain::file_name(&format!("{instance}_{character}"))?;
    Some(dir.join("hunt").join("heal").join(format!("{file}.toml")))
}
