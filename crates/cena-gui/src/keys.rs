//! Keybinds (`plan/47` step 7): a key, and the line it sends on the
//! character whose play window has the keyboard, as if typed there.
//!
//! **A key is named by its winit code** -- `Numpad8`, `F13`, `KeyA`,
//! `ArrowUp` -- with `ctrl+`, `shift+`, `alt+` or `cmd+` before it. The
//! physical key, not the character it types, so a binding means the same key
//! on every keyboard layout, and a numpad key is never its digit: egui folds
//! `Numpad8` into `8` (`egui-winit/src/lib.rs:1542` in the fork), which is
//! why the numpad comes through the author's fork instead
//! (`eframe::Frame::numpad_keys`), with `NumLock` read from each press.
//!
//! The keys come two ways. The numpad's through the fork's channel, which
//! catches only the numpad keys that are bound, so an unbound one still
//! types. Every other key through egui, which names F1-F35, the arrows and
//! the rest; a bound key's press is taken before anything else sees it. A
//! key that types -- a letter, a digit, a mark, with no Ctrl, Alt or Cmd --
//! never fires a binding, or nobody could type it.
//!
//! **Not yet** (`plan/47` step 7): a key egui has no name for -- Pause,
//! Scroll Lock, Print Screen, and macOS's Clear, which winit reports as
//! `NumLock` -- needs the fork's hook widened from the numpad to every key,
//! a change to the author's repository. The file names such a key and is
//! told so, rather than a binding that silently never fires.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use egui::{Key, Modifiers};

/// The keybinds file, in the data folder.
pub(crate) const FILE: &str = "keybinds.toml";

/// Ctrl, in [`Chord::held`].
pub(crate) const CTRL: u8 = 1;
/// Shift.
pub(crate) const SHIFT: u8 = 2;
/// Alt, Option on macOS.
pub(crate) const ALT: u8 = 4;
/// Cmd on macOS, the Windows key elsewhere.
pub(crate) const CMD: u8 = 8;

/// A key with the modifiers held.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Chord {
    /// The key, as winit names its code.
    pub(crate) key: String,
    /// The modifiers held: [`CTRL`], [`SHIFT`], [`ALT`], [`CMD`].
    pub(crate) held: u8,
}

impl Chord {
    /// `key` held with `modifiers`.
    pub(crate) fn of(key: &str, modifiers: Modifiers) -> Self {
        let bit = |on: bool, bit: u8| if on { bit } else { 0 };
        Self {
            key: key.to_owned(),
            held: bit(modifiers.ctrl, CTRL)
                | bit(modifiers.shift, SHIFT)
                | bit(modifiers.alt, ALT)
                | bit(modifiers.mac_cmd, CMD),
        }
    }

    /// A chord as written, `ctrl+shift+F1`: modifiers in any order and case,
    /// the key last, by its winit name or the fork's numpad name (`num_8`).
    ///
    /// # Errors
    ///
    /// What is wrong with it, in words for the player.
    pub(crate) fn parse(text: &str) -> Result<Self, String> {
        let mut chord = Self {
            key: String::new(),
            held: 0,
        };
        let parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let Some((key, held)) = parts.split_last() else {
            return Err(format!("`{text}` names no key."));
        };
        for modifier in held {
            match modifier.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => chord.held |= CTRL,
                "shift" => chord.held |= SHIFT,
                "alt" | "option" => chord.held |= ALT,
                "cmd" | "command" | "super" | "win" => chord.held |= CMD,
                other => {
                    return Err(format!(
                        "`{other}` in `{text}` is not Ctrl, Shift, Alt or Cmd."
                    ));
                }
            }
        }
        chord.key = known(key).map_err(|why| format!("`{text}`: {why}"))?;
        Ok(chord)
    }

    /// Whether this chord types something rather than commanding: a letter,
    /// a digit, a mark or the space, with no Ctrl, Alt or Cmd held.
    fn types(&self) -> bool {
        self.held & (CTRL | ALT | CMD) == 0 && TYPING.contains(&self.key.as_str())
    }
}

/// The keys that type, by winit name.
const TYPING: &[&str] = &[
    "KeyA",
    "KeyB",
    "KeyC",
    "KeyD",
    "KeyE",
    "KeyF",
    "KeyG",
    "KeyH",
    "KeyI",
    "KeyJ",
    "KeyK",
    "KeyL",
    "KeyM",
    "KeyN",
    "KeyO",
    "KeyP",
    "KeyQ",
    "KeyR",
    "KeyS",
    "KeyT",
    "KeyU",
    "KeyV",
    "KeyW",
    "KeyX",
    "KeyY",
    "KeyZ",
    "Digit0",
    "Digit1",
    "Digit2",
    "Digit3",
    "Digit4",
    "Digit5",
    "Digit6",
    "Digit7",
    "Digit8",
    "Digit9",
    "Minus",
    "Equal",
    "Comma",
    "Period",
    "Slash",
    "Backslash",
    "Semicolon",
    "Quote",
    "Backquote",
    "BracketLeft",
    "BracketRight",
    "Space",
];

/// The sixteen numpad keys: winit's name, and the fork's.
const NUMPAD: &[(&str, &str)] = &[
    ("Numpad0", "num_0"),
    ("Numpad1", "num_1"),
    ("Numpad2", "num_2"),
    ("Numpad3", "num_3"),
    ("Numpad4", "num_4"),
    ("Numpad5", "num_5"),
    ("Numpad6", "num_6"),
    ("Numpad7", "num_7"),
    ("Numpad8", "num_8"),
    ("Numpad9", "num_9"),
    ("NumpadAdd", "num_plus"),
    ("NumpadSubtract", "num_minus"),
    ("NumpadMultiply", "num_multiply"),
    ("NumpadDivide", "num_divide"),
    ("NumpadEnter", "num_enter"),
    ("NumpadDecimal", "num_decimal"),
];

/// Keys winit names that no path reaches yet: they need the fork's hook
/// widened (this module's docs).
const NOT_YET: &[&str] = &[
    "Pause",
    "ScrollLock",
    "PrintScreen",
    "NumLock",
    "ContextMenu",
    "CapsLock",
];

/// The winit name `key` is written as, checked: a key this build can see.
fn known(key: &str) -> Result<String, String> {
    if let Some((winit, _)) = NUMPAD
        .iter()
        .find(|(winit, fork)| winit.eq_ignore_ascii_case(key) || fork.eq_ignore_ascii_case(key))
    {
        return Ok((*winit).to_owned());
    }
    if let Some(name) = Key::ALL
        .iter()
        .filter_map(|key| winit_name(*key))
        .find(|name| name.eq_ignore_ascii_case(key))
    {
        return Ok(name);
    }
    if NOT_YET.iter().any(|name| name.eq_ignore_ascii_case(key)) {
        return Err(format!(
            "Hydra cannot see {key} yet: it needs the egui fork's key hook widened (plan/47 step 7)."
        ));
    }
    Err(format!(
        "{key} is not a key winit names; see the key names in plan/47 step 7."
    ))
}

/// winit's name for the physical key egui calls `key`: `KeyA` for `A`,
/// `Digit1` for `Num1`, the same name for the function keys, the arrows and
/// the rest; `None` for an egui key that is a character and not a key
/// (`Plus`, `Colon`, `Pipe`...).
pub(crate) fn winit_name(key: Key) -> Option<String> {
    let name = format!("{key:?}");
    if name.len() == 1 {
        return Some(format!("Key{name}"));
    }
    if let Some(digit) = name.strip_prefix("Num")
        && digit.len() == 1
    {
        return Some(format!("Digit{digit}"));
    }
    let renamed = match key {
        Key::Equals => "Equal",
        Key::Backtick => "Backquote",
        Key::OpenBracket => "BracketLeft",
        Key::CloseBracket => "BracketRight",
        Key::Plus
        | Key::Colon
        | Key::Pipe
        | Key::Questionmark
        | Key::Exclamationmark
        | Key::OpenCurlyBracket
        | Key::CloseCurlyBracket
        | Key::Copy
        | Key::Cut
        | Key::Paste => return None,
        _ => return Some(name),
    };
    Some(renamed.to_owned())
}

/// The keybinds, as the file says.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Keybinds {
    binds: BTreeMap<Chord, String>,
    /// The numpad always sends its bindings, `NumLock` or not: `numpad =
    /// "always"`. macOS has no `NumLock`, so this is how a Mac binds its
    /// numpad until the Clear key can switch it (this module's docs).
    pub(crate) numpad_always: bool,
}

/// The file as written.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    numpad: Option<String>,
    #[serde(default)]
    keys: BTreeMap<String, String>,
}

impl Keybinds {
    /// Read `text`, a keybinds file. Every binding that is wrong is said,
    /// and the rest still bind.
    pub(crate) fn read(text: &str) -> (Self, Vec<String>) {
        let file = match toml::from_str::<File>(text) {
            Ok(file) => file,
            Err(why) => return (Self::default(), vec![format!("{FILE}: {why}")]),
        };
        let mut problems = Vec::new();
        let mut keybinds = Self::default();
        match file.numpad.as_deref() {
            None | Some("numlock") => {}
            Some("always") => keybinds.numpad_always = true,
            Some(other) => problems.push(format!(
                "numpad = \"{other}\": it is \"numlock\" or \"always\"."
            )),
        }
        for (written, line) in file.keys {
            match Chord::parse(&written) {
                Ok(chord) if chord.types() => problems.push(format!(
                    "`{written}` types: bind it with Ctrl, Alt or Cmd, or it could not be typed."
                )),
                Ok(chord) => {
                    keybinds.binds.insert(chord, line);
                }
                Err(why) => problems.push(why),
            }
        }
        (keybinds, problems)
    }

    /// Read the file at `path`; none there is no bindings and no problem.
    pub(crate) fn load(path: &Path) -> (Self, Vec<String>) {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::read(&text),
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => (Self::default(), Vec::new()),
            Err(why) => (Self::default(), vec![format!("{}: {why}", path.display())]),
        }
    }

    /// How many keys are bound.
    pub(crate) fn len(&self) -> usize {
        self.binds.len()
    }

    /// What `chord` sends, if it is bound.
    pub(crate) fn line(&self, chord: &Chord) -> Option<&str> {
        self.binds.get(chord).map(String::as_str)
    }

    /// The fork's names for the numpad keys that are bound, whatever the
    /// modifiers: what it is told to catch, so an unbound numpad key still
    /// types (`eframe::Frame::set_numpad_capture_keys`).
    pub(crate) fn numpad_caught(&self) -> HashSet<String> {
        self.binds
            .keys()
            .filter_map(|chord| NUMPAD.iter().find(|(winit, _)| *winit == chord.key))
            .map(|(_, fork)| (*fork).to_owned())
            .collect()
    }

    /// Take from `input` every key press this binds, and the lines they
    /// send, in order: taken, so no widget sees a bound key.
    pub(crate) fn take(&self, input: &mut egui::InputState) -> Vec<String> {
        if self.binds.is_empty() {
            return Vec::new();
        }
        let mut lines = Vec::new();
        input.events.retain(|event| {
            let egui::Event::Key {
                key,
                physical_key,
                pressed: true,
                modifiers,
                ..
            } = event
            else {
                return true;
            };
            let Some(name) = winit_name(physical_key.unwrap_or(*key)) else {
                return true;
            };
            match self.line(&Chord::of(&name, *modifiers)) {
                Some(line) => {
                    lines.push(line.to_owned());
                    false
                }
                None => true,
            }
        });
        lines
    }
}

/// The keybinds file for `data`, the data folder.
pub(crate) fn path(data: &Path) -> PathBuf {
    data.join(FILE)
}

/// The line a numpad press sends, when it is bound: the fork's event, its
/// key named as winit names it.
pub(crate) fn numpad_line(keybinds: &Keybinds, event: &eframe::NumpadKeyEvent) -> Option<String> {
    if !event.pressed || !event.consumed {
        return None;
    }
    let winit::keyboard::PhysicalKey::Code(code) = event.physical_key else {
        return None;
    };
    let name = format!("{code:?}");
    keybinds
        .line(&Chord::of(&name, event.modifiers))
        .map(str::to_owned)
}

#[cfg(test)]
mod tests;
