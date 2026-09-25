//! bigshot's `jewel <mnemonic>` routine verb (`cmd_jewel`,
//! `bigshot.lic:5149-5215`): a gemstone jewel's power activated by its
//! mnemonic. The game's command for it is named for the game, so it lives
//! here ([`super`]).

/// The jewel powers bigshot knows, by mnemonic, with the name the Cooldowns
/// dialog lists each by (`bigshot.lic:5177-5199`). bigshot refuses any other
/// mnemonic rather than guess.
const JEWELS: &[(&str, &str)] = &[
    ("bloodboil", "Blood Boil"),
    ("spellblade", "Spellblade's Fury"),
    ("arcascend", "Arcanist's Ascendancy"),
    ("geospite", "Geomancer's Spite"),
    ("forceofwill", "Force of Will"),
    ("arcaneintensity", "Arcane Intensity"),
    ("arcaneopus", "Arcane Opus"),
    ("bloodsiphon", "Blood Siphon"),
    ("bloodwell", "Blood Wellspring"),
    ("epossess", "Evanescent Possession"),
    ("manawellspring", "Mana Wellspring"),
    ("spiritwell", "Spirit Wellspring"),
    ("stamwell", "Stamina Wellspring"),
    ("terrortribute", "Terror's Tribute"),
    ("arcblade", "Arcanist's Blade"),
    ("arcwill", "Arcanist's Will"),
    ("imaerabalm", "Imaera's Balm"),
    ("reckless", "Reckless Precision"),
    ("unearthchains", "Unearthly Chains"),
    ("witchhunt", "Witchhunter's Ascendancy"),
    ("manashield", "Mana Shield"),
    ("arcaneaegis", "Arcane Aegis"),
];

/// The command that activates the jewel power `mnemonic`, and the name its
/// cooldown is listed by; `None` for a mnemonic bigshot does not know.
pub(crate) fn activate(mnemonic: &str) -> Option<(String, &'static str)> {
    let mnemonic = mnemonic.trim().to_ascii_lowercase();
    let (_, name) = JEWELS.iter().find(|(m, _)| *m == mnemonic)?;
    Some((format!("gemstone activate {mnemonic}"), name))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_known_jewel_is_activated_by_its_mnemonic_and_another_is_not() {
        assert_eq!(
            super::activate("Spellblade"),
            Some((
                "gemstone activate spellblade".to_owned(),
                "Spellblade's Fury"
            ))
        );
        assert_eq!(super::activate("sparkle"), None);
    }
}
