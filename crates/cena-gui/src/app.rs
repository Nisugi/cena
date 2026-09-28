//! The window: what the binary opens with no `--headless` or `--web`
//! (`plan/47` step 2). The hub, over the sessions the binary attached, and a
//! play window for each character (step 4), each its own native window.
//!
//! eframe owns the main thread for as long as the window is open, so the
//! binary builds its runtime by hand and runs the sessions there; this only
//! draws what their feeds left on the seats, and hands what the player asks
//! for to the runtime.
//!
//! A character's play window opens when it starts. Closing it leaves the
//! character running headless (the author: *"when their running gui play
//! window is closed, if the connection isn't closed then they remain
//! headless"*); its hub card opens it again.
//!
//! Keybinds (step 7, `crate::keys`) send on the character whose play window
//! has the keyboard: a bound key is taken from that window's input, and the
//! numpad, which the fork hands over apart from any window, goes to the one
//! focused.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cena_ui::LifecycleView;

use crate::hub::{HubAction, HubView};
use crate::keys::{self, Keybinds};
use crate::layout::Library;
use crate::own::Own;
use crate::play::{Asked, Play, PlayView};
use crate::sessions::{Seat, lock};
use crate::widget::Character;
use crate::{Hub, Menu, Sessions};

/// The window's title: the product's name (`CLAUDE.md`: anything
/// user-facing is Hydra, not the working name).
pub const TITLE: &str = "Hydra";

/// The app eframe drives: the hub over [`Sessions`], and the play windows.
pub struct App {
    hub: Hub,
    sessions: Sessions,
    /// Each attached character's play window, open or not, by session.
    plays: BTreeMap<u32, Window>,
    /// Where play windows keep their layouts; `None`, and they keep none.
    layouts: Option<PathBuf>,
    /// The presets a player saved, which every play window adds from
    /// (`plan/49` Stage A step 7), kept beside the layouts.
    presets: Library,
    /// The keybinds, and the file they are read from, when there is one.
    keys: Keybinds,
    keys_file: Option<PathBuf>,
    /// What a play window says of the keybinds: how many, and what is wrong.
    keys_said: Vec<String>,
    /// The fork is to be told again which numpad keys to catch.
    catch_again: bool,
    /// Numpad lines this frame, for the play window with the keyboard.
    numpad: Vec<String>,
    /// `NumLock`, as the last numpad press showed it.
    numlock: Option<bool>,
    /// The settings menu, the one every way in opens (`plan/50` §7).
    menu: Menu,
    /// Hydra's own settings, kept in the window's own file (step 2).
    own: Own,
    /// The fork was last told to hand every numpad key to the Keys page.
    caught_for_menu: bool,
    /// A numpad key pressed this frame while the Keys page waits for one.
    numpad_for_menu: Option<String>,
}

/// One character's play window.
#[derive(Debug)]
struct Window {
    play: Play,
    open: bool,
    /// Its session was closed, when last drawn.
    ended: bool,
}

impl App {
    /// The hub over `sessions`, on its first tab, its play windows keeping
    /// no layout.
    #[must_use]
    pub fn new(sessions: Sessions) -> Self {
        Self {
            hub: Hub::default(),
            sessions,
            plays: BTreeMap::new(),
            layouts: None,
            presets: Library::default(),
            keys: Keybinds::default(),
            keys_file: None,
            keys_said: Vec::new(),
            catch_again: true,
            numpad: Vec::new(),
            numlock: None,
            menu: Menu::default(),
            own: Own::default(),
            caught_for_menu: false,
            numpad_for_menu: None,
        }
    }

    /// The same, keeping what it keeps in `data`, the data folder: play
    /// windows' layouts by character name (`plan/47` step 6), the keybinds
    /// read from it (step 7), and Hydra's own settings (`plan/50` §7 step 2).
    #[must_use]
    pub fn keeping(sessions: Sessions, data: &std::path::Path) -> Self {
        let own = Own::load(data);
        let mut app = Self {
            layouts: Some(data.join("layouts")),
            presets: Library::load(Some(data.join("layouts"))),
            keys_file: Some(keys::path(data)),
            ..Self::new(sessions)
        };
        app.hub.card_width = own.card_width();
        app.own = own;
        app.read_keys();
        app
    }

    /// Read the keybinds file again, and say what it bound.
    fn read_keys(&mut self) {
        let Some(file) = &self.keys_file else {
            return;
        };
        let (keys, problems) = Keybinds::load(file);
        self.keys_said = std::iter::once(if keys.len() == 0 {
            format!(
                "No keys bound: bind them here, or write them in {}.",
                file.display()
            )
        } else {
            format!("{} keys bound, from {}.", keys.len(), file.display())
        })
        .chain(problems)
        .collect();
        self.keys = keys;
        self.catch_again = true;
    }

    /// Draw one frame into `ui` -- the hub, then each open play window --
    /// and act on what the player asked. What eframe calls each frame, and
    /// what a test drives directly.
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        crate::carry::set_key(ui.ctx(), self.own.drag_with());
        let seats = self.sessions.seated();
        self.seat(&seats);
        let glance = self.sessions.glance();
        if let Some((_, left)) = &glance.said {
            // A frame when the answer is due to go, or it stays until the
            // next input.
            ui.ctx().request_repaint_after(*left);
        }
        let windowed: Vec<u32> = self
            .plays
            .iter()
            .filter(|(_, window)| window.open)
            .map(|(session, _)| *session)
            .collect();
        let lich: Vec<u32> = seats
            .iter()
            .filter(|seat| seat.handle.lich_running())
            .map(|seat| seat.id.0)
            .collect();
        let view = HubView {
            cards: &glance.cards,
            offered: &glance.offered,
            roster: &glance.roster,
            listing: glance.listing.as_ref(),
            merged: &glance.merged,
            said: glance.said.as_ref().map(|(said, _)| said.as_str()),
            windowed: &windowed,
            lich: &lich,
        };
        match self.hub.show(ui, &view) {
            Some(HubAction::Ask(request)) => self.sessions.ask(request),
            Some(HubAction::Open(session)) => {
                if let Some(window) = self.plays.get_mut(&session) {
                    window.open = true;
                }
            }
            Some(HubAction::Settings) => self.menu.open_for(None),
            Some(HubAction::Lich(session, on)) => {
                if let Some(seat) = seats.iter().find(|seat| seat.id.0 == session) {
                    self.hydras(seat, lich_word(on));
                }
            }
            None => {}
        }
        // A width a drag set is kept once the drag lets go.
        if ui.ctx().dragged_id().is_none()
            && let Err(why) = self.own.keep_width(self.hub.card_width)
        {
            self.menu.tell(why);
        }
        for seat in &seats {
            match self.play(ui.ctx(), seat, &seats) {
                Some(Asked::Settings(page)) => {
                    // The character's own settings, by its roster name.
                    let name = glance
                        .roster
                        .iter()
                        .find(|card| card.character.eq_ignore_ascii_case(&seat.name))
                        .map(crate::menu::roster_name);
                    self.menu.open_at(name, page.as_deref());
                }
                Some(Asked::Keys) => self.menu.open_at(None, Some("keys")),
                _ => {}
            }
        }
        self.settings(ui.ctx(), &glance);
    }

    /// A window for each seat new since the last frame, open; none for a
    /// seat gone.
    fn seat(&mut self, seats: &[Arc<Seat>]) {
        self.plays
            .retain(|session, _| seats.iter().any(|seat| seat.id.0 == *session));
        for seat in seats {
            let layouts = self.layouts.clone();
            self.plays.entry(seat.id.0).or_insert_with(|| Window {
                play: Play::new(
                    seat.id.0,
                    &seat.name,
                    cena_session::instance(&seat.game),
                    layouts,
                ),
                open: true,
                ended: false,
            });
        }
    }

    /// Show `seat`'s play window, if open, and act on what it asked. The
    /// other `seats` are the characters its widgets may follow. The settings
    /// menu, when it asked for it, which the app opens.
    fn play(
        &mut self,
        context: &egui::Context,
        seat: &Arc<Seat>,
        seats: &[Arc<Seat>],
    ) -> Option<Asked> {
        let (keys, numpad, keys_said) = (&self.keys, &mut self.numpad, &self.keys_said);
        let numlock = self.numlock;
        let close_with_session = self.own.close_with_session();
        let window = self.plays.get_mut(&seat.id.0)?;
        let lifecycle = lock(&seat.card).lifecycle.clone();
        // Closed with its session when the player asked for that (`plan/50`
        // §6 item 11), once: reopened from its card, it stays open.
        let ended = matches!(lifecycle, LifecycleView::Closed { .. });
        if ended && !window.ended && close_with_session {
            window.open = false;
        }
        window.ended = ended;
        if !window.open {
            return None;
        }
        let snapshot = lock(&seat.snapshot).clone();
        let hunt = lock(&seat.hunt).clone();
        let others: Vec<Character> = seats
            .iter()
            .filter(|other| other.id != seat.id)
            .map(|other| other.seen_from(seat))
            .collect();
        let builder = egui::ViewportBuilder::default()
            .with_title(format!("{} — {TITLE}", seat.name))
            .with_inner_size([980.0, 680.0]);
        let (asked, bound, closed) = context.show_viewport_immediate(
            egui::ViewportId::from_hash_of(("play", seat.id.0)),
            builder,
            |ui, _class| {
                let closed = ui.input(|input| input.viewport().close_requested());
                // Taken before anything draws, so no widget sees a bound key.
                let mut bound = ui.ctx().input_mut(|input| keys.take(input));
                if ui.input(|input| input.focused) {
                    bound.append(numpad);
                }
                let story = lock(&seat.story);
                let view = PlayView {
                    name: &seat.name,
                    lifecycle: &lifecycle,
                    snapshot: snapshot.as_deref(),
                    story: &story,
                    now: Instant::now(),
                    hunt: hunt.as_ref(),
                    numlock,
                    keys: keys_said,
                    others: &others,
                    presets: &self.presets,
                    lich: seat.handle.lich_running(),
                };
                let asked = window.play.show(ui, &view);
                drop(story);
                (asked, bound, closed)
            },
        );
        if closed {
            window.open = false;
        }
        if let Some(after) = clocks_run(snapshot.as_deref(), &seat.story) {
            context.request_repaint_after(after);
        }
        for line in bound {
            self.sessions.send(seat, line);
        }
        match asked {
            Some(Asked::ReloadKeys) => self.read_keys(),
            Some(Asked::SavePreset(preset)) => self.presets.keep(preset),
            Some(Asked::ForgetPreset(name)) => self.presets.forget(&name),
            Some(Asked::Send(line)) => self.sessions.send(seat, line),
            Some(Asked::Quietly(line)) => self.sessions.send_quietly(seat, line),
            Some(asked @ (Asked::Settings(_) | Asked::Keys)) => return Some(asked),
            Some(Asked::Stop) => self.hydras(seat, "stop"),
            Some(Asked::Lich(on)) => self.hydras(seat, lich_word(on)),
            None => {}
        }
        None
    }

    /// Send Hydra's command `word` on `seat`'s character, with its symbol.
    fn hydras(&self, seat: &Arc<Seat>, word: &str) {
        let symbol = seat
            .handle
            .command_symbol()
            .unwrap_or(cena_session::command::claimant::DEFAULT_SYMBOL);
        self.sessions.send(seat, format!("{symbol}{word}"));
    }

    /// The window was asked to close. With a character still playing, it
    /// asks first, as the hub's Shut down does, and stays open; otherwise it
    /// closes, and the binary quits whatever is left. Returns whether it
    /// closes.
    pub fn close_asked(&mut self) -> bool {
        let playing = self
            .sessions
            .cards()
            .iter()
            .any(|card| !matches!(card.lifecycle, LifecycleView::Closed { .. }));
        if playing && !self.sessions.closing() {
            self.hub.confirm_shutdown();
            return false;
        }
        true
    }
}

/// `;lich`'s word for switching the player's own Lich `on`, or off.
fn lich_word(on: bool) -> &'static str {
    if on { "lich on" } else { "lich off" }
}

/// When something in a play window counts down by itself -- roundtime,
/// cast time, a banner, an effect's time left -- how soon it must be drawn
/// again without an event to prompt it: the clocks a quarter of a second,
/// an effect, which counts whole seconds, a second.
fn clocks_run(
    snapshot: Option<&cena_session::Snapshot>,
    story: &std::sync::Mutex<crate::story::Story>,
) -> Option<Duration> {
    let state = snapshot.map(|snapshot| &snapshot.state);
    let clocks = state.is_some_and(|state| {
        state.roundtime_remaining().is_some_and(|s| s > 0)
            || state.casttime_remaining().is_some_and(|s| s > 0)
    });
    if clocks || lock(story).alerts_at(Instant::now()).next().is_some() {
        return Some(Duration::from_millis(250));
    }
    let effects = state.is_some_and(|state| {
        state.game_time_now().is_some_and(|now| {
            state
                .effects
                .iter()
                .any(|(id, _)| state.effects.remaining(id, now).is_some_and(|s| s > 0))
                || state
                    .world
                    .pulse
                    .as_ref()
                    .and_then(|pulse| pulse.due(now))
                    .is_some_and(|(_, most)| most > 0)
        })
    });
    effects.then_some(Duration::from_secs(1))
}

impl App {
    /// The fork's numpad presses this frame: `NumLock` as they show it, and
    /// the lines the bound ones send on the play window with the keyboard;
    /// or, while the Keys page waits for a key, the first press, for it.
    fn numpad_pressed(&mut self, pressed: &[eframe::NumpadKeyEvent]) {
        if let Some(on) = pressed.iter().rev().find_map(|event| event.numlock_on) {
            self.numlock = Some(on);
        }
        if self.menu.waiting_for_key() {
            self.numpad.clear();
            self.numpad_for_menu = pressed
                .iter()
                .filter(|event| !event.repeat)
                .find_map(keys::numpad_chord)
                .map(|chord| chord.written());
            return;
        }
        self.numpad_for_menu = None;
        self.numpad = pressed
            .iter()
            .filter_map(|event| keys::numpad_line(&self.keys, event))
            .collect();
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let waiting = self.menu.waiting_for_key();
        if waiting != self.caught_for_menu {
            self.caught_for_menu = waiting;
            self.catch_again = true;
        }
        if std::mem::take(&mut self.catch_again) {
            // While the Keys page waits for a key, every numpad key comes
            // here, so none is typed as its digit.
            frame.set_numpad_capture_mode(if self.keys.numpad_always || waiting {
                eframe::NumpadCaptureMode::Always
            } else {
                eframe::NumpadCaptureMode::NumLockAware
            });
            frame.set_numpad_capture_keys((!waiting).then(|| self.keys.numpad_caught()));
        }
        self.numpad_pressed(frame.numpad_keys());
        if ui.ctx().input(|input| input.viewport().close_requested()) && !self.close_asked() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        egui::CentralPanel::default().show(ui, |ui| self.draw(ui));
    }
}

/// Open the window over `sessions` and run it until it closes: by the
/// player, or by [`Sessions::close`] once the run is over. Blocks the
/// calling thread, which must be the main one.
///
/// # Errors
///
/// The window could not be opened: no display, or no graphics adapter.
pub fn run(sessions: Sessions) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(TITLE)
            .with_inner_size([560.0, 720.0]),
        ..Default::default()
    };
    eframe::run_native(
        TITLE,
        options,
        Box::new(move |creation| {
            sessions.opened(&creation.egui_ctx);
            let data = cena_session::character_store::data_dir();
            Ok(Box::new(App::keeping(sessions, &data)))
        }),
    )
}

mod settings;
#[cfg(test)]
mod tests;
