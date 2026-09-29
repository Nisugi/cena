//! A pressed key's macro, done (`plan/52` step 1): commands sent on the
//! character as if typed, the command input filled, or one of Hydra's
//! actions asked of the window as its own button would ask it. A send
//! macro's commands after a wait are kept here and sent when it is over,
//! each frame sending those due and asking for a frame at the next.

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::App;
use crate::keys::binding::Step;
use crate::keys::{Action, Macro};
use crate::play::Asked;
use crate::sessions::Seat;

/// A command a send macro sends once its wait is over.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Later {
    /// When.
    at: Instant,
    /// On which character's session.
    session: u32,
    /// The command.
    line: String,
}

/// What a play window asks for when a key performs `action`: what its own
/// button asks.
pub(super) fn asked(action: Action) -> Asked {
    match action {
        Action::Stop => Asked::Stop,
        Action::Settings => Asked::Settings(None),
    }
}

impl App {
    /// `made`, a send macro, on `seat` from `now`: its commands before any
    /// wait sent at once, the rest kept until their waits are over.
    pub(super) fn send_macro(&mut self, seat: &Arc<Seat>, made: &Macro, now: Instant) {
        let Ok(steps) = made.steps() else {
            return;
        };
        let mut at = now;
        for step in steps {
            match step {
                Step::Wait(wait) => at += wait,
                Step::Line(line) if at <= now => self.sessions.send(seat, line),
                Step::Line(line) => self.later.push(Later {
                    at,
                    session: seat.id.0,
                    line,
                }),
            }
        }
    }

    /// The kept commands whose wait is over by `now`, sent in the order
    /// they were kept, to their characters among `seats`; one whose
    /// character has gone is dropped. How long until the next is due.
    pub(super) fn send_due(&mut self, seats: &[Arc<Seat>], now: Instant) -> Option<Duration> {
        let (due, kept): (Vec<Later>, Vec<Later>) =
            self.later.drain(..).partition(|later| later.at <= now);
        self.later = kept;
        for later in due {
            if let Some(seat) = seats.iter().find(|seat| seat.id.0 == later.session) {
                self.sessions.send(seat, later.line);
            }
        }
        self.later
            .iter()
            .map(|later| later.at.saturating_duration_since(now))
            .min()
    }
}
