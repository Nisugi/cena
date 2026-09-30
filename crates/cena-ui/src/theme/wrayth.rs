//! Wrayth's colours as a theme's pins (`plan/57` step 8): the `<presets>` of
//! a Wrayth settings file, each `<p id color>` read to `#rrggbb` by the
//! trigger importer (`cena-behavior`'s `triggers/wrayth.rs`, which resolves
//! its `@N` palette entries), become pins on the tokens that mean the same.
//! A preset Hydra has no token for is noted, never guessed at.

use std::collections::BTreeMap;

use super::{Rgb, Token, parse_hex};

/// Wrayth's preset ids and the tokens they mean.
const PRESETS: &[(&str, Token)] = &[
    ("roomName", Token::RoomName),
    ("bold", Token::Creature),
    ("monsterbold", Token::Creature),
    ("speech", Token::Speech),
    ("whisper", Token::Whisper),
    ("thought", Token::Thought),
    ("link", Token::Link),
    ("links", Token::Link),
];

/// `presets`, Wrayth's by id as `#rrggbb`, as pins; and a note for each that
/// is not a token of Hydra's or not a colour.
#[must_use]
pub fn pins_from_wrayth(presets: &BTreeMap<String, String>) -> (BTreeMap<Token, Rgb>, Vec<String>) {
    let mut pins = BTreeMap::new();
    let mut notes = Vec::new();
    for (id, colour) in presets {
        let Some((_, token)) = PRESETS
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(id))
        else {
            notes.push(format!(
                "the preset `{id}` is not a colour Hydra names, and is left out"
            ));
            continue;
        };
        match parse_hex(colour) {
            Some(rgb) => {
                pins.insert(*token, rgb);
            }
            None => notes.push(format!(
                "`{id}`'s colour `{colour}` is not #rrggbb, and is left out"
            )),
        }
    }
    (pins, notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_presets_hydra_names_become_pins_and_the_rest_are_noted() {
        let presets: BTreeMap<String, String> = [
            ("roomName", "#ecc013"),
            ("bold", "#ff3300"),
            ("speech", "#f0eee8"),
            ("familiar", "#00ff00"),
            ("thought", "green"),
        ]
        .into_iter()
        .map(|(id, colour)| (id.to_owned(), colour.to_owned()))
        .collect();
        let (pins, notes) = pins_from_wrayth(&presets);
        assert_eq!(pins.get(&Token::RoomName), Some(&[0xec, 0xc0, 0x13]));
        assert_eq!(pins.get(&Token::Creature), Some(&[0xff, 0x33, 0x00]));
        assert_eq!(pins.get(&Token::Speech), Some(&[0xf0, 0xee, 0xe8]));
        assert_eq!(pins.len(), 3);
        assert_eq!(notes.len(), 2, "{notes:?}");
        assert!(notes[0].contains("familiar"));
        assert!(notes[1].contains("thought"));
    }
}
