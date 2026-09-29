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

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::NumpadCaptureMode;
use winit::keyboard::KeyCode;

use super::App;
use crate::keys::binding::Step;
use crate::keys::{self, Action, KeyFile, Macro, Whose};
use crate::play::{Asked, Play};
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

/// The macros the keys pressed in a play window this frame do, and whether
/// it has the keyboard: those egui saw, taken from its input before anything
/// draws so no widget sees a bound key, then, while it has the keyboard,
/// those the fork `caught`. One on the command input while something else
/// has the keyboard is left to that (`plan/52` step 3).
pub(super) fn pressed(
    ui: &egui::Ui,
    keys: keys::Keys<'_>,
    play: &Play,
    caught: &mut Vec<keys::Chord>,
) -> (Vec<Macro>, bool) {
    let typing = play.typing(ui.ctx());
    let aside = |made: &Macro| matches!(made, Macro::Act(action) if action.on_input() && !typing);
    let mut pressed = ui.ctx().input_mut(|input| keys.take(input, aside));
    let focused = ui.input(|input| input.focused);
    if focused {
        pressed.extend(
            caught
                .drain(..)
                .filter_map(|chord| keys.does(&chord).cloned())
                .filter(|made| !aside(made)),
        );
    }
    (pressed, focused)
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
/// button asks. An action on its command input or its windows it does
/// itself (`Play::act`), and a line it sends is asked to be sent.
pub(super) fn asked(context: &egui::Context, window: &mut Play, action: Action) -> Option<Asked> {
    Some(match action {
        Action::Stop => Asked::Stop,
        Action::Settings => Asked::Settings(None),
        Action::Set(set) => Asked::UseSet(set),
        Action::Character(n) => Asked::Character(n),
        _ => return window.act(context, action).map(Asked::Send),
    })
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
        self.keyboard_now = Some(session);
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

    /// The play window of `seats`' `n`th character, from 1, opened and
    /// given the keyboard (`plan/52` step 8); nothing past the last.
    pub(super) fn bring(&mut self, context: &egui::Context, seats: &[Arc<Seat>], n: u8) {
        let Some(seat) = usize::from(n).checked_sub(1).and_then(|at| seats.get(at)) else {
            return;
        };
        if let Some(window) = self.plays.get_mut(&seat.id.0) {
            window.open = true;
        }
        let viewport = egui::ViewportId::from_hash_of(("play", seat.id.0));
        context.send_viewport_cmd_to(viewport, egui::ViewportCommand::Focus);
    }

    /// No play window had the keyboard this frame: the hub, the settings or
    /// another program has it, and the fork is told to catch none of the
    /// play windows' keys, so a bound `NumpadEnter` still sends the hub's
    /// login form.
    pub(super) fn left_keyboard(&mut self) {
        if self.keyboard_now.is_none() && self.focused.take().is_some() {
            self.catch_again = true;
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
    /// again, Clear switched the numpad, or the keyboard went to another
    /// play window or to none.
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
        let (numpad, captured) = self.keys_to_catch(waiting, mac);
        frame.set_numpad_capture_keys(numpad);
        frame.set_key_capture(captured);
    }

    /// What the fork is to catch: the numpad keys by the fork's names, all
    /// of them for `None`, and the keys egui has no name for. While the
    /// Keys page is `waiting` for a key, every one; while a play window has
    /// the keyboard, those its keys bind; while none has, none of them. On
    /// a Mac (`mac`), always Clear, the numpad's switch.
    pub(super) fn keys_to_catch(
        &mut self,
        waiting: bool,
        mac: bool,
    ) -> (Option<HashSet<String>>, HashSet<KeyCode>) {
        let file = self.focused.as_ref().and_then(|(_, file)| file.clone());
        self.load_mine(file.as_deref());
        let mine = file
            .as_ref()
            .and_then(|file| self.mine.get(file))
            .map(|(mine, _)| mine);
        let keys = self.keys.of(mine);
        let (numpad, mut captured) = match (waiting, self.focused.is_some()) {
            (true, _) => (None, keys::capturable()),
            (false, true) => (Some(keys.numpad_caught()), keys.key_capture()),
            (false, false) => (Some(HashSet::new()), HashSet::new()),
        };
        if mac {
            captured.insert(keys::NUM_LOCK);
        }
        (numpad, captured)
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
