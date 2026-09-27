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
use crate::play::{Asked, Play, PlayView};
use crate::sessions::{Seat, lock};
use crate::widget::Character;
use crate::{Hub, Sessions};

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
}

/// One character's play window.
#[derive(Debug)]
struct Window {
    play: Play,
    open: bool,
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
        }
    }

    /// The same, keeping what it keeps in `data`, the data folder: play
    /// windows' layouts by character name (`plan/47` step 6), and the
    /// keybinds read from it (step 7).
    #[must_use]
    pub fn keeping(sessions: Sessions, data: &std::path::Path) -> Self {
        let mut app = Self {
            layouts: Some(data.join("layouts")),
            presets: Library::load(Some(data.join("layouts"))),
            keys_file: Some(keys::path(data)),
            ..Self::new(sessions)
        };
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
            format!("No keys bound: write them in {}.", file.display())
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
        let view = HubView {
            cards: &glance.cards,
            offered: &glance.offered,
            merged: &glance.merged,
            said: glance.said.as_ref().map(|(said, _)| said.as_str()),
            windowed: &windowed,
        };
        match self.hub.show(ui, &view) {
            Some(HubAction::Ask(request)) => self.sessions.ask(request),
            Some(HubAction::Open(session)) => {
                if let Some(window) = self.plays.get_mut(&session) {
                    window.open = true;
                }
            }
            None => {}
        }
        for seat in &seats {
            self.play(ui.ctx(), seat, &seats);
        }
    }

    /// A window for each seat new since the last frame, open; none for a
    /// seat gone.
    fn seat(&mut self, seats: &[Arc<Seat>]) {
        self.plays
            .retain(|session, _| seats.iter().any(|seat| seat.id.0 == *session));
        for seat in seats {
            let layouts = self.layouts.clone();
            self.plays.entry(seat.id.0).or_insert_with(|| Window {
                play: Play::new(seat.id.0, &seat.name, layouts),
                open: true,
            });
        }
    }

    /// Show `seat`'s play window, if open, and act on what it asked. The
    /// other `seats` are the characters its widgets may follow.
    fn play(&mut self, context: &egui::Context, seat: &Arc<Seat>, seats: &[Arc<Seat>]) {
        let (keys, numpad, keys_said) = (&self.keys, &mut self.numpad, &self.keys_said);
        let numlock = self.numlock;
        let Some(window) = self.plays.get_mut(&seat.id.0) else {
            return;
        };
        if !window.open {
            return;
        }
        let snapshot = lock(&seat.snapshot).clone();
        let lifecycle = lock(&seat.card).lifecycle.clone();
        let hunt = lock(&seat.hunt).clone();
        let others: Vec<Character> = seats
            .iter()
            .filter(|other| other.id != seat.id)
            .map(|other| Character {
                name: other.name.clone(),
                snapshot: lock(&other.snapshot).clone(),
                hunt: lock(&other.hunt).clone(),
            })
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
            Some(Asked::Stop) => {
                let symbol = seat
                    .handle
                    .command_symbol()
                    .unwrap_or(cena_session::command::claimant::DEFAULT_SYMBOL);
                self.sessions.send(seat, format!("{symbol}stop"));
            }
            None => {}
        }
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
        })
    });
    effects.then_some(Duration::from_secs(1))
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        if std::mem::take(&mut self.catch_again) {
            frame.set_numpad_capture_mode(if self.keys.numpad_always {
                eframe::NumpadCaptureMode::Always
            } else {
                eframe::NumpadCaptureMode::NumLockAware
            });
            frame.set_numpad_capture_keys(Some(self.keys.numpad_caught()));
        }
        let pressed = frame.numpad_keys();
        if let Some(on) = pressed.iter().rev().find_map(|event| event.numlock_on) {
            self.numlock = Some(on);
        }
        self.numpad = pressed
            .iter()
            .filter_map(|event| keys::numpad_line(&self.keys, event))
            .collect();
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

#[cfg(test)]
mod tests {
    use super::*;
    use egui::accesskit::Role;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable as _;

    /// A handle with no session behind it: session 0, as every test
    /// handle is.
    fn handle() -> cena_session::SessionHandle {
        cena_session::SessionHandle::new(
            tokio::sync::mpsc::channel(1).0,
            cena_session::GenerationCell::default(),
            tokio::sync::broadcast::channel(1).0,
        )
    }

    /// A character that starts gets its play window; closed, it runs
    /// headless and its card offers the window again, which reopens it.
    #[test]
    fn a_character_gets_a_window_and_keeps_playing_without_it() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("a runtime");
        let sessions = Sessions::new(runtime.handle().clone());
        sessions.seat_for_test(handle(), "Ashryn");
        let mut harness = Harness::builder()
            .with_size((1200.0, 900.0))
            .build_ui_state(|ui, app: &mut App| app.draw(ui), App::new(sessions));
        harness.run();
        assert!(
            harness.query_by_role(Role::TextInput).is_some(),
            "its window"
        );
        assert!(harness.query_by_label("Open window").is_none());

        if let Some(window) = harness.state_mut().plays.get_mut(&0) {
            window.open = false;
        }
        harness.run();
        assert!(harness.query_by_role(Role::TextInput).is_none(), "headless");
        harness.get_by_label("Open window").click();
        harness.run();
        harness.run();
        assert!(harness.query_by_role(Role::TextInput).is_some(), "reopened");
    }

    /// A bound key sends its line on the character whose window has it, as
    /// if typed there; the command input never sees the key.
    #[test]
    fn a_bound_key_sends_on_its_window() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("a runtime");
        let sessions = Sessions::new(runtime.handle().clone());
        sessions.seat_for_test(handle(), "Ashryn");
        let mut app = App::new(sessions);
        app.keys = Keybinds::read("[keys]\nF5 = \"look\"\n").0;
        let mut harness = Harness::builder()
            .with_size((1200.0, 900.0))
            .build_ui_state(|ui, app: &mut App| app.draw(ui), app);
        harness.run();
        harness.key_press(egui::Key::F5);
        harness.run();
        harness.run();
        assert!(harness.query_by_label("> look").is_some());
        assert_eq!(
            harness.get_by_role(Role::TextInput).value().as_deref(),
            Some(""),
            "the input never saw it"
        );
    }

    /// What the binary says of a character's hunt shows in its window's Hunt
    /// pane, and goes when the hunt does.
    #[test]
    fn a_hunt_shows_in_its_window() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("a runtime");
        let sessions = Sessions::new(runtime.handle().clone());
        sessions.seat_for_test(handle(), "Ashryn");
        let told = sessions.clone();
        let mut harness = Harness::builder()
            .with_size((1200.0, 900.0))
            .build_ui_state(|ui, app: &mut App| app.draw(ui), App::new(sessions));
        harness.run();
        assert!(harness.query_by_label("No hunt running.").is_some());
        told.hunt(
            cena_session::SessionId::FIRST,
            Some(cena_ui::HuntView {
                running: "ojandhaart".to_owned(),
                phase: "resting (out of mana)".to_owned(),
                doing: "waiting 5s".to_owned(),
                target: None,
                waiting: Some("mana 30%, wants 50%".to_owned()),
            }),
        );
        harness.run();
        assert!(
            harness
                .query_by_label("Waiting: mana 30%, wants 50%")
                .is_some()
        );
        told.hunt(cena_session::SessionId::FIRST, None);
        harness.run();
        assert!(harness.query_by_label("No hunt running.").is_some());
    }

    /// Typed before the window has seen the session, a line is echoed and
    /// the player is told it did not go.
    #[test]
    fn a_line_before_any_snapshot_is_not_sent_and_says_so() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("a runtime");
        let sessions = Sessions::new(runtime.handle().clone());
        sessions.seat_for_test(handle(), "Ashryn");
        let mut harness = Harness::builder()
            .with_size((1200.0, 900.0))
            .build_ui_state(|ui, app: &mut App| app.draw(ui), App::new(sessions));
        harness.run();
        harness.get_by_role(Role::TextInput).type_text("look");
        harness.run();
        harness.key_press(egui::Key::Enter);
        harness.run();
        harness.run();
        assert!(harness.query_by_label("> look").is_some());
        assert!(
            harness
                .query_by_label("Not connected yet; nothing was sent.")
                .is_some()
        );
        // Stop is the character's own `;stop`, as if typed.
        harness.get_by_label("Stop").click();
        harness.run();
        harness.run();
        assert!(harness.query_by_label("> ;stop").is_some());
    }

    /// A custom window saved as a preset from a play window is kept in the
    /// library every character adds from, in its file beside the layouts.
    #[test]
    fn a_preset_saved_is_kept_for_every_character() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("a runtime");
        let data = std::env::temp_dir().join(format!("cena-app-presets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        let sessions = Sessions::new(runtime.handle().clone());
        sessions.seat_for_test(handle(), "Ashryn");
        let mut harness = Harness::builder()
            .with_size((1200.0, 900.0))
            .build_ui_state(
                |ui, app: &mut App| app.draw(ui),
                App::keeping(sessions, &data),
            );
        harness.run();
        harness.get_by_label("Left: ?").click_secondary();
        harness.run();
        harness.get_by_label("Save as preset...").click();
        harness.run();
        harness.key_press(egui::Key::Enter);
        harness.run();
        harness.run();
        let kept = Library::load(Some(data.join("layouts")));
        let names: Vec<&str> = kept
            .presets()
            .iter()
            .map(|preset| preset.name.as_str())
            .collect();
        assert_eq!(names, ["Loadout"]);
        harness.get_by_label("Layout").click();
        harness.run();
        harness.get_by_label("Add a widget...").click();
        harness.run();
        harness.get_by_label("Forget").click();
        harness.run();
        harness.run();
        assert!(
            Library::load(Some(data.join("layouts")))
                .presets()
                .is_empty(),
            "forgotten"
        );
        let _ = std::fs::remove_dir_all(&data);
    }

    /// A window is drawn again soon while something counts down by itself:
    /// a quarter of a second for the clocks, a second for an effect's time
    /// left, and not at all when nothing does.
    #[test]
    fn a_window_is_drawn_again_while_something_counts_down() {
        let story = std::sync::Mutex::new(crate::story::Story::default());
        let mut quiet = crate::fixture::snapshot();
        quiet.state.roundtime_ends = None;
        assert_eq!(clocks_run(Some(&quiet), &story), None);
        let mut buffed = quiet.clone();
        let now = buffed.state.game_time_now().expect("a clock");
        buffed.state.effects.insert(
            "1".to_owned(),
            cena_session::Effect {
                category: "Buffs".to_owned(),
                text: "Rapid Fire".to_owned(),
                ends_at: Some(now + 60),
                percent: 100,
            },
        );
        assert_eq!(
            clocks_run(Some(&buffed), &story),
            Some(Duration::from_secs(1))
        );
        let mut struck = buffed.clone();
        struck.state.roundtime_ends = Some(now + 3);
        assert_eq!(
            clocks_run(Some(&struck), &story),
            Some(Duration::from_millis(250))
        );
    }
}
