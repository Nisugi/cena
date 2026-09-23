//! Ctrl-C, routed through the orderly shutdown from every phase of a run.
//!
//! # Why this is a module and not a `select!` arm
//!
//! It WAS a `select!` arm -- one, in `frontend::wait_for_stop`, the hold at the
//! end of a run. Every phase before it had no handler, so Ctrl-C during
//! `wait_for_room` (20s), `--demo`, `--capture`/`--psm`/`--typeahead` (about
//! 30s each) or `--first` got the OS default: the process died on the spot. No
//! `quit` (`plan/16` §5b), so the character was left link-dead; no sink flush,
//! so the log lost its tail; the combat recorder's thread never joined, so its
//! last hunt stayed open. And `main.rs` said, above the hold, that Ctrl-C "falls
//! through to the SAME orderly shutdown" -- true of the one phase that had the
//! arm, and read as true of the run (review finding 8).
//!
//! So Ctrl-C is now a [`CancellationToken`] cancelled by one listener installed
//! once, and every phase between login and logout is raced against it with
//! [`unless_interrupted`]. An interrupted phase is DROPPED -- the same
//! cancellation every other wait in this workspace uses (`plan/12` §5.5) -- and
//! `main` carries on to the shutdown it would have reached anyway.
//!
//! # A second Ctrl-C exits at once
//!
//! Installing a handler replaces the OS default for the rest of the process.
//! Without an escape, a shutdown that hangs -- `quit` waits up to 10s for the
//! server, a flush on a stuck disk -- could only be ended from another
//! terminal. The second press is that escape, and it says what it skips.

use std::future::Future;
use tokio_util::sync::CancellationToken;

/// Listen for Ctrl-C for the rest of the process, and return the token the
/// first press cancels.
///
/// Called AFTER the credential prompts, deliberately: until a session exists
/// there is nothing to shut down in order, and the OS default -- die now -- is
/// exactly right for someone who changed their mind at the password prompt.
pub(crate) fn on_ctrl_c() -> CancellationToken {
    let interrupt = CancellationToken::new();
    let cancel = interrupt.clone();
    tokio::spawn(async move {
        if let Err(error) = tokio::signal::ctrl_c().await {
            // The default handler is still in place when registration fails,
            // so Ctrl-C still stops the process -- just not in order.
            eprintln!(
                "[interrupt] could not listen for Ctrl-C ({error}); pressing it \
                 will kill the process WITHOUT the orderly shutdown."
            );
            return;
        }
        eprintln!(
            "\n[interrupt] Ctrl-C: finishing the orderly shutdown (quit, log \
             flush). Press Ctrl-C again to exit at once."
        );
        cancel.cancel();
        if tokio::signal::ctrl_c().await.is_ok() {
            eprintln!(
                "[interrupt] second Ctrl-C: exiting WITHOUT the orderly \
                 shutdown -- the log's tail may be lost and the character may \
                 be left link-dead."
            );
            std::process::exit(130);
        }
    });
    interrupt
}

/// Run one phase of the session, unless Ctrl-C comes first.
///
/// `None` when interrupted, with the phase dropped mid-flight. That is safe for
/// every phase `main` passes here: each one only SENDS through a
/// `SessionHandle` or READS a subscription, and neither owns the socket -- the
/// supervisor does, and `main`'s shutdown ends it the same way whether a phase
/// finished or not.
///
/// Checks the token first, so a phase that follows an interrupted one is not
/// started at all -- `select!` would otherwise poll it once before noticing.
pub(crate) async fn unless_interrupted<T>(
    interrupt: &CancellationToken,
    phase: impl Future<Output = T>,
) -> Option<T> {
    if interrupt.is_cancelled() {
        return None;
    }
    tokio::select! {
        biased;
        () = interrupt.cancelled() => None,
        out = phase => Some(out),
    }
}

#[cfg(test)]
mod tests {
    use super::unless_interrupted;
    use std::time::Duration;
    use tokio_util::sync::CancellationToken;

    #[tokio::test(start_paused = true)]
    async fn an_interrupt_ends_a_phase_that_would_otherwise_run_on() {
        // The shape of finding 8: `wait_for_room` waits 20s, a capture 30s.
        // A phase that never finishes stands in for all of them.
        let interrupt = CancellationToken::new();
        let trigger = interrupt.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            trigger.cancel();
        });

        let ended = tokio::time::timeout(
            Duration::from_mins(1),
            unless_interrupted(&interrupt, std::future::pending::<()>()),
        )
        .await;
        assert_eq!(
            ended,
            Ok(None),
            "Ctrl-C did not end the phase, so the only way out was to kill the \
             process -- no quit, no log flush"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_phase_after_an_interrupt_is_not_started() {
        let interrupt = CancellationToken::new();
        interrupt.cancel();
        let mut started = false;
        let out = unless_interrupted(&interrupt, async {
            started = true;
        })
        .await;
        assert_eq!(out, None);
        assert!(
            !started,
            "a phase was started after Ctrl-C -- it may SEND to the game"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn an_uninterrupted_phase_returns_its_result() {
        let interrupt = CancellationToken::new();
        assert_eq!(unless_interrupted(&interrupt, async { 7 }).await, Some(7));
    }
}
