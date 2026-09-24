//! A wedged behavior is detectable (`plan/12` §5.5), and taken off the
//! character.
//!
//! # Progress is a heartbeat, not traffic
//!
//! §5.5 asks for "no progress within `BEHAVIOR_WATCHDOG` -> log, cancel".
//! Progress cannot be "sent a command": a hunt resting in town waits minutes
//! for mana with nothing to send (eohunter rechecks every 30 s, `rest.rb`),
//! and a walk waits out a ferry. What a wedged behavior stops doing is
//! **turning its loop** -- an await that never returns, a loop that never
//! yields. So a behavior beats a [`Heartbeat`] each time round its loop, and
//! a watchdog beside it -- not inside it, because a stuck loop cannot notice
//! itself -- preempts it when the beats stop.
//!
//! Preempting is `plan/12` §4.3's stop: its cancel token first, then, if it
//! has not let go within `PREEMPT_GRACE`, the authority is taken, and its
//! queued commands are refused.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use cena_session::{Notice, NoticeKind, Preempted, SessionHandle};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

/// How long a behavior may go without turning its loop before it is
/// stopped.
///
/// **A starting value** (`plan/12` §9 leaves the class to measurement). The
/// loop it bounds turns about four times a second -- eohunter's engine sleeps
/// 0.25 s between ticks (`runner.rb:36`) -- so thirty seconds is over a
/// hundred missed turns, and far longer than any one await a behavior makes
/// (every one of them has its own deadline, §5.5).
pub const BEHAVIOR_WATCHDOG: Duration = Duration::from_secs(30);

/// When a behavior last turned its loop. Cloned into the watchdog.
#[derive(Clone, Debug)]
pub struct Heartbeat(Arc<Mutex<Instant>>);

impl Default for Heartbeat {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(Instant::now())))
    }
}

impl Heartbeat {
    /// The loop turned.
    pub fn beat(&self) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = Instant::now();
    }

    fn since(&self) -> Duration {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .elapsed()
    }
}

/// How a watch ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Watched {
    /// The behavior stopped on its own, or was stopped: `stop` was cancelled.
    Stopped,
    /// It went [`BEHAVIOR_WATCHDOG`] without a beat, and was preempted.
    Wedged(Preempted),
}

/// Watch a behavior until it stops, preempting it if its heartbeat stops
/// for `limit` ([`BEHAVIOR_WATCHDOG`] in play). `stop` is the behavior's own
/// cancel token; `name` is what the player is told stopped.
pub async fn watch(
    handle: &SessionHandle,
    stop: &CancellationToken,
    heartbeat: &Heartbeat,
    limit: Duration,
    name: &str,
) -> Watched {
    loop {
        tokio::select! {
            () = stop.cancelled() => return Watched::Stopped,
            () = tokio::time::sleep(limit / 4) => {}
        }
        let quiet = heartbeat.since();
        if quiet >= limit {
            let preempted = handle.preempt(stop).await;
            handle.say(Notice::line(
                NoticeKind::Error,
                format!(
                    "{name} stopped responding for {}s, so Hydra stopped it.",
                    quiet.as_secs()
                ),
            ));
            return Watched::Wedged(preempted);
        }
    }
}
