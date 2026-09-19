//! The command vocabulary: where a command came from, and what came back.
//!
//! [`Origin`], [`Refusal`], [`Outcome`], [`Sent`], [`Gate`] and [`CommandId`].
//! These are the **types a behavior reasons in** -- the answers to "who asked
//! for this" and "what happened" -- as against [`super`]'s transport, which is
//! how a command gets from a caller to the actor.
//!
//! Split out of `command.rs` under Rule 4.1 (`plan/05:352-353`) -- **move code
//! down, do not raise the cap.** That file reached 473 lines against the 400
//! default when `send_now` landed, and this seam was the obvious one: almost
//! every line added was vocabulary ([`Sent`], [`Gate`]) rather than transport.
//!
//! The next split, if this grows again: [`Outcome`] and [`Sent`] to
//! `command/verdict/result.rs`, leaving [`Origin`] and [`Gate`] -- the two
//! types a CALLER passes in -- here.

use cena_protocol::Frame;

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
    /// A script sent it -- **including a command from another character's
    /// session** (`plan/16` §5a).
    ///
    /// # Why this variant exists before any script does
    ///
    /// Not because scripting is decided -- it is **not** (`plan/16` §5a). It
    /// exists because `Origin` is matched on wherever the interleaving policy
    /// is decided, and every one of those matches is a statement about how a
    /// script command behaves relative to a behavior's. Answering that with
    /// **one** behavior in the tree is a line each; answering it once Hunt,
    /// Heal and Travel exist means re-deciding the policy for all of them,
    /// each written assuming it was the only claimant.
    ///
    /// So the cost of adding it now is a variant. The cost of adding it later
    /// is a policy migration.
    ///
    /// # It behaves like `Manual`, deliberately
    ///
    /// > **AUTHOR, 2026-09-18:** a character receiving a cross-character
    /// > command should perform it *"as if they just sent it."*
    ///
    /// So it jumps the queue and does **not** preempt the recipient's own
    /// behavior -- `plan/12` §4.1's rule, unchanged. It is a separate variant
    /// from [`Self::Manual`] not because it queues differently but because a
    /// **log has to be able to tell them apart**: "the player typed this" and
    /// "another character's script sent this" are different facts about a
    /// session, and collapsing them makes a transcript unreadable at exactly
    /// the moment it matters.
    ///
    /// # It carries no authority token
    ///
    /// Like `Manual`, and for the same reason: §4.1's correction is that input
    /// is not a claimant. A script that wants to run a *sequence* claims the
    /// authority and sends as [`Self::Behavior`]; this variant is for
    /// individual commands.
    Script,
}

impl Origin {
    /// The claimant's token, if a behavior sent this.
    #[must_use]
    pub const fn token(self) -> Option<crate::queue::AuthorityToken> {
        match self {
            // Neither the player nor a script is a claimant (§4.1).
            Self::Manual | Self::Script => None,
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

/// What a [`SessionHandle::send_now`] did.
///
/// # Why this is not [`Outcome`]
///
/// Because they answer different questions, and one type answering both would
/// make [`Outcome::Confirmed`] mean two things. `Outcome` is the verdict on a
/// **round trip**: `Confirmed` carries the frame from the wire that matched
/// the caller's matcher. `send_now` opens no window, matches no frame and
/// waits for nothing, so it has no such frame -- and an earlier draft of this
/// fabricated a `Frame::Prompt` to put in one, which would have put a frame
/// that never crossed the wire into a value whose documented meaning is "the
/// frame that matched".
///
/// The evidence that actually matters is different too: not *what answered*,
/// but *what the gate was decided on*.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sent {
    /// The bytes reached the wire.
    ///
    /// **Not "the action fired."** That is confirmed by its effect
    /// (`plan/16` §2) -- an [`Effects`](cena_model::Effects) lookup on the
    /// spell id, which the capture of 2026-09-18 showed is reliable: casting
    /// `515` put `Rapid Fire` in `Buffs` under exactly that id.
    Ok {
        /// The server second the roundtime gate was decided against, when one
        /// ran. `None` under [`Gate::None`], where no clock is consulted.
        ///
        /// Here so a log can show what the decision was made on. A `send_now`
        /// that went out against a five-second-stale clock and a `send_now`
        /// against a fresh one are different events, and without this they are
        /// the same line.
        at: Option<u32>,
    },
    /// It did not go out, and why.
    Refused(Refusal),
    /// The session is gone, or the write failed.
    Dead,
    /// From a previous connection (`plan/12` §5.2).
    Interrupted,
}

/// Whether an instant action is gated on roundtime.
///
/// # Why this is not a `bool`
///
/// Because the caller is asserting something about the game, and a bare
/// `true` at the call site does not say what. `plan/16` §1.1 has two rules and
/// this names which one applies:
///
/// > **AUTHOR:** *"they can't be activated while in roundtime"* -- but also
/// > *"shouldn't be subject to typeahead or waiting (**depending on the
/// > action**)"*.
///
/// That parenthesis is why there are two variants. Which actions are exempt is
/// **UNVERIFIED** (`plan/16` §8, question 1), so the type carries the question
/// rather than a guess: today every caller passes [`Self::Roundtime`], and the
/// day one does not, it says so at the call site instead of flipping a
/// boolean.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    /// Refuse while in roundtime. The default for everything measured so far.
    Roundtime,
    /// Send regardless. For an action established not to be roundtime-gated.
    None,
}
