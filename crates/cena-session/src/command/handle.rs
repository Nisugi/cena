//! The transport: [`SessionHandle`], [`Envelope`], [`Inbox`].
//!
//! How a command gets from a caller to the actor, as against [`verdict`]'s
//! vocabulary of what a command IS and what came back.
//!
//! `plan/12` §4.5 already specifies [`Outcome`], so it is taken verbatim
//! rather than redesigned. The rest of this module is the plumbing that makes
//! `send_and_await` **one call** with no public send-then-wait pair, which is
//! how §4.5 makes the arm-before-send race unwritable rather than merely
//! discouraged.
//!
//! # Why this is a third file and not `mod.rs`
//!
//! Because `mod.rs` may not implement. An earlier draft of this split left the
//! transport in `mod.rs` and said so in its header -- "this file is not a
//! facade" -- and `facade_files_stay_facades` rejected it at once, naming four
//! `impl` blocks. **The rule is enforced, not advisory** (`plan/05` §0: a rule
//! that is not enforced is a wish), and the arch test was right: a `mod.rs`
//! that wires AND implements is the shape the rule exists to prevent.
//!
//! [`verdict`]: super::verdict

use super::verdict::{CommandId, Gate, Origin, Outcome, Refusal, Sent};
use crate::lifecycle::Generation;
use tokio::sync::oneshot;

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
    /// Send this line **now**, opening no window (`plan/16` §1.4).
    ///
    /// On the same channel as [`Self::Command`] deliberately. An instant
    /// action's whole purpose is to precede the command it modifies -- "a few
    /// in a row, since they activate instantly and the command triggers"
    /// (AUTHOR) -- and a second channel would give no ordering guarantee
    /// between the sigil and the attack it is supposed to modify. Same reason
    /// `Claim` rides here.
    SendNow {
        /// The line to send, without a trailing newline.
        line: String,
        /// Manual, behavior or script. Recorded in the log and published on
        /// [`Event::Sent`](crate::Event::Sent); it decides nothing here,
        /// because an instant action has no queue position to decide.
        origin: Origin,
        /// The connection this belongs to (`plan/12` §5.2).
        generation: Generation,
        /// Whether the roundtime gate applies.
        gate: Gate,
        /// Where the immediate verdict goes -- **not** a round-trip outcome.
        reply: oneshot::Sender<Sent>,
    },
    /// Send the exit command and **wait for the server to close the socket**
    /// (`plan/16` §5b).
    ///
    /// # Why this rides the command channel
    ///
    /// Because it must be **ordered behind everything already queued**. A quit
    /// delivered out of band could overtake a command a behavior sent a
    /// microsecond earlier, and the last thing a session did before exiting
    /// would be lost. Arriving here means every earlier `Inbox` has been
    /// handled, which is the ordering guarantee a clean exit needs and the same
    /// reason `Claim` and `SendNow` ride here.
    ///
    /// # Why it is not just `cancel()`
    ///
    /// Cancelling breaks the read loop, and **the read loop is what observes
    /// the EOF**. A quit sent after a cancel would have no reader left to see
    /// the server close, so the one thing §5b asks for -- telling *the server
    /// closed because we asked* from *the connection dropped* -- would be
    /// unobservable. The actor therefore stays alive, sends the command, and
    /// keeps reading until the stream ends.
    Quit {
        /// How long to wait for the server's EOF before giving up on it.
        ///
        /// Bounded because the socket must close either way: `plan/12`
        /// criterion 6's "no leaked sockets" may not become conditional on the
        /// server cooperating.
        timeout: std::time::Duration,
        /// How the exit went. Dropping this receiver is fine -- a caller that
        /// stops caring does not stop the shutdown.
        reply: oneshot::Sender<Farewell>,
    },
}

/// How an orderly exit went (`plan/16` §5b.3).
///
/// Ports the three-way distinction Lich draws at
/// `reference/lich-5/lib/common/orderly_shutdown.rb:181-191`, where sending the
/// command, the reader stopping in time, and the stream actually reaching EOF
/// are three separate checks that can each fail on their own.
///
/// **All three still close the socket.** This reports what happened; it does
/// not decide whether to clean up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Farewell {
    /// The command went out and the server closed the stream. A clean exit.
    Acknowledged,
    /// The command went out and the server never closed within the timeout.
    ///
    /// Lich raises `ServerExitTimeout` here. The session still ends -- the
    /// difference is that the game may not have registered the logout, so a
    /// character can be left in-world for the usual link-dead interval.
    TimedOut,
    /// The command could not be written at all: the transport was already
    /// gone.
    ///
    /// **Not a failure of the shutdown** -- there is nothing to say goodbye to.
    /// Distinguished from [`Self::TimedOut`] because a log that conflates them
    /// cannot tell a dead socket from an unresponsive server.
    Unsent,
}

/// A handle a caller uses to reach the session.
///
/// Cloneable, so a behavior and the manual-input surface hold the same handle
/// and their commands therefore go through the **same queue** -- which is
/// literally criterion 3 (`plan/12:459`).
#[derive(Clone, Debug)]
pub struct SessionHandle {
    sender: tokio::sync::mpsc::Sender<Inbox>,
    /// **Shared, not copied.** A handle cloned before a reconnect must keep
    /// working afterwards; see [`GenerationCell`] for why that does not weaken
    /// `plan/12` §4.4's discard rule.
    generation: crate::lifecycle::GenerationCell,
}

impl SessionHandle {
    /// Wrap a sender. Called by [`crate::actor`] when it builds the session.
    #[must_use]
    pub fn new(
        sender: tokio::sync::mpsc::Sender<Inbox>,
        generation: crate::lifecycle::GenerationCell,
    ) -> Self {
        Self { sender, generation }
    }

    /// The generation this handle stamps **right now**.
    ///
    /// Reads the shared cell rather than a copy taken at construction, so a
    /// handle cloned in an earlier generation reports the current one.
    #[must_use]
    pub fn generation(&self) -> Generation {
        self.generation.get()
    }

    /// The shared counter this handle reads its generation from.
    ///
    /// For a supervisor, which advances it between connections. Ordinary
    /// callers want [`Self::generation`], which is the value rather than the
    /// cell.
    #[must_use]
    pub fn generation_cell(&self) -> crate::lifecycle::GenerationCell {
        self.generation.clone()
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
            generation: self.generation.get(),
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

    /// Send a line **without opening a round-trip window** (`plan/16` §1.4).
    ///
    /// # What this is for
    ///
    /// Any command that incurs **no roundtime of its own** and whose answer is
    /// checked *later* rather than waited on.
    ///
    /// > **AUTHOR, 2026-09-18:** *"anything that doesn't cause roundtime would
    /// > be an instant action."*
    ///
    /// In practice, in rough order of volume (`plan/16` §5.2g):
    ///
    /// | Caller | Verified by |
    /// |---|---|
    /// | **`travel`** -- a run of cardinal directions over a known route | arrival, at the end |
    /// | observation -- `look`, `look <target>`, `assess <target>` | the text returned |
    /// | instant abilities -- sigils, `515`, `140` | an `Effects` id lookup |
    /// | combat openers -- `target`, `stance offensive`, `attack` | the attack's own result |
    ///
    /// **`move` is NOT in this list**, and the distinction is the author's:
    ///
    /// > *"movement is instant and we care. True but it depends. Let's call it
    /// > `move` and `travel` so move we care and travel we dont."*
    ///
    /// A single `move` is instant but its answer decides what happens next, so
    /// it belongs on [`Self::send_and_await`]. `travel` is the same command
    /// batched over a route the mapdb already knows, verifying at the end --
    /// which is what makes fast travel possible, and what every measurement in
    /// `plan/16` §5.2 was for.
    ///
    /// Originally written for instant abilities alone:
    ///
    /// > **AUTHOR, 2026-09-18:** *"those sigils using fput suck because they
    /// > wait for a response instead of being instant."*
    ///
    /// Cena had the same defect structurally, and it was worst exactly where
    /// it hurts most. [`CommandQueue::take_next`](crate::CommandQueue::take_next)
    /// yields nothing while a window is open, and `send_and_await` was the
    /// **only** send path -- so `sigil of escape`, the ability for leaving a
    /// fight you are losing, queued behind whatever the running behavior last
    /// sent, for up to that command's full roundtime.
    ///
    /// # How it differs from [`Self::send_and_await`]
    ///
    /// | | `send_and_await` | `send_now` |
    /// |---|---|---|
    /// | Opens an `InFlight` window | yes | **no** |
    /// | Blocked by an open window | yes | **no** |
    /// | Gated on | the queue | **roundtime** |
    /// | Returns | the round trip's outcome | whether the bytes went out |
    ///
    /// Those are two different gates, not a strict-vs-relaxed version of one.
    /// `send_and_await` serialises round trips so attribution works
    /// (`plan/12` §4.4); `send_now` has no attribution to protect because it
    /// waits for nothing.
    ///
    /// # The return is a SEND verdict, not a round trip
    ///
    /// [`Sent::Ok`] means **the bytes reached the wire**, carrying the server
    /// second the gate was decided against -- the evidence it ran on, so a log
    /// can show what the decision was made on. It does **not** mean the action
    /// fired. That is confirmed by its
    /// **effect** (`plan/16` §2):
    ///
    /// > **AUTHOR:** *"we can still verify the event happened from those
    /// > instant actions because they all return with an effect in buffs or
    /// > cooldowns or whathaveyou."*
    ///
    /// which is an [`Effects`](cena_model::Effects) lookup on the id, and
    /// deliberately not this function's business.
    ///
    /// # Unknown is not "go ahead"
    ///
    /// With [`Gate::Roundtime`] and an unknown clock -- no prompt seen yet --
    /// this refuses [`Refusal::Transient`]. `plan/12` §5.2 makes `Unknown`
    /// first-class precisely so it cannot be silently read as `false`, and
    /// `Transient` is the honest answer: try again once a prompt has arrived.
    /// It is not [`Refusal::Roundtime`], which would claim knowledge of a
    /// roundtime nobody has reported.
    ///
    /// # Why there is no deadline here
    ///
    /// `plan/12` §5.5 requires that "every wait has a deadline", and
    /// `send_and_await` takes one. This does not, because **it is not the same
    /// kind of wait.** `send_and_await` waits on the *game* -- an unbounded
    /// party that may never answer -- so it needs a bound. This waits on the
    /// *actor*, which answers in the same turn it receives the message: there
    /// is no window to close and no frame to arrive. The one way it never
    /// answers is the actor being gone, and that drops the sender, which
    /// resolves immediately as [`Outcome::Dead`]. A timeout here would be a
    /// deadline on a wait that cannot hang.
    ///
    /// # Errors
    ///
    /// Never. Like `send_and_await`, the failure modes are values.
    pub async fn send_now(&self, line: &str, origin: Origin, gate: Gate) -> Sent {
        let (reply, answer) = oneshot::channel();
        let message = Inbox::SendNow {
            line: line.to_owned(),
            origin,
            generation: self.generation.get(),
            gate,
            reply,
        };
        match self.sender.try_send(message) {
            Ok(()) => {}
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                return Sent::Refused(Refusal::Transient);
            }
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => return Sent::Dead,
        }
        answer.await.unwrap_or(Sent::Dead)
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

    /// Exit cleanly: send the game's exit command and wait for the server to
    /// close the connection (`plan/16` §5b).
    ///
    /// # This is a request, not a kill
    ///
    /// It returns when the session has said goodbye, so a caller that wants a
    /// clean exit **awaits this and then cancels**, rather than cancelling. A
    /// bare cancel is still correct for an abrupt stop -- criterion 6 holds
    /// either way -- but it leaves the game seeing a dropped link rather than a
    /// logout.
    ///
    /// The `quit` is also a **signal to the client layer, not only the
    /// server**. In Lich it triggers the whole save sequence, and the author's
    /// reason for wanting it is that one: *"it sends the signal to lich to
    /// shutdown so it has time to save everything without corruption"*. Cena
    /// has no separate proxy, so the save and the send are in one process --
    /// but the ordering obligation is inherited, and §5b.2 is explicit that
    /// saving happens **before** the connection closes.
    ///
    /// Returns [`Farewell::Unsent`] if the session is already gone, which is
    /// not an error: quitting a dead session has nothing to send.
    pub async fn quit(&self, timeout: std::time::Duration) -> Farewell {
        let (reply, answer) = oneshot::channel();
        if self
            .sender
            .send(Inbox::Quit { timeout, reply })
            .await
            .is_err()
        {
            return Farewell::Unsent;
        }
        answer.await.unwrap_or(Farewell::Unsent)
    }
}
