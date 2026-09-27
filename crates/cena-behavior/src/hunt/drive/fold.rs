//! What the hunt's own state takes from the session's events. Moved down out
//! of `drive.rs` when M8's flags gave it an arm, rather than grow a file
//! already past its cap.

use cena_session::{Event, GameState, State};

use crate::error::BehaviorError;

/// Fold one event into a state: a frame is applied; a reconnect invalidates
/// what a reconnect invalidates and is waited out; a close ends the behavior;
/// a flag a trigger set is set here too, so a step's `flag` guard reads it
/// (`plan/45` Stage 2).
pub(super) fn fold_into(state: &mut GameState, event: &Event) -> Result<(), BehaviorError> {
    match event {
        Event::Frame(frame) => {
            state.apply(frame);
            Ok(())
        }
        // A drop is waited out, holding the authority (SE-4 (c)); the
        // driver marks it. Invalidating twice is harmless.
        Event::StateChanged(State::Reconnecting) => {
            state.invalidate_for_reconnect();
            Ok(())
        }
        Event::StateChanged(State::Closed) => Err(BehaviorError::Dead),
        Event::Flag(change) => {
            state.flags.apply(change);
            Ok(())
        }
        _ => Ok(()),
    }
}
