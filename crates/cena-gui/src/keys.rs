//! Keybinds (`plan/47` step 7, `plan/52`): a key, and the macro it does on
//! the character whose play window has the keyboard -- commands sent as if
//! typed there, text put in the command input, or one of Hydra's actions
//! (`binding.rs`).
//!
//! **Hydra's defaults live in the code** (`defaults.rs`); the keybinds file
//! holds only what the player added or changed, a key written `""` unbinding
//! a default (`plan/52` §2).
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

use egui::Modifiers;

pub use binding::{Action, Macro};
pub(crate) use names::winit_name;
use names::{NUMPAD, TYPING, known};
pub use page::KeyRow;

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

    /// The chord as the file and the Keys page write it: its modifiers in
    /// one order, then its key, `Ctrl+Shift+F1`. [`Chord::parse`] reads it
    /// back.
    pub(crate) fn written(&self) -> String {
        let mut written = String::new();
        for (bit, name) in [(CTRL, "Ctrl"), (SHIFT, "Shift"), (ALT, "Alt"), (CMD, "Cmd")] {
            if self.held & bit != 0 {
                written.push_str(name);
                written.push('+');
            }
        }
        written.push_str(&self.key);
        written
    }

    /// Whether this chord types something rather than commanding: a letter,
    /// a digit, a mark or the space, with no Ctrl, Alt or Cmd held.
    fn types(&self) -> bool {
        self.held & (CTRL | ALT | CMD) == 0 && TYPING.contains(&self.key.as_str())
    }
}

/// The keybinds in effect: Hydra's defaults, with the file's changes on
/// top.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Keybinds {
    /// Hydra's own (`defaults.rs`).
    defaults: BTreeMap<Chord, Macro>,
    /// The file's: a macro, or `None` where it unbinds a default.
    file: BTreeMap<Chord, Option<Macro>>,
    /// The numpad always sends its bindings, `NumLock` or not: `numpad =
    /// "always"`. macOS has no `NumLock`, so this is how a Mac binds its
    /// numpad until the Clear key can switch it (this module's docs).
    pub(crate) numpad_always: bool,
}

impl Default for Keybinds {
    /// Hydra's defaults alone: no file.
    fn default() -> Self {
        Self {
            defaults: defaults::defaults(),
            file: BTreeMap::new(),
            numpad_always: false,
        }
    }
}

/// The file as written.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    numpad: Option<String>,
    #[serde(default)]
    keys: BTreeMap<String, toml::Value>,
}

impl Keybinds {
    /// Read `text`, a keybinds file, over Hydra's defaults. Every binding
    /// that is wrong is said, and the rest still bind.
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
        for (written, value) in file.keys {
            match Chord::parse(&written) {
                Ok(chord) if chord.types() => problems.push(format!(
                    "`{written}` types: bind it with Ctrl, Alt or Cmd, or it could not be typed."
                )),
                // Each command one line, or it is said, not bound: a line with
                // a newline would send two (the crate review of 2026-09-28,
                // R10); a send macro's commands are cut apart first.
                Ok(chord) => match Macro::read(&value) {
                    Ok(made) => {
                        keybinds.file.insert(chord, made);
                    }
                    Err(why) => problems.push(format!("`{written}`: {why}.")),
                },
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

    /// How many keys are bound, defaults and all.
    pub(crate) fn len(&self) -> usize {
        self.bound().count()
    }

    /// How many of those the file changed or added.
    pub(crate) fn changed(&self) -> usize {
        self.file.len()
    }

    /// Each key bound, and what it does.
    fn bound(&self) -> impl Iterator<Item = (&Chord, &Macro)> {
        let file = self
            .file
            .iter()
            .filter_map(|(chord, made)| made.as_ref().map(|made| (chord, made)));
        let defaults = self
            .defaults
            .iter()
            .filter(|(chord, _)| !self.file.contains_key(*chord));
        file.chain(defaults)
    }

    /// Every key Hydra or the file binds or unbinds, in key order, with
    /// what it does now and what Hydra's default is: what the Keys page
    /// lists.
    pub(crate) fn rows(&self) -> Vec<KeyRow> {
        let chords: std::collections::BTreeSet<&Chord> =
            self.defaults.keys().chain(self.file.keys()).collect();
        chords
            .into_iter()
            .map(|chord| KeyRow {
                key: chord.written(),
                does: self.does(chord).cloned(),
                default: self.defaults.get(chord).cloned(),
            })
            .collect()
    }

    /// What `chord` does, if it is bound.
    pub(crate) fn does(&self, chord: &Chord) -> Option<&Macro> {
        match self.file.get(chord) {
            Some(made) => made.as_ref(),
            None => self.defaults.get(chord),
        }
    }

    /// Whether Hydra binds `chord` by default.
    pub(crate) fn has_default(&self, chord: &Chord) -> bool {
        self.defaults.contains_key(chord)
    }

    /// The fork's names for the numpad keys that are bound, whatever the
    /// modifiers: what it is told to catch, so an unbound numpad key still
    /// types (`eframe::Frame::set_numpad_capture_keys`).
    pub(crate) fn numpad_caught(&self) -> HashSet<String> {
        self.bound()
            .filter_map(|(chord, _)| NUMPAD.iter().find(|(winit, _)| *winit == chord.key))
            .map(|(_, fork)| (*fork).to_owned())
            .collect()
    }

    /// Take from `input` every key press this binds, and the macros they
    /// do, in order: taken, so no widget sees a bound key.
    pub(crate) fn take(&self, input: &mut egui::InputState) -> Vec<Macro> {
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
            match self.does(&Chord::of(&name, *modifiers)) {
                Some(made) => {
                    lines.push(made.clone());
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

/// The chord a numpad press is, its key named as winit names it; `None` for
/// a release, or a key with no code.
pub(crate) fn numpad_chord(event: &eframe::NumpadKeyEvent) -> Option<Chord> {
    if !event.pressed {
        return None;
    }
    let winit::keyboard::PhysicalKey::Code(code) = event.physical_key else {
        return None;
    };
    Some(Chord::of(&format!("{code:?}"), event.modifiers))
}

/// The macro a numpad press does, when it is bound and the fork caught it.
pub(crate) fn numpad_macro(keybinds: &Keybinds, event: &eframe::NumpadKeyEvent) -> Option<Macro> {
    if !event.consumed {
        return None;
    }
    keybinds.does(&numpad_chord(event)?).cloned()
}

pub(crate) mod binding;
mod defaults;
mod names;
pub(crate) mod page;
#[cfg(test)]
mod tests;
pub(crate) mod write;
