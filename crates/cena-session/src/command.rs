//! What a command is, and what comes back: [`Outcome`], [`CommandId`],
//! [`Envelope`].
//!
//! `plan/12` §4.5 already specifies [`Outcome`], so it is taken verbatim
//! rather than redesigned. The rest of this module is the plumbing that makes
//! `send_and_await` **one call** with no public send-then-wait pair, which is
//! how §4.5 makes the arm-before-send race unwritable rather than merely
//! discouraged.

use crate::lifecycle::Generation;
use cena_protocol::Frame;
use tokio::sync::oneshot;

/// A correlation id for logs and the audit trail.
///
/// **Not for wire matching.** `plan/12` §4.4 corrected an earlier draft on
/// exactly this: "The game carries no command ids. That table was
/// unimplementable. Attribution is *temporal*." This id exists so a reader can
/// reconstruct which command a late line probably belonged to, and for nothing
/// else.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommandId(pub u64);

/// Where a command came from. `plan/12` §4.1's correction lives here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// The player typed it.
    ///
    /// **Manual input is not a claimant** (`plan/12` §4.1, CORRECTED
    /// 2026-09-18). It jumps the queue and it never touches the authority
    /// token. An earlier draft made it priority 1 and preemptive, which meant
    /// typing `say hi` mid-hunt would abort Hunt. Neither reference
    /// implementation does that.
    Manual,
    /// A behavior sent it, carrying the token it claimed.
    ///
    /// The token is here so the session can ENFORCE §4.2 rather than merely
    /// describe it: a command from a behavior that does not hold the authority
    /// is refused, not queued. Before this, `claim`/`release`/`authority`
    /// existed on `CommandQueue` and **nothing in the session ever called
    /// them**, so two behaviors sharing a session would both have their
    /// commands sent in FIFO order -- verbatim the failure §4.2 names, "an
    /// attack that fires four seconds after the fight ended".
    Behavior(crate::queue::AuthorityToken),
}

impl Origin {
    /// The claimant's token, if a behavior sent this.
    #[must_use]
    pub const fn token(self) -> Option<crate::queue::AuthorityToken> {
        match self {
            Self::Manual => None,
            Self::Behavior(token) => Some(token),
        }
    }

    /// Whether a behavior sent this, whoever holds the authority.
    #[must_use]
    pub const fn is_behavior(self) -> bool {
        matches!(self, Self::Behavior(_))
    }
}

/// Why a command was not run.
///
/// The three game-state refusals map onto the exact conditions Lich's `fput`
/// keys on (`reference/lich-5/lib/global_defs.rb:1555` roundtime,
/// `:1577` stunned/webbed). **Step 2 detects and reports; it does not
/// resend.** Lich's resend ladder with `max_resends` is behavior policy, and
/// building it now would be a config option with no second caller (Rule -1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Try again later: the queue was full, or the session was busy.
    Transient,
    /// Will never succeed as issued.
    Permanent,
    /// "...wait N seconds."
    Roundtime,
    /// The character is stunned.
    Stunned,
    /// The character is webbed.
    Webbed,
}

/// What a round trip produced. Verbatim from `plan/12` §4.5.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The window closed with a match. Carries the frame that matched.
    Confirmed(Box<Frame>),
    /// No match within the window.
    ///
    /// **Never "the command did not happen"** (`plan/12` §4.4). A slow line
    /// can arrive after its own prompt; no client can eliminate that, so a
    /// behavior must tolerate a matching line arriving one window late.
    Timeout,
    /// Preempted or cancelled. Cancellation does not un-send: the command may
    /// already have reached the game (`plan/12` §4.4).
    Interrupted,
    /// The session is gone.
    Dead,
    /// Not run, with a reason.
    Refused(Refusal),
}

/// A command handed to the session, with the channel its answer goes back on.
#[derive(Debug)]
pub struct Envelope {
    /// The id, for logs.
    pub id: CommandId,
    /// The line to send, without a trailing newline.
    pub line: String,
    /// Manual or behavior. Decides queue position, never authority.
    pub origin: Origin,
    /// Where the [`Outcome`] goes. `oneshot` is the shape that makes
    /// `send_and_await` one call: the sender is created and moved into the
    /// queue in the same expression that creates the receiver, so there is no
    /// moment at which a caller holds a sent command and no armed waiter.
    pub reply: oneshot::Sender<Outcome>,
    /// The connection this command belongs to. `plan/12` §5.2: anything from
    /// a prior generation is discarded.
    pub generation: Generation,
    /// Which frame answers this command (`plan/12` §4.4, "the waiter's
    /// matcher"). Supplied by the caller, because only the caller knows what
    /// it asked for.
    pub matcher: crate::queue::Matcher,
}

/// What the session's inbox carries.
///
/// One channel, not two: a `Claim` sent after a command must not be able to
/// overtake it, and two channels in a `select!` give no ordering guarantee
/// between them. `plan/12` §4.2's rule is about ordering, so the transport has
/// to preserve it.
#[derive(Debug)]
pub enum Inbox {
    /// Run this command.
    Command(Box<Envelope>),
    /// Take the authority, or report who holds it.
    Claim {
        /// The token the caller wants to hold.
        token: crate::queue::AuthorityToken,
        /// `Ok(())` if granted, `Err(AuthorityHeld)` naming the current holder.
        reply: oneshot::Sender<Result<(), crate::queue::AuthorityHeld>>,
    },
    /// Give the authority back. A release by a non-holder is ignored.
    Release(crate::queue::AuthorityToken),
}

/// A handle a caller uses to reach the session.
///
/// Cloneable, so a behavior and the manual-input surface hold the same handle
/// and their commands therefore go through the **same queue** -- which is
/// literally criterion 3 (`plan/12:459`).
#[derive(Clone, Debug)]
pub struct SessionHandle {
    sender: tokio::sync::mpsc::Sender<Inbox>,
    generation: Generation,
}

impl SessionHandle {
    /// Wrap a sender. Called by [`crate::actor`] when it builds the session.
    #[must_use]
    pub fn new(sender: tokio::sync::mpsc::Sender<Inbox>, generation: Generation) -> Self {
        Self { sender, generation }
    }

    /// The generation this handle is bound to.
    #[must_use]
    pub fn generation(&self) -> Generation {
        self.generation
    }

    /// Send a command and wait for its typed [`Outcome`]. **One call.**
    ///
    /// `plan/12` §4.5: "`send_and_await` is one call. There is no public
    /// send-then-wait pair, so the race cannot be written." That is why this
    /// module exports no `send` and no `await_outcome`.
    ///
    /// `try_send`, never `send().await`. A blocking send from the UI is how a
    /// slow session wedges the frontend; a full queue is
    /// [`Refusal::Transient`], which the caller can act on, rather than a
    /// stall it cannot see. `plan/12` §5.5 requires bounded channels
    /// everywhere and no unbounded wait.
    ///
    /// The `deadline` is this call's, applied with `tokio::time::timeout`.
    /// §5.5: "every wait has a deadline".
    ///
    /// # Errors
    ///
    /// Never. The failure modes are values: a closed channel is
    /// [`Outcome::Dead`], a full one is [`Outcome::Refused`].
    pub async fn send_and_await(
        &self,
        id: CommandId,
        line: &str,
        origin: Origin,
        deadline: std::time::Duration,
        matcher: crate::queue::Matcher,
    ) -> Outcome {
        let (reply, answer) = oneshot::channel();
        let envelope = Envelope {
            id,
            line: line.to_owned(),
            origin,
            reply,
            generation: self.generation,
            matcher,
        };
        match self.sender.try_send(Inbox::Command(Box::new(envelope))) {
            Ok(()) => {}
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                return Outcome::Refused(Refusal::Transient);
            }
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => return Outcome::Dead,
        }
        match tokio::time::timeout(deadline, answer).await {
            // The actor resolved it.
            Ok(Ok(outcome)) => outcome,
            // The actor dropped the sender: the session ended mid-flight.
            Ok(Err(_)) => Outcome::Dead,
            // The actor is alive but the window never closed.
            Err(_elapsed) => Outcome::Timeout,
        }
    }

    /// Take the command authority.
    ///
    /// `plan/12` §4.2: "A behavior that wants the authority while another
    /// holds it gets `Err(AuthorityHeld)`. It does not queue behind it."
    /// **This call does not queue** -- it resolves immediately either way, so
    /// a losing behavior finds out now rather than acting four seconds late.
    ///
    /// # Errors
    ///
    /// [`AuthorityHeld`](crate::queue::AuthorityHeld) naming the holder, or
    /// if the session is gone -- a dead session holds nothing a caller can
    /// use, so it is reported as held by no-one it can displace.
    pub async fn claim(
        &self,
        token: crate::queue::AuthorityToken,
    ) -> Result<(), crate::queue::AuthorityHeld> {
        let (reply, answer) = oneshot::channel();
        if self.sender.try_send(Inbox::Claim { token, reply }).is_err() {
            return Err(crate::queue::AuthorityHeld(token));
        }
        answer
            .await
            .unwrap_or(Err(crate::queue::AuthorityHeld(token)))
    }

    /// Give the authority back. Ignored if this token does not hold it.
    ///
    /// Fire-and-forget: a release has no answer worth waiting for, and a
    /// behavior releasing during cleanup must not block. §4.3 is explicit that
    /// cleanup "cannot send commands" -- this is not a command.
    pub fn release(&self, token: crate::queue::AuthorityToken) {
        let _ = self.sender.try_send(Inbox::Release(token));
    }
}
