//! What the hubs are offered: the roster's characters, less those running
//! (`play.rs`'s table, moved down at its cap).

use super::Table;
use crate::{roster, secrets};

impl Table {
    /// Tell the hubs which characters they can add: in the roster, with a saved
    /// password, and not running. A name on two games is offered as
    /// `GAME:Name`.
    pub(super) async fn offer(&self) {
        if self.web.is_none() && self.gui.is_none() {
            return;
        }
        let running: Vec<(String, String)> = self
            .host
            .lock()
            .await
            .sessions()
            .filter(|(_, hosted)| hosted.is_running())
            .map(|(_, hosted)| (hosted.who.game.clone(), hosted.who.character.clone()))
            .collect();
        let roster = match roster::all(&self.dir) {
            Ok(roster) => roster,
            Err(why) => {
                let why = format!(
                    "[play] the roster could not be read, so no character is offered: {why}"
                );
                let mut said = self
                    .roster_problem
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if said.as_deref() != Some(why.as_str()) {
                    eprintln!("{why}");
                    *said = Some(why);
                }
                Vec::new()
            }
        };
        let available = roster::available(&roster, &running, secrets::saved);
        if let Some(web) = &self.web {
            web.sessions().offer(available.clone());
        }
        if let Some(gui) = &self.gui {
            gui.offer(available);
            gui.roster(
                roster
                    .iter()
                    .map(|entry| entry.card(secrets::saved(&entry.account)))
                    .collect(),
            );
        }
    }
}
