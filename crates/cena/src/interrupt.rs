//! Ctrl-C, routed through the orderly shutdown from every phase of a run.
//!
//! # Why this is a module and not a `select!` arm
//!
//! It WAS a `select!` arm, in the hold at the end of M1's single-session run,
//! and every phase before it had no handler: Ctrl-C got the OS default and
//! the process died on the spot -- no `quit` (`plan/16` §5b), so the
//! character was left link-dead, and no log flush (review finding 8). So
//! Ctrl-C is a [`CancellationToken`] cancelled by one listener installed
//! once, and the run ends by its one orderly path when it fires
//! (`play.rs`).
//!
//! # A second Ctrl-C exits at once
//!
//! Installing a handler replaces the OS default for the rest of the process.
//! Without an escape, a shutdown that hangs -- `quit` waits up to 10s for the
//! server, a flush on a stuck disk -- could only be ended from another
//! terminal. The second press is that escape, and it says what it skips.

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
