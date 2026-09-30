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
        let editor = &mut self.triggers;
        let said = glance.said.as_ref().map(|(said, _)| said.as_str());
        let characters: Vec<String> = glance
            .roster
            .iter()
            .map(|card| card.character.clone())
            .collect();
        let mut asked = Vec::new();
        let closed = self.placements.show(
            context,
            TRIGGERS,
            egui::ViewportId::from_hash_of(TRIGGERS),
            ([1100.0, 680.0], format!("Triggers — {TITLE}")),
            |ui| asked.extend(editor.show(ui, glance.triggers.as_ref(), said, &characters)),
        );
        if closed {
            self.triggers.open = false;
        }
        for change in asked {
            self.sessions.ask(HubRequest::Trigger(change));
        }
        if let Some((ask, inbox)) = self.triggers.take_check() {
            self.sessions.check_log(ask, inbox);
        }
    }
}
