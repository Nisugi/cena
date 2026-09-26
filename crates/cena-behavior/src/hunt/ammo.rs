//! An arrow the game will not fire (`cmd_ranged`, `bigshot.lic:6385-6400`).
//!
//! When `fire` is answered *"You cannot fire"*, bigshot stows what is in the
//! right hand; and when the stow answers that its container is closed, it
//! opens the profile's `ammo_container` and puts the item there. The lines go
//! one a tick, ahead of the routine, from this module's own queue, so a
//! target dying between them does not drop the arrow in hand.

use std::collections::VecDeque;

use cena_session::GameState;

use super::engine::Hunt;

/// Where the recovery stands.
#[derive(Debug, Default)]
pub(super) struct Ammo {
    /// The game refused a `fire`: stow the right hand at the next tick.
    refused: bool,
    /// The item being stowed, until its reply is read.
    stowing: Option<String>,
    /// Lines still to send, in order.
    pending: VecDeque<String>,
}

impl Hunt {
    /// Read the game's answer to the last line for the recovery.
    pub(super) fn ammo_replied(&mut self, lines: &[&str]) {
        if lines
            .iter()
            .any(|line| line.trim_start().starts_with("You cannot fire"))
        {
            self.ammo.refused = true;
        }
        if let Some(item) = self.ammo.stowing.take()
            && lines.iter().any(|line| line.contains("closed"))
            && let Some(container) = self.profile.aim.ammo_container.clone()
        {
            self.ammo.pending.push_back(format!("open my {container}"));
            self.ammo
                .pending
                .push_back(format!("put #{item} in my {container}"));
        }
    }

    /// The next recovery line, ahead of the routine.
    pub(super) fn ammo_line(&mut self, state: &GameState) -> Option<String> {
        if let Some(line) = self.ammo.pending.pop_front() {
            return Some(line);
        }
        if !std::mem::take(&mut self.ammo.refused) {
            return None;
        }
        let item = state.right_hand.id()?.to_owned();
        let line = format!("stow #{item}");
        self.ammo.stowing = Some(item);
        Some(line)
    }
}
