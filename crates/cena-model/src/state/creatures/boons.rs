//! A creature's boons: what its name's adjectives say it can do.
//!
//! bigshot's `initialize_boon_data` (`bigshot.lic:3147-3182`), whole: 34
//! traits, each with the adjectives that name it. The feed shows a boon
//! only as an adjective before the creature's name, and `assess` lists
//! them (`The <creature> appears to be flickering and robust.`,
//! `check_boons`, `:8085-8120`), so the table is how a trait is known at
//! all. [`assessed`] reads that line.
//!
//! The adjectives the bestiary strips before a lookup
//! (`instance.rs`'s `BOON_ADJECTIVES`, from `creature.rb`) are a different
//! list: Lich's, for finding the template, where this is bigshot's, for
//! knowing the trait. They mostly agree and are kept apart because each
//! is its source's.

/// Each trait, with the adjectives that name it.
pub const TRAITS: &[(&str, &[&str])] = &[
    ("blink", &["flickering", "wavering"]),
    ("bolt_shield", &["shielded"]),
    ("boosted_hp", &["robust", "stalwart"]),
    ("boosted_mana", &["luminous", "lustrous"]),
    ("boosted_defense", &["sinuous", "flexile"]),
    ("boosted_offense", &["combative", "belligerent"]),
    ("cheat_death", &["glorious", "illustrious"]),
    ("confuse", &["blurry", "shifting"]),
    ("counter_attack", &["apt", "ready"]),
    ("crit_death_immune", &["resolute", "unflinching"]),
    ("crit_padding", &["stout", "hardy"]),
    ("crit_weighting", &["shimmering", "gleaming"]),
    ("damage_padding", &["flinty", "tough"]),
    ("dmg_weighting", &["barbed", "spiny"]),
    ("diseased", &["pestilent", "afflicted", "diseased"]),
    ("dispelling", &["dazzling", "flashy"]),
    ("elem_flares", &["glittering"]),
    ("elemental_negation", &["sparkling", "shining"]),
    ("extra_elem", &["glowing"]),
    ("extra_spirit", &["radiant"]),
    ("extra_other", &["twinkling"]),
    ("ethereal", &["ethereal", "wispy", "ghostly"]),
    ("frenzy", &["raging", "frenzied"]),
    ("jack", &["adroit", "deft"]),
    ("magic_resistance", &["rune-covered", "tattooed"]),
    ("mind_blast", &["canny", "keen"]),
    ("parting_shot", &["dreary", "drab"]),
    ("physical_negation", &["indistinct", "nebulous"]),
    ("poisonous", &["sickly green", "oozing"]),
    ("regen", &["slimy", "muculent"]),
    ("soul", &["tenebrous", "shadowy"]),
    ("stun_immune", &["steadfast", "unyielding"]),
    ("terrifying", &["ghastly", "grotesque"]),
    ("weaken", &["spindly", "lanky"]),
];

/// The trait an adjective names, ignoring case.
#[must_use]
pub fn trait_of(adjective: &str) -> Option<&'static str> {
    let adjective = adjective.trim();
    TRAITS.iter().find_map(|(name, adjectives)| {
        adjectives
            .iter()
            .any(|a| a.eq_ignore_ascii_case(adjective))
            .then_some(*name)
    })
}

/// The traits an `assess` line names, in order and once each: `The
/// <creature> appears to be flickering, robust and keen.` `None`: not
/// such a line. An adjective the table does not know is skipped, as
/// bigshot's `compact` skips it.
#[must_use]
pub fn assessed(line: &str) -> Option<Vec<&'static str>> {
    let (_, rest) = line.split_once("appears to be ")?;
    let phrase = rest.split('.').next().unwrap_or(rest).to_ascii_lowercase();
    let mut traits = Vec::new();
    for part in phrase.split(',').flat_map(|p| p.split(" and ")) {
        if let Some(name) = trait_of(part)
            && !traits.contains(&name)
        {
            traits.push(name);
        }
    }
    Some(traits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_assess_line_is_read_as_traits() {
        assert_eq!(
            assessed("The kobold appears to be flickering, robust and sickly green."),
            Some(vec!["blink", "boosted_hp", "poisonous"])
        );
        assert_eq!(assessed("The kobold appears to be calm."), Some(vec![]));
        assert_eq!(assessed("You see nothing unusual."), None);
        assert_eq!(TRAITS.len(), 34);
    }
}
