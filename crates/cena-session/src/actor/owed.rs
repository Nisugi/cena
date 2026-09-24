//! Whose prompt arrives next: the in-flight command's, or an instant action's.
//!
//! # Why a count was not enough
//!
//! `send_now` bypasses the queue, so its response is attributed to nothing --
//! but it still draws a prompt, and a prompt is what closes the in-flight
//! command's window (`plan/12` §4.4). Review SE-5 fixed that with a single
//! counter, `send_now_prompts_owed`, whose rule was "every owed prompt arrives
//! before the waiting command's". **That holds only for an instant action
//! written BEFORE the command it modifies**, which is the batching shape
//! SE-5 was about.
//!
//! Written AFTER -- a sigil while an `attack` is already on the wire -- the
//! server answers in wire order, so the attack's reply comes first. The
//! counter then suppressed the attack's own text, spent the attack's prompt as
//! the sigil's, and credited the sigil's text to the attack. VERIFIED before
//! this module, by the review's probe and now by
//! `tests/send_now.rs::an_instant_action_goes_out_while_a_window_is_open`:
//! the attack resolved `Confirmed("You feel a surge.")` on a wire reading
//! `["look", "attack", "sigil of power"]`.
//!
//! So what is recorded is **wire order**, in the one form it can take. At most
//! one window is open at a time, so every owed prompt is either ahead of that
//! window's prompt or behind it:
//!
//! * `before` -- instant actions written while no window was open, or before
//!   the in-flight command was. Their prompts arrive first.
//! * `after` -- instant actions written while the window was open. Their
//!   prompts arrive after the window's own, and once it has closed they are
//!   ahead of whatever is sent next, so they move to `before`.
//!
//! # The deadline (review finding 9)
//!
//! A prompt that never comes -- a command the server swallowed, a merged
//! reply -- used to leave the counter permanently high, and every later window
//! would then close one prompt late, **for the rest of the connection**. Each
//! owed prompt is therefore stamped, and one older than
//! [`OWED_PROMPT_DEADLINE`] is presumed never to arrive.
//!
//! INFERRED, not measured: no instant action has been observed to draw no
//! prompt. The bound is a safety net for the case this ledger cannot see, and
//! it is deliberately generous so that it never fires on a reply that is
//! merely slow.

use std::collections::VecDeque;
use std::time::Duration;
use tokio::time::Instant;

/// How long an instant action's prompt is waited for before it is presumed
/// lost.
///
/// An instant action incurs no roundtime of its own and is answered at network
/// speed -- a sigil sent during roundtime is answered at once with "...wait",
/// not held. Thirty seconds is two orders of magnitude over a slow round trip,
/// and equal to the backoff ladder's top rung: long enough that expiring an
/// entry whose prompt WAS coming is not a realistic failure, short enough that
/// a lost one costs a single window rather than the connection. INFERRED; see
/// the module docs.
pub(super) const OWED_PROMPT_DEADLINE: Duration = Duration::from_secs(30);

/// The prompts owed to instant actions, in wire order relative to the one
/// window that may be open. See the module docs.
#[derive(Debug, Default)]
pub(super) struct OwedPrompts {
    /// Owed prompts that arrive BEFORE the in-flight window's, stamped with
    /// when their instant action was written.
    before: VecDeque<Instant>,
    /// Owed prompts that arrive AFTER it.
    after: VecDeque<Instant>,
}

impl OwedPrompts {
    /// An instant action has just been written.
    ///
    /// `window_open` is whether a command was already on the wire awaiting its
    /// prompt when these bytes went out -- the fact that decides which side of
    /// that prompt this one's lands on.
    pub(super) fn instant_sent(&mut self, window_open: bool) {
        let now = Instant::now();
        if window_open {
            self.after.push_back(now);
        } else {
            self.before.push_back(now);
        }
    }

    /// Whether frames arriving now answer the in-flight window.
    ///
    /// False while an instant action written ahead of it is still owed its
    /// prompt: everything up to that prompt is the instant action's response.
    /// Skipping only the terminator would leave the text in between credited
    /// to whoever happened to be waiting (the original SE-5 follow-up).
    pub(super) fn window_is_answered_next(&mut self) -> bool {
        self.expire();
        self.before.is_empty()
    }

    /// A prompt arrived. Returns whether it is the in-flight window's.
    ///
    /// `window_open` says whether there is a window for it to close. With none
    /// open and nothing owed, a prompt belongs to nobody -- the game prompts
    /// after output it sent unasked -- and `false` is correct for that too.
    pub(super) fn prompt(&mut self, window_open: bool) -> bool {
        self.expire();
        if self.before.pop_front().is_some() {
            return false;
        }
        if !window_open {
            return false;
        }
        // The window's own prompt. What was written after its command is now
        // ahead of whatever is sent next.
        self.before = std::mem::take(&mut self.after);
        true
    }

    /// How many prompts are owed, both sides together. For logs and tests.
    #[cfg(test)]
    fn len(&self) -> usize {
        self.before.len() + self.after.len()
    }

    /// Forget owed prompts at the front that are past [`OWED_PROMPT_DEADLINE`].
    ///
    /// Only `before` is expired: it is the side that can steal a window's
    /// prompt. An `after` entry moves to `before` when its window closes and is
    /// judged there, by the stamp it was written with.
    fn expire(&mut self) {
        let now = Instant::now();
        while self
            .before
            .front()
            .is_some_and(|sent| now.saturating_duration_since(*sent) >= OWED_PROMPT_DEADLINE)
        {
            self.before.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn an_instant_action_sent_before_the_command_takes_the_first_prompt() {
        let mut owed = OwedPrompts::default();
        owed.instant_sent(false);
        assert!(!owed.window_is_answered_next());
        assert!(!owed.prompt(true), "the sigil's prompt is not the window's");
        assert!(owed.window_is_answered_next());
        assert!(owed.prompt(true));
        assert_eq!(owed.len(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn an_instant_action_sent_after_the_command_waits_behind_its_prompt() {
        let mut owed = OwedPrompts::default();
        owed.instant_sent(true);
        assert!(
            owed.window_is_answered_next(),
            "the command's reply is first"
        );
        assert!(owed.prompt(true), "and so is its prompt");
        assert!(!owed.window_is_answered_next(), "then the sigil's is owed");
        assert!(!owed.prompt(true), "and it does not close the next window");
        assert_eq!(owed.len(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn an_owed_prompt_that_never_comes_expires() {
        let mut owed = OwedPrompts::default();
        owed.instant_sent(false);
        tokio::time::advance(OWED_PROMPT_DEADLINE).await;
        assert!(owed.window_is_answered_next());
        assert!(owed.prompt(true));
    }
}
