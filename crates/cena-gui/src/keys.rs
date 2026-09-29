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
//! The keys come three ways. The numpad's through the fork's numpad channel,
//! which catches only the numpad keys that are bound, so an unbound one
//! still types. The keys egui has no name for -- Pause, Scroll Lock, Print
//! Screen, Caps Lock, the context-menu key -- through the fork's key capture
//! (`plan/47` step 7, the fork pinned at `ed8b264`), which catches only those
//! bound, so an unbound Caps Lock is left to egui. Every other key through
//! egui, which names F1-F35, the arrows and the rest; a bound key's press is
//! taken before anything else sees it. A key that types -- a letter, a digit,
//! a mark, with no Ctrl, Alt or Cmd -- never fires a binding, or nobody could
//! type it.
//!
//! **`NumLock` is nobody's.** On Windows and Linux it is the operating
//! system's switch for the numpad; on macOS, which has none, winit calls the
//! Clear key `NumLock`, and Clear is Hydra's switch there, the numpad typing
//! or sending its keys (`app.rs`, `numpad_mode`).

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use egui::Modifiers;

pub use binding::{Action, Macro};
use file::Layer;
pub(crate) use file::{KeyFile, SETS, Whose};
pub(crate) use names::NUM_LOCK;
pub(crate) use names::winit_name;
use names::{CAPTURED, NUMPAD, TYPING, known};
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

/// The keybinds: Hydra's defaults, and every character's file on top. A
/// character's own file goes over them in [`Keybinds::of`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Keybinds {
    /// Hydra's own (`defaults.rs`), all in set 0.
    defaults: BTreeMap<Chord, Macro>,
    /// Every character's: the keybinds file.
    every: KeyFile,
}

impl Default for Keybinds {
    /// Hydra's defaults alone: no file.
    fn default() -> Self {
        Self {
            defaults: defaults::defaults(),
            every: KeyFile::default(),
        }
    }
}

impl Keybinds {
    /// Read `text`, every character's keybinds file, over Hydra's defaults.
    /// Every binding that is wrong is said, and the rest still bind.
    #[cfg(test)]
    pub(crate) fn read(text: &str) -> (Self, Vec<String>) {
        let (every, problems) = KeyFile::read(text, Whose::Every);
        (
            Self {
                every,
                ..Self::default()
            },
            problems,
        )
    }

    /// Read the file at `path`; none there is no bindings and no problem.
    pub(crate) fn load(path: &Path) -> (Self, Vec<String>) {
        let (every, problems) = KeyFile::load(path, Whose::Every);
        (
            Self {
                every,
                ..Self::default()
            },
            problems,
        )
    }

    /// The numpad sends its keys with `NumLock` on too: `numpad =
    /// "always"`. macOS has no `NumLock`, so this is how a Mac binds its
    /// numpad until the Clear key switches it (this module's docs).
    pub(crate) fn numpad_always(&self) -> bool {
        self.every.numpad_always
    }

    /// The keys in effect for the character whose own file is `mine`, or,
    /// with none, for no character.
    pub(crate) fn of<'a>(&'a self, mine: Option<&'a KeyFile>) -> Keys<'a> {
        Keys { binds: self, mine }
    }

    /// How many keys are bound for no character, defaults and all.
    pub(crate) fn len(&self) -> usize {
        self.of(None).bound().count()
    }

    /// How many every character's file binds or unbinds, in every set.
    pub(crate) fn changed(&self) -> usize {
        self.every.len()
    }

    /// What `chord` does for no character, if it is bound.
    #[cfg(test)]
    pub(crate) fn does(&self, chord: &Chord) -> Option<&Macro> {
        self.of(None).does(chord)
    }

    /// What `chord` does in set 0 beneath a file's own line: under every
    /// character's, Hydra's default; under a character's, every
    /// character's, then Hydra's.
    pub(crate) fn beneath(&self, whose: Whose, chord: &Chord) -> Option<&Macro> {
        if whose == Whose::Character
            && let Some(made) = self.every.sets[0].get(chord)
        {
            return made.as_ref();
        }
        self.defaults.get(chord)
    }

    /// Every key set `set` binds or unbinds -- in `mine`, a character's own
    /// file, when the page is a character's, and in every character's --
    /// with what it does, whose file that came from, and Hydra's default:
    /// what the Keys page lists. Set 0's hold Hydra's defaults too.
    pub(crate) fn rows(&self, set: u8, mine: Option<&KeyFile>) -> Vec<KeyRow> {
        let set = usize::from(set.min(SETS - 1));
        let layers: Vec<(Whose, &Layer)> = mine
            .map(|mine| (Whose::Character, &mine.sets[set]))
            .into_iter()
            .chain(std::iter::once((Whose::Every, &self.every.sets[set])))
            .collect();
        let empty = BTreeMap::new();
        let defaults = if set == 0 { &self.defaults } else { &empty };
        let chords: BTreeSet<&Chord> = layers
            .iter()
            .flat_map(|(_, layer)| layer.keys())
            .chain(defaults.keys())
            .collect();
        chords
            .into_iter()
            .map(|chord| {
                // What the first layer after `from` binds it to, or Hydra.
                let after = |from: usize| {
                    layers[from..]
                        .iter()
                        .find_map(|(_, layer)| layer.get(chord).cloned())
                        .unwrap_or_else(|| defaults.get(chord).cloned())
                };
                let (does, from, beneath) = match layers
                    .iter()
                    .position(|(_, layer)| layer.contains_key(chord))
                {
                    Some(at) => (after(at), Some(layers[at].0), after(at + 1)),
                    None => (defaults.get(chord).cloned(), None, None),
                };
                KeyRow {
                    key: chord.written(),
                    does,
                    default: defaults.get(chord).cloned(),
                    from,
                    beneath,
                }
            })
            .collect()
    }
}

/// The keys in effect for one character, or for none: a press looks in the
/// chosen set, the character's own file and then every character's; then
/// in set 0 the same way; then at Hydra's defaults. The first that binds or
/// unbinds the key has it (`plan/52` §2).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Keys<'a> {
    binds: &'a Keybinds,
    mine: Option<&'a KeyFile>,
}

impl<'a> Keys<'a> {
    /// The files' sets a press looks in, in order, before Hydra's defaults.
    fn layers(self) -> impl Iterator<Item = &'a Layer> {
        let chosen = self.mine.map_or(0, |mine| mine.chosen.min(SETS - 1));
        let sets = if chosen == 0 {
            vec![0]
        } else {
            vec![chosen, 0]
        };
        sets.into_iter().flat_map(move |set| {
            let set = usize::from(set);
            self.mine
                .map(|mine| &mine.sets[set])
                .into_iter()
                .chain(std::iter::once(&self.binds.every.sets[set]))
        })
    }

    /// What `chord` does, if it is bound.
    pub(crate) fn does(self, chord: &Chord) -> Option<&'a Macro> {
        for layer in self.layers() {
            if let Some(made) = layer.get(chord) {
                return made.as_ref();
            }
        }
        self.binds.defaults.get(chord)
    }

    /// Each key bound, and what it does.
    fn bound(self) -> impl Iterator<Item = (&'a Chord, &'a Macro)> {
        let chords: BTreeSet<&'a Chord> = self
            .layers()
            .flat_map(BTreeMap::keys)
            .chain(self.binds.defaults.keys())
            .collect();
        chords
            .into_iter()
            .filter_map(move |chord| self.does(chord).map(|made| (chord, made)))
    }

    /// The fork's names for the numpad keys that are bound, whatever the
    /// modifiers: what it is told to catch, so an unbound numpad key still
    /// types (`eframe::Frame::set_numpad_capture_keys`).
    pub(crate) fn numpad_caught(self) -> HashSet<String> {
        self.bound()
            .filter_map(|(chord, _)| NUMPAD.iter().find(|(winit, _)| *winit == chord.key))
            .map(|(_, fork)| (*fork).to_owned())
            .collect()
    }

    /// The keys egui has no name for that are bound, whatever the
    /// modifiers: what the fork is told to catch (`eframe::Frame::
    /// set_key_capture`), so one unbound is left to egui.
    pub(crate) fn key_capture(self) -> HashSet<winit::keyboard::KeyCode> {
        self.bound()
            .filter_map(|(chord, _)| CAPTURED.iter().find(|(name, _)| *name == chord.key))
            .map(|(_, code)| *code)
            .collect()
    }

    /// Take from `input` every key press this binds, and the macros they
    /// do, in order: taken, so no widget sees a bound key.
    pub(crate) fn take(self, input: &mut egui::InputState) -> Vec<Macro> {
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

/// A character's own keys file, in `data` beside its settings, by the code
/// of its game and its name: `None` for a game Hydra does not know.
pub(crate) fn character_path(data: &Path, game: &str, name: &str) -> Option<PathBuf> {
    cena_session::store::character_path(data, cena_session::instance(game)?, name, ".keys.toml")
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

/// The chord of a numpad press the fork caught, to be done as the window
/// with the keyboard binds it; `None` for one let through to be typed.
pub(crate) fn numpad_caught(event: &eframe::NumpadKeyEvent) -> Option<Chord> {
    if !event.consumed {
        return None;
    }
    numpad_chord(event)
}

/// Every key egui has no name for that a binding may use: what the fork
/// catches while the Keys page waits for a key, so it can take one.
pub(crate) fn capturable() -> HashSet<winit::keyboard::KeyCode> {
    CAPTURED.iter().map(|(_, code)| *code).collect()
}

/// The chord a press the fork's key capture caught is; `None` for a
/// release, or a key with no code.
pub(crate) fn captured_chord(event: &eframe::CapturedKeyEvent) -> Option<Chord> {
    if !event.pressed {
        return None;
    }
    let winit::keyboard::PhysicalKey::Code(code) = event.physical_key else {
        return None;
    };
    Some(Chord::of(&format!("{code:?}"), event.modifiers))
}

pub(crate) mod binding;
mod defaults;
pub(crate) mod file;
mod names;
pub(crate) mod page;
#[cfg(test)]
mod tests;
pub(crate) mod write;
