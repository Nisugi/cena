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
//! Hydra's (the author: *"sure alt + number = set"*). The sending actions
//! have Wrayth's keys (`plan/52` §3, step 3): `NumpadEnter` sends or repeats,
//! Ctrl with an Enter repeats the last command and Alt the one before it,
//! Up and Down walk what was typed, and Escape clears the input. The window
//! in use scrolls with the page keys (step 4): a page, with Shift a line,
//! with Ctrl to the oldest or newest line. Ctrl+Tab turns the tabs of the
//! window in use, with Shift back (step 5); choosing the window has no key,
//! a click chooses it (§6 answer 3). Ctrl+F finds, F3 the one found before
//! and Shift+F3 the one after (step 6). Tab targets the next creature and
//! Shift+Tab the one before (step 7; the author: *"I prefer tab for
//! targetting"*). Each later step of the plan adds the
//! keys of the actions it builds.

use std::collections::BTreeMap;

use super::binding::{Action, Macro};
use super::{ALT, CTRL, Chord, SHIFT};

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

/// The sending actions' keys, and the modifiers held.
const SENDING: [(&str, u8, Action); 8] = [
    ("NumpadEnter", 0, Action::SendOrRepeat),
    ("Enter", CTRL, Action::RepeatLast),
    ("NumpadEnter", CTRL, Action::RepeatLast),
    ("Enter", ALT, Action::RepeatSecondLast),
    ("NumpadEnter", ALT, Action::RepeatSecondLast),
    ("ArrowUp", 0, Action::HistoryBack),
    ("ArrowDown", 0, Action::HistoryForward),
    ("Escape", 0, Action::ClearInput),
];

/// The scrolling actions' keys, and the modifiers held.
const SCROLLING: [(&str, u8, Action); 8] = [
    ("PageUp", 0, Action::ScrollPageUp),
    ("PageDown", 0, Action::ScrollPageDown),
    ("PageUp", SHIFT, Action::ScrollLineUp),
    ("PageDown", SHIFT, Action::ScrollLineDown),
    ("Home", CTRL, Action::ScrollTop),
    ("PageUp", CTRL, Action::ScrollTop),
    ("End", CTRL, Action::ScrollBottom),
    ("PageDown", CTRL, Action::ScrollBottom),
];

/// The tabs' keys, Find's and the targets', and the modifiers held.
const TABS: [(&str, u8, Action); 7] = [
    ("Tab", CTRL, Action::NextTab),
    ("Tab", CTRL | SHIFT, Action::PreviousTab),
    ("KeyF", CTRL, Action::Find),
    ("F3", 0, Action::FindNext),
    ("F3", SHIFT, Action::FindPrevious),
    ("Tab", 0, Action::TargetNext),
    ("Tab", SHIFT, Action::TargetPrevious),
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
    for (key, held, action) in SENDING.into_iter().chain(SCROLLING).chain(TABS) {
        keys.insert(chord(key, held), Macro::Act(action));
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
            59,
            "eleven walks, eleven peers, four marks, eight sending, eight scrolling, two tabs, three finding, two targets, ten sets"
        );
        for (chord, made) in &defaults {
            let written = chord.written();
            assert_eq!(Chord::parse(&written).as_ref(), Ok(chord), "{written}");
            assert_eq!(chord.refused(), None, "{written}");
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
