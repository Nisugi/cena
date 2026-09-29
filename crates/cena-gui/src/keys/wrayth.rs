//! A Wrayth key set read (`plan/52` §4, step 9): the `<macros>` of a Wrayth
//! settings file, each of its ten sets (`<keys id name>`, its keys `<k key
//! action/>`) into the Hydra set of the same number. No XML crate: the file
//! is Wrayth's own flat shape, read as `crates/cena-behavior/src/triggers/
//! wrayth.rs` reads its highlights.
//!
//! A key is Wrayth's name for it, `Alt-Ctrl-E`, `Keypad 8`, `Page Up`, made
//! Hydra's. What it does:
//!
//! - a `{Token}` is Hydra's action for it (`{HistoryPrev}` is `history_back`,
//!   `{MacroSet}3` is `macro_set_3`), and a token Hydra has no action for is
//!   said, not bound;
//! - text ending `\r` sends, and without it fills the input; `\r` inside it
//!   breaks one command from the next, `\p` is a second's wait, `\x` (clear
//!   the line first) is dropped, since a sent command never touches the
//!   input; `@` and `\?` are said, since Hydra has no cursor mark or prompt.

use super::binding::{Action, Macro};
use super::{Chord, SETS};

/// A key of a Wrayth set, made Hydra's: its set, its key and what it does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Imported {
    /// The set, 0 to 9.
    pub(crate) set: u8,
    /// The key.
    pub(crate) chord: Chord,
    /// What it does.
    pub(crate) does: Macro,
}

/// Wrayth's tokens Hydra has an action for.
const TOKENS: [(&str, Action); 13] = [
    ("HistoryPrev", Action::HistoryBack),
    ("HistoryNext", Action::HistoryForward),
    ("RepeatLast", Action::RepeatLast),
    ("RepeatSecondToLast", Action::RepeatSecondLast),
    ("ReturnOrRepeatLast", Action::SendOrRepeat),
    ("PageUp", Action::ScrollPageUp),
    ("PageDown", Action::ScrollPageDown),
    ("LineUp", Action::ScrollLineUp),
    ("LineDown", Action::ScrollLineDown),
    ("BufferTop", Action::ScrollTop),
    ("BufferBottom", Action::ScrollBottom),
    ("CycleWindows", Action::NextWindow),
    ("CycleWindowsReverse", Action::PreviousWindow),
];

/// Wrayth's names for keys that are not a letter, a digit or an F key, and
/// Hydra's.
const KEYS: [(&str, &str); 30] = [
    ("Keypad 0", "Numpad0"),
    ("Keypad 1", "Numpad1"),
    ("Keypad 2", "Numpad2"),
    ("Keypad 3", "Numpad3"),
    ("Keypad 4", "Numpad4"),
    ("Keypad 5", "Numpad5"),
    ("Keypad 6", "Numpad6"),
    ("Keypad 7", "Numpad7"),
    ("Keypad 8", "Numpad8"),
    ("Keypad 9", "Numpad9"),
    ("Keypad +", "NumpadAdd"),
    ("Keypad -", "NumpadSubtract"),
    ("Keypad *", "NumpadMultiply"),
    ("Keypad /", "NumpadDivide"),
    ("Keypad .", "NumpadDecimal"),
    ("Keypad Enter", "NumpadEnter"),
    ("Page Up", "PageUp"),
    ("Page Down", "PageDown"),
    ("UP", "ArrowUp"),
    ("DOWN", "ArrowDown"),
    ("LEFT", "ArrowLeft"),
    ("RIGHT", "ArrowRight"),
    ("Esc", "Escape"),
    ("Enter", "Enter"),
    ("Tab", "Tab"),
    ("Home", "Home"),
    ("End", "End"),
    ("Insert", "Insert"),
    ("Delete", "Delete"),
    ("Space", "Space"),
];

/// Read `text`, a Wrayth settings file: every key of its `<macros>` Hydra
/// can bind, and what it could not, in words for the player.
pub(crate) fn read(text: &str) -> (Vec<Imported>, Vec<String>) {
    let mut keys = Vec::new();
    let mut said = Vec::new();
    let Some(macros) = between(text, "<macros", "</macros>") else {
        return (
            keys,
            vec!["It has no <macros>: no key set to import.".to_owned()],
        );
    };
    for set in macros.split("<keys ").skip(1) {
        let head = set.split('>').next().unwrap_or_default();
        let Some(id) = attribute(head, "id").and_then(|id| id.parse::<u8>().ok()) else {
            said.push("A <keys> with no set number was left out.".to_owned());
            continue;
        };
        if id >= SETS {
            said.push(format!("Set {id} was left out: Hydra has sets 0 to 9."));
            continue;
        }
        let body = set.split("</keys>").next().unwrap_or_default();
        for k in body.split("<k ").skip(1) {
            let (Some(key), Some(action)) = (attribute(k, "key"), attribute(k, "action")) else {
                continue;
            };
            match one(&key, &action) {
                Ok(Some((chord, does))) => keys.push(Imported {
                    set: id,
                    chord,
                    does,
                }),
                Ok(None) => {}
                Err(why) => said.push(format!("{key} ({}): {why}", in_set(id))),
            }
        }
    }
    (keys, said)
}

/// `set 3`, or `the default set` for Wrayth's set 0.
fn in_set(set: u8) -> String {
    if set == 0 {
        "the default set".to_owned()
    } else {
        format!("set {set}")
    }
}

/// One Wrayth key and its action, made Hydra's; `None` for one that does
/// nothing.
fn one(key: &str, action: &str) -> Result<Option<(Chord, Macro)>, String> {
    let chord = chord(key)?;
    if let Some(why) = chord.refused() {
        return Err(why);
    }
    let Some(does) = macro_of(action)? else {
        return Ok(None);
    };
    does.check()?;
    Ok(Some((chord, does)))
}

/// A Wrayth key's name, `Alt-Ctrl-Keypad Enter`, as a Hydra chord.
fn chord(key: &str) -> Result<Chord, String> {
    let mut rest = key.trim();
    let mut written = String::new();
    loop {
        let before = rest;
        for (wrayth, hydra) in [("Alt-", "Alt+"), ("Ctrl-", "Ctrl+"), ("Shift-", "Shift+")] {
            if let Some(after) = rest.strip_prefix(wrayth) {
                written.push_str(hydra);
                rest = after;
            }
        }
        if rest == before {
            break;
        }
    }
    let name = KEYS
        .iter()
        .find(|(wrayth, _)| wrayth.eq_ignore_ascii_case(rest))
        .map(|(_, hydra)| (*hydra).to_owned())
        .or_else(|| {
            let mut chars = rest.chars();
            match (chars.next(), chars.next()) {
                (Some(letter), None) if letter.is_ascii_alphabetic() => {
                    Some(format!("Key{}", letter.to_ascii_uppercase()))
                }
                (Some(digit), None) if digit.is_ascii_digit() => Some(format!("Digit{digit}")),
                _ => (rest.starts_with(['F', 'f']) && rest[1..].parse::<u8>().is_ok())
                    .then(|| rest.to_ascii_uppercase()),
            }
        })
        .ok_or_else(|| format!("Hydra has no key called {rest}"))?;
    written.push_str(&name);
    Chord::parse(&written)
}

/// What a Wrayth action does, as a Hydra macro; `None` for nothing.
fn macro_of(action: &str) -> Result<Option<Macro>, String> {
    if let Some(token) = action.strip_prefix('{') {
        let (name, after) = token.split_once('}').unwrap_or((token, ""));
        if name == "MacroSet"
            && let Ok(set) = after.trim().parse::<u8>()
            && set < SETS
        {
            return Ok(Some(Macro::Act(Action::Set(set))));
        }
        return TOKENS
            .iter()
            .find(|(wrayth, _)| *wrayth == name)
            .map(|(_, action)| Some(Macro::Act(*action)))
            .ok_or_else(|| format!("Hydra has no action for {{{name}}}"));
    }
    if action.contains('@') || action.contains("\\?") {
        return Err("Hydra has no cursor mark (@) or prompt (\\?) in a macro".to_owned());
    }
    let sends = action.trim_end().ends_with("\\r");
    let text = action.replace("\\x", "").replace("\\p", "\\rs1\\r");
    let text = text.replace("\\r", "\r");
    let text = text.trim_matches('\r');
    if text.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(if sends {
        Macro::Send(text.to_owned())
    } else {
        Macro::Fill(text.to_owned())
    }))
}

/// The text between `open`, a tag's start, and `close`.
fn between<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = text.find(open)?;
    let end = text[start..].find(close)? + start;
    Some(&text[start..end])
}

/// The value of attribute `name` in a tag's text, quoted either way, its
/// entities read.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let mut at = 0;
    while let Some(found) = tag[at..].find(name) {
        let start = at + found;
        at = start + name.len();
        let before = tag[..start].chars().next_back();
        if before.is_some_and(|c| !c.is_whitespace()) {
            continue;
        }
        let rest = tag[at..].trim_start().strip_prefix('=')?.trim_start();
        let quote = rest.chars().next().filter(|c| *c == '\'' || *c == '"')?;
        let value = rest[1..].split(quote).next()?;
        return Some(
            value
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"")
                .replace("&apos;", "'")
                .replace("&amp;", "&"),
        );
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const MNSTR: &str = include_str!("../../tests/fixtures/wrayth_macros.xml");

    fn find<'a>(keys: &'a [Imported], set: u8, key: &str) -> Option<&'a Macro> {
        let chord = Chord::parse(key).ok()?;
        keys.iter()
            .find(|imported| imported.set == set && imported.chord == chord)
            .map(|imported| &imported.does)
    }

    /// The author's own Wrayth set, stock set 0 and a set 1 of their own:
    /// its keys made Hydra's, sending, filling and acting, and what Hydra
    /// has no action for said, each by name.
    #[test]
    fn the_authors_wrayth_set_imports() {
        let (keys, said) = read(MNSTR);
        let send = |text: &str| Some(Macro::Send(text.to_owned()));
        assert_eq!(find(&keys, 0, "Numpad8").cloned(), send("n"), "\\x dropped");
        assert_eq!(find(&keys, 0, "NumpadAdd").cloned(), send("look"));
        assert_eq!(
            find(&keys, 0, "Alt+KeyC").cloned(),
            send("xml toggle containers")
        );
        assert_eq!(
            find(&keys, 0, "ArrowUp"),
            Some(&Macro::Act(Action::HistoryBack))
        );
        assert_eq!(
            find(&keys, 0, "Alt+NumpadEnter"),
            Some(&Macro::Act(Action::RepeatSecondLast))
        );
        assert_eq!(
            find(&keys, 0, "Ctrl+Home"),
            Some(&Macro::Act(Action::ScrollTop))
        );
        assert_eq!(
            find(&keys, 0, "Alt+Digit3"),
            Some(&Macro::Act(Action::Set(3)))
        );
        assert_eq!(find(&keys, 0, "Tab"), Some(&Macro::Act(Action::NextWindow)));
        assert_eq!(
            find(&keys, 1, "F1"),
            Some(&Macro::Fill("prep 111".to_owned())),
            "no \\r: it types"
        );
        assert_eq!(keys.iter().filter(|key| key.set == 1).count(), 3);
        for token in [
            "{Rest}",
            "{ToggleMusic}",
            "{Copy}",
            "{PauseScript}",
            "{ExportDialog}",
        ] {
            assert!(
                said.iter().any(|why| why.contains(token)),
                "{token} said: {said:?}"
            );
        }
        assert_eq!(keys.len() + said.len(), 63 + 3, "every key bound or said");
    }

    /// A wait, a command break, the entities, a file with no macros, and a
    /// key that types, each as the plan says.
    #[test]
    fn waits_breaks_and_what_is_refused() {
        let file = r#"<settings><macros set="1"><keys id='2' name='x'><k key="F5" action="stance off\pattack\r"/><k key="Q" action="look\r"/><k key="F6" action="say &quot;hi&quot;\r"/><k key="F7" action="look @"/></keys></macros></settings>"#;
        let (keys, said) = read(file);
        assert_eq!(
            find(&keys, 2, "F5"),
            Some(&Macro::Send("stance off\rs1\rattack".to_owned()))
        );
        assert_eq!(
            find(&keys, 2, "F6"),
            Some(&Macro::Send("say \"hi\"".to_owned()))
        );
        assert_eq!(said.len(), 2, "{said:?}");
        assert!(said[0].starts_with("Q (set 2): KeyQ types"), "{said:?}");
        assert!(said[1].contains("cursor mark"), "{said:?}");
        let (none, said) = read("<settings/>");
        assert!(none.is_empty());
        assert_eq!(said.len(), 1);
    }
}
