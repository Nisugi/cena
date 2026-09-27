//! The window: what the binary opens with no `--headless` or `--web`
//! (`plan/47` step 2). The hub, over the sessions the binary attached.
//!
//! eframe owns the main thread for as long as the window is open, so the
//! binary builds its runtime by hand and runs the sessions there; this only
//! draws what their feeds left on the seats, and hands what the player asks
//! for to the runtime.

use cena_ui::LifecycleView;

use crate::hub::HubView;
use crate::{Hub, Sessions};

/// The window's title: the product's name (`CLAUDE.md`: anything
/// user-facing is Hydra, not the working name).
pub const TITLE: &str = "Hydra";

/// The app eframe drives: the hub over [`Sessions`].
pub struct App {
    hub: Hub,
    sessions: Sessions,
}

impl App {
    /// The hub over `sessions`, on its first tab.
    #[must_use]
    pub fn new(sessions: Sessions) -> Self {
        Self {
            hub: Hub::default(),
            sessions,
        }
    }

    /// Draw one frame into `ui`: what eframe calls each frame, and what a
    /// test drives directly. A request the player made goes to the binary.
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        let glance = self.sessions.glance();
        let view = HubView {
            cards: &glance.cards,
            offered: &glance.offered,
            merged: &glance.merged,
            said: glance.said.as_deref(),
        };
        if let Some(request) = self.hub.show(ui, &view) {
            self.sessions.ask(request);
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
            Ok(Box::new(App::new(sessions)))
        }),
    )
}
