//! Why a behavior stopped, shared by every behavior in this crate.
//!
//! It lived in `look.rs`, the M1 behavior, until `look` was retired to the
//! test support at M6 (author, 2026-09-24: *"get rid of any test behaviors,
//! like look"*). The type outlived the behavior it was written for because
//! `sync` and travel's driver return it too.

use cena_session::Outcome;

/// Why a behavior stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BehaviorError {
    /// The token was cancelled: the player, or whoever runs the behavior,
    /// said stop.
    ///
    /// **Only that.** `Outcome::Interrupted` used to land here too, but its
    /// one producer is a command from an earlier connection being discarded
    /// -- the connection changed, which is [`Self::Disconnected`]. A stop
    /// earns its one take-back (`travel::drive`); a changed connection must
    /// not.
    ///
    /// `plan/12` §4.4: cancellation does not un-send. A cancelled command may
    /// already have reached the game; this means "stop waiting and do not act
    /// on the result", never "it did not happen".
    Cancelled,
    /// The session is gone.
    ///
    /// **Permanent**, unlike [`Self::Disconnected`]: nothing is coming back.
    Dead,
    /// The connection was lost, and the session expects another.
    ///
    /// `plan/12` §5.1 puts a reconnecting session in a state where **"no
    /// automation runs"**, so the correct response is to stop *this run* --
    /// not to retry, and not to treat the session as finished. A behavior
    /// cannot act with no transport, and whatever restarts behaviors once
    /// `Ready` returns is above this layer.
    ///
    /// Separate from [`Self::Dead`] because a supervisor reads them
    /// differently: `Dead` ends the session, this is an interruption in it.
    Disconnected,
    /// Another behavior holds the command authority.
    ///
    /// `plan/12` §4.2: a behavior that wants a held authority "gets
    /// `Err(AuthorityHeld)`. It does not queue behind it -- silent queueing is
    /// how you get an attack that fires four seconds after the fight ended."
    /// So this is returned immediately, never after a wait.
    AuthorityHeld,
}

impl BehaviorError {
    /// What a round trip's [`Outcome`] means for the behavior that sent it,
    /// **when it means the session is gone or going**: `None` for an answer,
    /// a timeout or a refusal, which each behavior reads its own way.
    ///
    /// # Why one function
    ///
    /// `look` (now test support), `sync` and travel's driver each spelled this out, and the three
    /// copies agreed on everything but `Interrupted` -- which all three read
    /// as [`Self::Cancelled`] while travel's own `send` read the same event as
    /// [`Self::Disconnected`] (review, 2026-09-23). `Interrupted`'s one
    /// producer is the actor discarding a command stamped for an earlier
    /// connection (`cena-session`, `actor/io.rs`): the connection changed, and
    /// that is a disconnection. A stop earns its one take-back
    /// (`travel::drive`); a changed connection must not.
    ///
    /// Only a stop the behavior's own token saw is `Cancelled`, and no
    /// `Outcome` can say that: the token is raced beside the await, not
    /// inside it.
    #[must_use]
    pub fn from_outcome(outcome: &Outcome) -> Option<BehaviorError> {
        match outcome {
            // `Handled` is the desk's answer to a typed line, which a
            // behavior's round trip never goes through; an answer if it did.
            Outcome::Confirmed(_) | Outcome::Timeout | Outcome::Refused(_) | Outcome::Handled => {
                None
            }
            Outcome::Dead => Some(BehaviorError::Dead),
            // §5.1: no automation runs while a session has no transport.
            Outcome::Interrupted | Outcome::Disconnected => Some(BehaviorError::Disconnected),
        }
    }
}

#[cfg(test)]
mod tests {
    use cena_session::command::Refusal;

    use super::*;

    /// The one mapping every behavior shares. `Interrupted` is the case the
    /// review found wrong three times over (2026-09-23): it is the actor
    /// discarding an earlier connection's command, so it is a disconnection.
    ///
    /// A unit test, and not only the end-to-end ones beside `sync` and
    /// travel: `cena-session` now also answers a stale command `Disconnected`
    /// at admission (`actor/io.rs`), so a test that moves the connection
    /// reaches `Disconnected` first and cannot tell whether `Interrupted` is
    /// read right. MEASURED: all three end-to-end tests stayed green with the
    /// old `Interrupted -> Cancelled` arm restored.
    #[test]
    fn a_command_from_an_older_connection_is_a_disconnection_not_a_stop() {
        assert_eq!(
            BehaviorError::from_outcome(&Outcome::Interrupted),
            Some(BehaviorError::Disconnected)
        );
        assert_eq!(
            BehaviorError::from_outcome(&Outcome::Disconnected),
            Some(BehaviorError::Disconnected)
        );
        assert_eq!(
            BehaviorError::from_outcome(&Outcome::Dead),
            Some(BehaviorError::Dead)
        );
        for answered in [Outcome::Timeout, Outcome::Refused(Refusal::Transient)] {
            assert_eq!(BehaviorError::from_outcome(&answered), None, "{answered:?}");
        }
    }
}
