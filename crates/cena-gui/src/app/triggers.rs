//! The trigger editor's window (`plan/54`): opened by *Triggers* beside
//! *Settings*, its changes handed to the binary, which owns the file.

use cena_ui::HubRequest;

use super::{App, TITLE};

/// The trigger editor's key among the windows' places.
const TRIGGERS: &str = "triggers";

impl App {
    /// Open the trigger editor; opening it asks the binary for the file.
    pub(super) fn open_triggers(&mut self) {
        if self.triggers.open() {
            self.sessions.ask(HubRequest::Triggers);
        }
    }

    /// The trigger editor, in a window of its own, while it is open.
    pub(super) fn trigger_window(
        &mut self,
        context: &egui::Context,
        glance: &crate::sessions::Glance,
    ) {
        if !self.triggers.open {
            return;
        }
        let builder = self
            .placements
            .builder(TRIGGERS, [1100.0, 680.0])
            .with_title(format!("Triggers — {TITLE}"));
        let editor = &mut self.triggers;
        let said = glance.said.as_ref().map(|(said, _)| said.as_str());
        let characters: Vec<String> = glance
            .roster
            .iter()
            .map(|card| card.character.clone())
            .collect();
        // What every pass asks, kept: see `App::play`.
        let (mut asked, mut closed, mut seen) = (Vec::new(), false, None);
        context.show_viewport_immediate(
            egui::ViewportId::from_hash_of(TRIGGERS),
            builder,
            |ui, _class| {
                closed |= ui.input(|input| input.viewport().close_requested());
                seen = Some(ui.input(|input| input.viewport().clone()));
                asked.extend(editor.show(ui, glance.triggers.as_ref(), said, &characters));
            },
        );
        if let Some(seen) = &seen {
            self.placements
                .note(TRIGGERS, seen, std::time::Instant::now());
        }
        if closed {
            self.triggers.open = false;
            self.placements.closed(TRIGGERS);
        }
        for change in asked {
            self.sessions.ask(HubRequest::Trigger(change));
        }
    }
}
