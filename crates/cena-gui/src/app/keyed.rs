//! A pressed key's macro, done (`plan/52` step 1): commands sent on the
//! character as if typed, the command input filled, or one of Hydra's
//! actions asked of the window as its own button would ask it. A send
//! macro's commands after a wait are kept here and sent when it is over,
//! each frame sending those due and asking for a frame at the next.
//!
//! **The keys the fork catches** (`plan/47` step 7): the numpad's, through
//! its numpad channel, and those egui has no name for -- Pause, Scroll Lock
//! and the rest -- through its key capture, pinned at `ed8b264`. Each is told
//! only the bound keys, so an unbound one still types or reaches egui; while
//! the Keys page waits for a key it is told every one, and the first press
//! goes to the page. On macOS the Clear key, which winit calls `NumLock`, is
//! always caught: it switches the numpad between typing and sending its
//! keys, as `NumLock` does elsewhere. Each is told the keys of the play
//! window that last had the keyboard, its character's own among them
//! (`plan/52` step 2).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::NumpadCaptureMode;

use super::App;
use crate::keys::binding::Step;
use crate::keys::{self, Action, KeyFile, Macro, Whose};
use crate::play::Asked;
use crate::sessions::Seat;

/// How the fork catches the numpad: everything while the Keys page waits
/// for a key, so none is typed as its digit; on macOS, which has no
/// `NumLock`, all of it or none as the Clear key last switched it; elsewhere
/// `NumLock` decides, unless the keybinds file says `numpad = "always"`.
///
/// `clear` is how Clear last switched the numpad, on a Mac; `None`
/// elsewhere.
pub(super) fn numpad_mode(waiting: bool, clear: Option<bool>, always: bool) -> NumpadCaptureMode {
    match (waiting, clear) {
        (true, _) | (false, Some(true)) => NumpadCaptureMode::Always,
        (false, Some(false)) => NumpadCaptureMode::Off,
        (false, None) if always => NumpadCaptureMode::Always,
        (false, None) => NumpadCaptureMode::NumLockAware,
    }
}

/// A command a send macro sends once its wait is over.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Later {
    /// When.
    at: Instant,
    /// On which character's session.
    session: u32,
    /// The command.
    line: String,
}

/// What a play window asks for when a key performs `action`: what its own
/// button asks.
pub(super) fn asked(action: Action) -> Asked {
    match action {
        Action::Stop => Asked::Stop,
        Action::Settings => Asked::Settings(None),
        Action::Set(set) => Asked::UseSet(set),
    }
}

impl App {
    /// The keys file of the character `name` of the game `game`, in the
    /// data folder; `None` where nothing is kept, or the game is unknown.
    pub(super) fn mine_path(&self, game: &str, name: &str) -> Option<PathBuf> {
        let data = self.keys_file.as_deref()?.parent()?;
        keys::character_path(data, game, name)
    }

    /// Read the character's keys file at `file` when it has not been read.
    pub(super) fn load_mine(&mut self, file: Option<&Path>) {
        if let Some(file) = file
            && !self.mine.contains_key(file)
        {
            let read = KeyFile::load(file, Whose::Character);
            self.mine.insert(file.to_owned(), read);
        }
    }

    /// What a play window says of its keys: every character's, then how
    /// many of `name`'s own there are and the set it uses, and what is
    /// wrong in its file.
    pub(super) fn keys_said(
        &self,
        mine: Option<&(KeyFile, Vec<String>)>,
        name: &str,
    ) -> Vec<String> {
        let mut said = self.keys_said.clone();
        if let Some((mine, problems)) = mine {
            if mine.len() > 0 {
                said.push(format!("{} of {name}'s own.", mine.len()));
            }
            if mine.chosen != 0 {
                said.push(format!("Macro set {} in use, over set 0.", mine.chosen));
            }
            said.extend(problems.iter().cloned());
        }
        said
    }

    /// The play window of `session`, whose character's keys file is
    /// `file`, has the keyboard: the fork catches its keys, and is told so
    /// when it did not have it before.
    pub(super) fn took_keyboard(&mut self, session: u32, file: Option<PathBuf>) {
        if self.focused.as_ref().map(|(had, _)| *had) != Some(session) {
            self.focused = Some((session, file));
            self.catch_again = true;
        }
    }

    /// The send macros keys did on `seat`'s window this frame, sent from
    /// now; a command kept for after a wait has the next frame ask for the
    /// one it is due in (`send_due`).
    pub(super) fn send_macros(
        &mut self,
        context: &egui::Context,
        seat: &Arc<Seat>,
        sends: Vec<Macro>,
    ) {
        let now = Instant::now();
        for made in sends {
            self.send_macro(seat, &made, now);
        }
        if !self.later.is_empty() {
            context.request_repaint();
        }
    }

    /// `made`, a send macro, on `seat` from `now`: its commands before any
    /// wait sent at once, the rest kept until their waits are over.
    pub(super) fn send_macro(&mut self, seat: &Arc<Seat>, made: &Macro, now: Instant) {
        let Ok(steps) = made.steps() else {
            return;
        };
        let mut at = now;
        for step in steps {
            match step {
                Step::Wait(wait) => at += wait,
                Step::Line(line) if at <= now => self.sessions.send(seat, line),
                Step::Line(line) => self.later.push(Later {
                    at,
                    session: seat.id.0,
                    line,
                }),
            }
        }
    }

    /// The kept commands whose wait is over by `now`, sent in the order
    /// they were kept, to their characters among `seats`; one whose
    /// character has gone is dropped. How long until the next is due.
    pub(super) fn send_due(&mut self, seats: &[Arc<Seat>], now: Instant) -> Option<Duration> {
        let (due, kept): (Vec<Later>, Vec<Later>) =
            self.later.drain(..).partition(|later| later.at <= now);
        self.later = kept;
        for later in due {
            if let Some(seat) = seats.iter().find(|seat| seat.id.0 == later.session) {
                self.sessions.send(seat, later.line);
            }
        }
        self.later
            .iter()
            .map(|later| later.at.saturating_duration_since(now))
            .min()
    }

    /// Tell the fork which keys to catch, when that has changed: the Keys
    /// page began or stopped waiting for a key, the keybinds were read
    /// again, or Clear switched the numpad.
    pub(super) fn catch(&mut self, frame: &mut eframe::Frame) {
        let waiting = self.menu.waiting_for_key();
        if waiting != self.menu_waits {
            self.menu_waits = waiting;
            self.catch_again = true;
        }
        if !std::mem::take(&mut self.catch_again) {
            return;
        }
        let mac = cfg!(target_os = "macos");
        frame.set_numpad_capture_mode(numpad_mode(
            waiting,
            mac.then_some(self.clear_sends),
            self.keys.numpad_always(),
        ));
        let file = self.focused.as_ref().and_then(|(_, file)| file.clone());
        self.load_mine(file.as_deref());
        let mine = file
            .as_ref()
            .and_then(|file| self.mine.get(file))
            .map(|(mine, _)| mine);
        let keys = self.keys.of(mine);
        frame.set_numpad_capture_keys((!waiting).then(|| keys.numpad_caught()));
        let mut captured = if waiting {
            keys::capturable()
        } else {
            keys.key_capture()
        };
        if mac {
            captured.insert(keys::NUM_LOCK);
        }
        frame.set_key_capture(captured);
    }

    /// The fork's presses this frame, of the numpad (`numpad`) and of the
    /// keys egui has no name for (`captured`): `NumLock` as the numpad's
    /// show it; on a Mac (`mac`), Clear switching the numpad; the bound
    /// ones, for the play window with the keyboard to do; or, while the
    /// Keys page waits for a key, the first press, for it.
    pub(super) fn caught_pressed(
        &mut self,
        numpad: &[eframe::NumpadKeyEvent],
        captured: &[eframe::CapturedKeyEvent],
        mac: bool,
    ) {
        if let Some(on) = numpad.iter().rev().find_map(|event| event.numlock_on) {
            self.numlock = Some(on);
        }
        let clear = winit::keyboard::PhysicalKey::Code(keys::NUM_LOCK);
        let (switches, captured): (Vec<_>, Vec<_>) = captured
            .iter()
            .partition(|event| event.physical_key == clear);
        if mac && switches.iter().any(|event| event.pressed && !event.repeat) {
            self.clear_sends = !self.clear_sends;
            self.catch_again = true;
        }
        if self.menu.waiting_for_key() {
            self.caught.clear();
            let numpad = numpad
                .iter()
                .filter(|event| !event.repeat)
                .find_map(keys::numpad_chord);
            let other = captured
                .iter()
                .filter(|event| !event.repeat)
                .find_map(|event| keys::captured_chord(event));
            self.caught_for_page = numpad.or(other).map(|chord| chord.written());
            return;
        }
        self.caught_for_page = None;
        let mine = self
            .focused
            .as_ref()
            .and_then(|(_, file)| self.mine.get(file.as_ref()?))
            .map(|(mine, _)| mine);
        let keys = self.keys.of(mine);
        self.caught = numpad
            .iter()
            .filter_map(keys::numpad_caught)
            .chain(
                captured
                    .iter()
                    .filter_map(|event| keys::captured_chord(event)),
            )
            .filter(|chord| keys.does(chord).is_some())
            .collect();
    }
}
