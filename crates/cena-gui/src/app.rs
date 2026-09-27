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

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cena_ui::LifecycleView;

use crate::hub::{HubAction, HubView};
use crate::play::{Asked, Play, PlayView};
use crate::sessions::{Seat, lock};
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
        }
    }

    /// The same, its play windows keeping their layouts in `layouts`, by
    /// character name (`plan/47` step 6).
    #[must_use]
    pub fn keeping_layouts(sessions: Sessions, layouts: PathBuf) -> Self {
        Self {
            layouts: Some(layouts),
            ..Self::new(sessions)
        }
    }

    /// Draw one frame into `ui` -- the hub, then each open play window --
    /// and act on what the player asked. What eframe calls each frame, and
    /// what a test drives directly.
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        let seats = self.sessions.seated();
        self.seat(&seats);
        let glance = self.sessions.glance();
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
            said: glance.said.as_deref(),
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
            self.play(ui.ctx(), seat);
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

    /// Show `seat`'s play window, if open, and act on what it asked.
    fn play(&mut self, context: &egui::Context, seat: &Arc<Seat>) {
        let Some(window) = self.plays.get_mut(&seat.id.0) else {
            return;
        };
        if !window.open {
            return;
        }
        let snapshot = lock(&seat.snapshot).clone();
        let lifecycle = lock(&seat.card).lifecycle.clone();
        let builder = egui::ViewportBuilder::default()
            .with_title(format!("{} — {TITLE}", seat.name))
            .with_inner_size([980.0, 680.0]);
        let (asked, closed) = context.show_viewport_immediate(
            egui::ViewportId::from_hash_of(("play", seat.id.0)),
            builder,
            |ui, _class| {
                let closed = ui.input(|input| input.viewport().close_requested());
                let story = lock(&seat.story);
                let view = PlayView {
                    name: &seat.name,
                    lifecycle: &lifecycle,
                    snapshot: snapshot.as_deref(),
                    story: &story,
                    now: Instant::now(),
                };
                let asked = window.play.show(ui, &view);
                drop(story);
                (asked, closed)
            },
        );
        if closed {
            window.open = false;
        }
        if clocks_run(snapshot.as_deref(), &seat.story) {
            context.request_repaint_after(Duration::from_millis(250));
        }
        match asked {
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

/// Whether something in a play window counts down by itself -- roundtime,
/// cast time, a banner -- so the window must be drawn again soon without
/// an event to prompt it.
fn clocks_run(
    snapshot: Option<&cena_session::Snapshot>,
    story: &std::sync::Mutex<crate::story::Story>,
) -> bool {
    let counting = snapshot.is_some_and(|snapshot| {
        let state = &snapshot.state;
        state.roundtime_remaining().is_some_and(|s| s > 0)
            || state.casttime_remaining().is_some_and(|s| s > 0)
    });
    counting || lock(story).alerts_at(Instant::now()).next().is_some()
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
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
            let layouts = cena_session::character_store::data_dir().join("layouts");
            Ok(Box::new(App::keeping_layouts(sessions, layouts)))
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
}
