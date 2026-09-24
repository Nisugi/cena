//! The last check before an action's bytes go out (`plan/30` §3).
//!
//! A behavior settles roundtime and cast roundtime, then checks, then sends
//! -- every check after the wait, because *"conditions can change between
//! then and then"* (author, 2026-09-24). But it checks its own copy of the
//! state, which trails the actor's by whatever events are still queued. So
//! the actor checks once more, against the live model, at the moment it
//! writes. What no check can close is the window after the write, before
//! the game reads it; that is `plan/12` §4.4's to tolerate.
//!
//! Moved out of `io.rs`, which held `send_now`'s roundtime gate inline, when
//! the action gate would have taken that file past its cap (`plan/05` Rule
//! 4.1).

use cena_platform::ByteSource;

use super::SessionActor;
use crate::command::{Gate, Refusal};

impl<S: ByteSource> SessionActor<S> {
    /// Whether `gate` lets a command go out now: `Ok` with the server time
    /// when the clock is known, or the reason it may not.
    pub(super) fn check_gate(&self, gate: Gate) -> Result<Option<u32>, Refusal> {
        match gate {
            Gate::None => Ok(None),
            Gate::Roundtime => self.clear_of_roundtime().map(Some),
            Gate::Act { target } => {
                let at = self.clear_of_roundtime()?;
                // An unknown clock was refused above, so this is known.
                if self.state.in_casttime() == Some(true) {
                    return Err(Refusal::Casttime);
                }
                // Typed state, from `<indicator>`s (`plan/12` §5.2), not a
                // sentence scanned for -- Lich's `fput` keys on the prose
                // (`global_defs.rb:1578`).
                let status = self.state.status.known();
                if status.dead() == Some(true) {
                    return Err(Refusal::Permanent);
                }
                if status.stunned() == Some(true) {
                    return Err(Refusal::Stunned);
                }
                if status.webbed() == Some(true) {
                    return Err(Refusal::Webbed);
                }
                // Still here, and still worth an attack: `valid_target` is
                // Lich's `valid_target?` (`creature.rb:642-651`) -- dead by
                // the game's flag or by hit points, or never a real target.
                if let Some(id) = target
                    && !self
                        .state
                        .creatures()
                        .in_room()
                        .any(|creature| creature.id == id && creature.valid_target())
                {
                    return Err(Refusal::TargetGone);
                }
                Ok(Some(at))
            }
        }
    }

    /// Not in roundtime, with the server time; the reason otherwise.
    fn clear_of_roundtime(&self) -> Result<u32, Refusal> {
        match (self.state.in_roundtime(), self.state.game_time_now()) {
            (Some(true), _) => Err(Refusal::Roundtime),
            // Unknown is NOT permission (`plan/12` §5.2). `Transient` because
            // a prompt will arrive and then the answer is knowable -- it is
            // "ask again", not "never".
            (None, _) | (_, None) => Err(Refusal::Transient),
            (Some(false), Some(at)) => Ok(at),
        }
    }
}
