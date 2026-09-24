//! When a connection becomes [`State::Ready`](crate::State::Ready).
//!
//! # `Ready` used to mean nothing
//!
//! `SessionActor::run` stepped `Authenticating -> Syncing -> Ready` before its
//! first read, so `Ready` said "the loop has started", not "the login burst
//! has arrived". `plan/12` §5.1 defines it as *"state is trustworthy;
//! behaviors may run"* and §5.2 makes `Ready` distinct from `Connected`
//! precisely so a reconnect cannot silently make old state trustworthy. A
//! `Ready` published before a single byte was read did exactly that.
//!
//! # The rule: the first `<prompt>` after `<endSetup/>`
//!
//! **AUTHOR, 2026-09-23:** `<endSetup/><app char=... game=.../>` should
//! indicate ready. `<endSetup/>` alone is not quite it: MEASURED in two fresh
//! logins (`GSIV-Nisugi/2026/09/2026-09-22_18-20-52.xml` and
//! `2026-09-21_23-55-20.xml`), `<endSetup/>` is at line 154/155 and the
//! indicators and vitals arrive AFTER it, with the first `<prompt>` at line
//! 228/231. Lich reached the same conclusion from the other side:
//! `reference/lich-5/lib/gemstone/detachable_client_init.rb:28-37` -- *"the
//! first `<prompt>` follows everything the push describes ... whereas
//! `IconJOINED` and `<endSetup/>` arrive before some of it."* Lich keys on the
//! prompt alone; Cena keys on the prompt AFTER the marker, so that a prompt
//! the server sends before setup ends cannot open the gate early.
//!
//! **Per connection, for free.** A supervisor builds a new actor per
//! generation (`actor.rs`, "one actor is one connection"), so this struct is
//! fresh on every reconnect -- which re-logs in and re-sends the burst.
//!
//! # The fallback, and why 30 seconds
//!
//! A server that never sends `<endSetup/>` -- a changed protocol, or a
//! recording cut without the login -- must not leave the session un-`Ready`
//! forever, because every behavior would then be refused for the life of the
//! connection with nothing in the log to say why. So once
//! [`SETUP_DEADLINE`] has passed since the connection's FIRST BYTE, the next
//! prompt makes the session `Ready` anyway, and the actor logs that it did.
//!
//! 30 seconds. MEASURED: the burst is ~230 lines ending in its first prompt
//! (the two captures above). INFERRED, not timed: the server pushes it
//! unprompted as one burst, so it lands in a second or two -- the captures
//! carry no per-line clock to prove that. 30 s is an order of magnitude over
//! it, so the fallback does not fire on a slow login that WAS going to send
//! the marker. The cost of
//! it firing is bounded the other way: behaviors wait 30 s on a server that
//! no longer marks its setup, which is noticeable, logged, and not a hang.
//! It equals the backoff ladder's top rung and `OWED_PROMPT_DEADLINE`, so the
//! session has one "something is slow" constant rather than three.
//!
//! Measured from the first byte rather than from the actor's start, because a
//! connection whose server has not spoken yet has not been slow to finish its
//! setup -- it has not begun it.

use cena_protocol::Frame;
use std::time::Duration;
use tokio::time::Instant;

/// How long after the first byte a missing `<endSetup/>` stops holding the
/// session in `Syncing`. See the module docs.
pub const SETUP_DEADLINE: Duration = Duration::from_secs(30);

/// What this connection has seen of its login burst.
#[derive(Debug, Default)]
pub(super) struct Readiness {
    /// `<endSetup/>` has arrived on this connection.
    setup_ended: bool,
    /// When the connection's first byte arrived. `tokio` time, so a test
    /// under `start_paused` controls it.
    first_byte: Option<Instant>,
}

/// Why a prompt made the session `Ready`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Verdict {
    /// The first prompt after `<endSetup/>`: the rule.
    AfterSetup,
    /// No `<endSetup/>` within [`SETUP_DEADLINE`]: the fallback.
    SetupNeverEnded,
}

impl Readiness {
    /// Bytes arrived. Only the first call is recorded.
    pub(super) fn bytes_arrived(&mut self) {
        self.first_byte.get_or_insert_with(Instant::now);
    }

    /// Whether `frame` makes a `Syncing` connection `Ready`.
    pub(super) fn frame(&mut self, frame: &Frame) -> Option<Verdict> {
        match frame {
            Frame::EndSetup => {
                self.setup_ended = true;
                None
            }
            Frame::Prompt { .. } if self.setup_ended => Some(Verdict::AfterSetup),
            Frame::Prompt { .. }
                if self
                    .first_byte
                    .is_some_and(|at| at.elapsed() >= SETUP_DEADLINE) =>
            {
                Some(Verdict::SetupNeverEnded)
            }
            _ => None,
        }
    }
}
