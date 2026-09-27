//! The window: what the binary opens with no `--headless` or `--web`
//! (`plan/47` step 2). The hub, over the sessions the binary attached.
//!
//! eframe owns the main thread for as long as the window is open, so the
//! binary builds its runtime by hand and runs the sessions there; this only
//! draws what their feeds left on the seats.

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
    /// test drives directly.
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        let cards = self.sessions.cards();
        self.hub.show(ui, &cards);
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
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
            .with_inner_size([560.0, 640.0]),
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
