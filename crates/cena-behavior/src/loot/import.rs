//! eloot's `eloot.yaml` in, a [`LootProfile`] out, with what it could not
//! carry named (`plan/31` §6).
//!
//! The file is Ruby's `YAML.dump` of one hash whose keys are symbols
//! (`:loot_types:`) and whose values are scalars or block lists
//! (`- gem`). [`yaml::read`] reads both shapes; here the leading colon comes
//! off and a list is split on its lines.
//!
//! Three fates for a key: **carried** into the profile under the name of
//! what it does; **carried for the town** verbatim, under `[town]`, for the
//! rest phase's errands (Stage 4); or **dropped**, with a note when it held
//! something that would have changed behavior.

use std::collections::BTreeMap;

use super::profile::LootProfile;
use crate::hunt::yaml;

/// What an import produced.
#[derive(Clone, Debug, PartialEq)]
pub struct Import {
    /// The profile.
    pub profile: LootProfile,
    /// What was dropped or is not built. Empty when nothing was.
    pub notes: Vec<String>,
}

impl Import {
    /// The file to write: the notes as a comment at the head, then the
    /// profile as TOML.
    ///
    /// # Errors
    ///
    /// The profile could not be written as TOML ([`LootProfile::to_toml`]).
    pub fn render(&self) -> Result<String, String> {
        let mut out = String::from("# Hydra loot profile, imported from eloot's settings.\n");
        if self.notes.is_empty() {
            out.push_str("# Everything that reaches a hunt was carried over.\n");
        } else {
            out.push_str("#\n# What the importer dropped, or has not built:\n");
            for note in &self.notes {
                out.push_str("#   ");
                out.push_str(note);
                out.push('\n');
            }
        }
        out.push('\n');
        out.push_str(&self.profile.to_toml()?);
        Ok(out)
    }
}

/// Keys whose values are the town's: selling, tipping, hoarding, banking.
/// Carried verbatim under `[town]`.
const TOWN_PREFIXES: &[&str] = &[
    "sell_",
    "locksmith_",
    "gem_",
    "alchemy_",
    "use_standard_tipping",
    "use_incremental_tipping",
    "base_tip",
    "max_tip",
    "alpha_rate",
    "tipping_test",
    "appraisal_container",
    "charm_name",
    "coin_hand_name",
    "favor_left",
    "gambling_toss_min",
    "between",
    "trash_dump_types",
    "always_check_pool",
    "display_box_contents",
    "keep_transmogs",
];

/// Keys that are eloot's own bookkeeping or display, dropped silently.
const BOOKKEEPING: &[&str] = &[
    "debug",
    "debug_file",
    "silence",
    "keep_closed",
    "track_full_sacks",
    "log_unlootables",
    "use_disk_group",
    "tipping_test",
];

/// Read eloot's settings file.
///
/// # Errors
///
/// The text is not the YAML eloot writes ([`yaml::read`]).
pub fn import(yaml_text: &str) -> Result<Import, String> {
    let read = yaml::read(yaml_text)?;
    let mut source: BTreeMap<String, String> = read
        .into_iter()
        .map(|(key, value)| (key.trim_start_matches(':').to_owned(), value))
        .collect();
    let mut profile = LootProfile::default();
    let mut notes = Vec::new();

    let list = |key: &str, source: &mut BTreeMap<String, String>| -> Vec<String> {
        source
            .remove(key)
            .map(|text| items(&text))
            .unwrap_or_default()
    };
    profile.take = list("loot_types", &mut source);
    profile.leave = list("loot_exclude", &mut source);
    profile.overflow = list("overflow_containers", &mut source);
    profile.crumbly = list("crumbly", &mut source);
    profile.unlootable = list("unlootable", &mut source);
    profile.autoclose = list("auto_close", &mut source);

    let flag = |key: &str, source: &mut BTreeMap<String, String>| -> bool {
        source.remove(key).is_some_and(|text| text == "true")
    };
    profile.defensive = flag("loot_defensive", &mut source);
    profile.disk = flag("use_disk", &mut source);
    profile.sigil_on_fail = flag("sigil_determination_on_fail", &mut source);
    profile.phase_boxes = flag("loot_phase", &mut source);

    // Skinning (`plan/31` §5): the five switches, the four names, the two lists.
    let text = |key: &str, source: &mut BTreeMap<String, String>| -> String {
        source.remove(key).unwrap_or_default().trim().to_owned()
    };
    profile.skin.enable = flag("skin_enable", &mut source);
    profile.skin.kneel = flag("skin_kneel", &mut source);
    profile.skin.spell_604 = flag("skin_604", &mut source);
    profile.skin.resolve = flag("skin_resolve", &mut source);
    profile.skin.bounty_only = flag("skin_bounty_only", &mut source);
    profile.skin.weapon = text("skin_weapon", &mut source);
    profile.skin.sheath = text("skin_sheath", &mut source);
    profile.skin.weapon_blunt = text("skin_weapon_blunt", &mut source);
    profile.skin.sheath_blunt = text("skin_sheath_blunt", &mut source);
    profile.skin.exclude = list("skin_exclude", &mut source);
    profile.skin.unskinnable = list("unskinnable", &mut source);
    if profile.skin.enable && profile.skin.bounty_only {
        notes.push(
            "skin_bounty_only was on: not built, every eligible corpse is skinned".to_owned(),
        );
    }

    for (key, what) in [
        ("loot_keep", "names kept whatever their kind"),
        ("critter_exclude", "corpses never searched"),
    ] {
        if let Some(text) = source.remove(key)
            && !text.is_empty()
        {
            notes.push(format!("{key} ({what}) is not built; dropped: {text}"));
        }
    }
    if source
        .remove("use_bloodbands")
        .is_some_and(|text| text == "true")
    {
        notes.push("use_bloodbands was on: blood bands are not built; dropped".to_owned());
    }

    for (key, value) in source {
        if BOOKKEEPING.contains(&key.as_str()) {
            continue;
        }
        if TOWN_PREFIXES
            .iter()
            .any(|prefix| key == *prefix || key.starts_with(prefix))
        {
            profile.town.insert(key, town_value(&value));
            continue;
        }
        notes.push(format!(
            "{key} is not a setting the importer knows; dropped: {value:?}"
        ));
    }
    notes.extend(profile.problems());
    Ok(Import { profile, notes })
}

/// A block list's lines, or a scalar as one item; nothing for empty.
fn items(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// A town key's value as TOML: a list stays a list, `true`/`false` a
/// boolean, a number a number, the rest a string.
fn town_value(text: &str) -> toml::Value {
    if text.contains('\n') {
        return toml::Value::Array(items(text).into_iter().map(toml::Value::String).collect());
    }
    match text {
        "true" => toml::Value::Boolean(true),
        "false" => toml::Value::Boolean(false),
        _ => text
            .parse::<i64>()
            .map(toml::Value::Integer)
            .or_else(|_| text.parse::<f64>().map(toml::Value::Float))
            .unwrap_or_else(|_| toml::Value::String(text.to_owned())),
    }
}
