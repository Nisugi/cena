//! Hydra's default keys (`plan/52` §3): set 0 as a player first finds it.
//! They live here, in the code, and the keybinds file holds only what the
//! player added or changed (the author: *"yes defaults can live in code"*),
//! so a default added in a later Hydra reaches every player who never
//! changed that key.
//!
//! The numpad walks as Wrayth's stock key set has it, and `VellumFE`'s and
//! Genie's copy it (`plan/52` §1); Shift with it peers that way (the author,
//! 2026-09-28: *"shift + numpad = peer direction"*). Alt with a digit
//! chooses that macro set, as Wrayth's does, the one action with a key of
//! Hydra's (the author: *"sure alt + number = set"*). Each later step of the
//! plan adds the keys of the actions it builds.

use std::collections::BTreeMap;

use super::binding::{Action, Macro};
use super::{ALT, Chord, SHIFT};

/// Each numpad key that walks, and where.
const WALKS: [(&str, &str); 11] = [
    ("Numpad8", "north"),
    ("Numpad2", "south"),
    ("Numpad6", "east"),
    ("Numpad4", "west"),
    ("Numpad9", "northeast"),
    ("Numpad7", "northwest"),
    ("Numpad3", "southeast"),
    ("Numpad1", "southwest"),
    ("Numpad5", "out"),
    ("Numpad0", "down"),
    ("NumpadDecimal", "up"),
];

/// The numpad's four marks, as Wrayth binds them.
const MARKS: [(&str, &str); 4] = [
    ("NumpadAdd", "look"),
    ("NumpadSubtract", "info"),
    ("NumpadMultiply", "exp"),
    ("NumpadDivide", "health"),
];

/// Every key Hydra binds, and what each does.
pub(crate) fn defaults() -> BTreeMap<Chord, Macro> {
    let chord = |key: &str, held: u8| Chord {
        key: key.to_owned(),
        held,
    };
    let mut keys = BTreeMap::new();
    for (key, way) in WALKS {
        keys.insert(chord(key, 0), Macro::Send(way.to_owned()));
        keys.insert(chord(key, SHIFT), Macro::Send(format!("peer {way}")));
    }
    for (key, line) in MARKS {
        keys.insert(chord(key, 0), Macro::Send(line.to_owned()));
    }
    for set in 0..super::SETS {
        keys.insert(
            chord(&format!("Digit{set}"), ALT),
            Macro::Act(Action::Set(set)),
        );
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every default is a key this build can see, one that does not type,
    /// and a macro that can be done as written: a default the file would
    /// refuse would be a default nobody gets.
    #[test]
    fn every_default_is_one_the_file_would_take() {
        let defaults = defaults();
        assert_eq!(
            defaults.len(),
            36,
            "eleven walks, eleven peers, four marks, ten sets"
        );
        for (chord, made) in &defaults {
            let written = chord.written();
            assert_eq!(Chord::parse(&written).as_ref(), Ok(chord), "{written}");
            assert!(!chord.types(), "{written} types");
            assert_eq!(made.check(), Ok(()), "{written}");
        }
        let peer = Chord::parse("Shift+Numpad7").expect("a key");
        assert_eq!(
            defaults.get(&peer),
            Some(&Macro::Send("peer northwest".to_owned()))
        );
        assert_eq!(
            defaults.get(&Chord::parse("Alt+Digit3").expect("a key")),
            Some(&Macro::Act(Action::Set(3)))
        );
    }
}
