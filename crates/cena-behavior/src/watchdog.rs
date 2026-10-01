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
/// (every one of them has its own deadline, §5.5). One turn of the hunt can
/// be longer than this (a walk, a selling round), so the hunt beats at every
/// event it folds too (the crate review of 2026-10-01, BE-A-4).
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

/// How long a behavior the player stopped is given to finish its own ending:
/// a follower's `leave group`, a leader's board closed, *"Hunt: stopped."*
/// said. Its longest wait there is one send's five seconds; this is that and
/// room to spare.
pub const WIND_DOWN: Duration = Duration::from_secs(8);

/// Run a behavior's `run` beside its `watched` watchdog, and give it its
/// ending: when the player stops it, `run` is awaited on, up to
/// [`WIND_DOWN`], so the behavior sees its own cancel and cleans up after
/// itself; only a run that will not end in that time, or a wedged one, is cut
/// off with `cut`.
///
/// **Why not a plain `select!`.** The desk raced the two, and the watcher is
/// ready the moment the stop is: the run, still on its way out, was dropped
/// mid-cleanup. A stopped follower never sent `leave group`, a stopped leader
/// never closed its board, and the leader's rest then waited on the stale
/// follower for good (the review of 2026-09-29).
pub async fn outlasting<T>(
    run: impl Future<Output = T>,
    watched: impl Future<Output = Watched>,
    cut: impl FnOnce(Watched) -> T,
) -> T {
    let mut run = std::pin::pin!(run);
    tokio::select! {
        // The run first: one that has ended is never cut.
        biased;
        end = &mut run => end,
        watched = watched => match watched {
            Watched::Stopped => match tokio::time::timeout(WIND_DOWN, &mut run).await {
                Ok(end) => end,
                Err(_) => cut(Watched::Stopped),
            },
            wedged @ Watched::Wedged(_) => cut(wedged),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A run that cleans up after its stop: a second of work once cancelled.
    async fn cleaning(stop: CancellationToken) -> &'static str {
        stop.cancelled().await;
        tokio::time::sleep(Duration::from_secs(1)).await;
        "cleaned up"
    }

    #[tokio::test(start_paused = true)]
    async fn a_stopped_run_is_given_its_ending() {
        let stop = CancellationToken::new();
        let watched = {
            let stop = stop.clone();
            async move {
                stop.cancelled().await;
                Watched::Stopped
            }
        };
        stop.cancel();
        let end = outlasting(cleaning(stop.clone()), watched, |_| "cut off").await;
        assert_eq!(end, "cleaned up");
    }

    #[tokio::test(start_paused = true)]
    async fn a_run_that_will_not_end_is_cut_off_after_the_wind_down() {
        let started = Instant::now();
        let end = outlasting(
            std::future::pending::<&str>(),
            async { Watched::Stopped },
            |_| "cut off",
        )
        .await;
        assert_eq!(end, "cut off");
        assert!(started.elapsed() >= WIND_DOWN);
    }
}
