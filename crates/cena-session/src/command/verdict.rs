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
    /// **Not "the action fired", and not a receipt.** It is the *start* of a
    /// verification the caller still owes (`plan/16` §1.1b):
    ///
    /// > **AUTHOR, 2026-09-18:** *"I said fire and forget when I meant fire and
    /// > verify later, don't wait for verification then."*
    ///
    /// So every caller needs a named place the answer will appear:
    ///
    /// | Command | Verified by |
    /// |---|---|
    /// | a sigil, an instant cast | an [`Effects`](cena_model::Effects) id lookup (§2) |
    /// | a movement | the next room frame |
    /// | `look`, `assess` | the text it returns |
    ///
    /// The effects case is the one §2 was built on and the capture of
    /// 2026-09-18 showed is reliable: casting `515` put `Rapid Fire` in `Buffs`
    /// under exactly that id. **Movement is what shows it is not the only
    /// case** -- the mechanism is "check somewhere specific, later", and the
    /// *somewhere* varies by command.
    ///
    /// **A `send_now` with no such place is a bug.** A caller that cannot say
    /// where it will check is not deferring verification; it is one that never
    /// verifies.
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

/// Whether a command is gated on roundtime.
///
/// # Why this is not a `bool`
///
/// Because the caller is asserting something about the game, and a bare `true`
/// at the call site does not say what.
///
/// > **AUTHOR:** *"they can't be activated while in roundtime"* -- but also
/// > *"shouldn't be subject to typeahead or waiting (**depending on the
/// > action**)"*.
///
/// # The definition, in one line
///
/// > **AUTHOR, 2026-09-18:** *"anything that doesn't cause roundtime would be
/// > an instant action."*
///
/// **That is the whole rule, and it is a property rather than a list.** An
/// instant action is not a curated set of abilities to enumerate -- it is any
/// command that incurs no roundtime of its own. `plan/16` §1.2's seed list
/// (515, 140, the Sunfist sigils) is therefore a set of *examples*, never the
/// definition, and §1.2's note that "this list grows" understates it: there is
/// nothing to grow, because membership is decided by what the command does.
///
/// Two earlier drafts of these docs got this wrong in opposite directions.
/// The first called the exempt set **UNVERIFIED** and said "today every caller
/// passes [`Self::Roundtime`]". The second, on learning that `look <target>`
/// and `assess <target>` qualify, split it into "commands that act" versus
/// "commands that look" -- closer, but still a taxonomy where a property was
/// wanted. Observation is exempt *because* it causes no roundtime, not because
/// it is observation.
///
/// MEASURED 2026-09-18 (`plan/16` §1.5): a `look` sent **inside a 7-second
/// roundtime** executed normally -- full room render, no `...wait N`, no
/// refusal. The server does not gate what does not cost roundtime, and a
/// client that did would refuse commands the game would have run.
///
/// # This is NOT the same question as "when do we verify"
///
/// > **AUTHOR, 2026-09-18:** *"there is a difference between an instant action
/// > as in something we fire and forget and something that's instant and we
/// > care about the answer such as moving."* -- then, correcting it: *"I said
/// > fire and forget when I meant fire and verify later, don't wait for
/// > verification then."*
///
/// **Nothing is fire-and-forget.** The distinction is *when* the answer is
/// checked, never *whether*:
///
/// | | fire, verify later | wait for the answer |
/// |---|---|---|
/// | **causes no roundtime** | a sigil (check `Effects`) | **a movement** |
/// | **causes roundtime** | -- | an attack, a cast |
///
/// Both columns verify. The left one just does not **block** on it: a sigil is
/// sent, the next command follows immediately, and the effect is confirmed
/// afterwards by an id lookup (`plan/16` §2). That is what makes batching
/// possible -- not an absence of checking, but the checking being deferred.
///
/// This type answers only the **rows**: may it be sent while a roundtime is
/// running. Which *column* a command is in is a separate decision, made by
/// picking [`SessionHandle::send_now`](crate::SessionHandle::send_now) or
/// [`send_and_await`](crate::SessionHandle::send_and_await).
///
/// The top-right cell is why both exist. A movement is instant -- no roundtime
/// to wait out, so `Gate::None` is correct -- but the client cannot go on
/// without knowing the room changed, so it waits. `plan/16` §5.2g: a cardinal
/// run batches and verifies after, a `StringProc` door waits for its replies.
///
/// **The design consequence is that [`Sent`] is not a receipt.** It says the
/// bytes went out; it is the *start* of a verification the caller still owes,
/// not the end of one. A caller in the left column must have somewhere to
/// check later -- `Effects` for a sigil, the room for a move -- and a caller
/// that has nowhere to check is not fire-and-verify-later, it is a caller that
/// never verifies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    /// Refuse while in roundtime. For any command that **causes** roundtime:
    /// attacks, casts, most abilities.
    Roundtime,
    /// Send regardless. For any command that causes **no** roundtime -- the
    /// author's definition of an instant action.
    ///
    /// Sigils, Rapid Fire, Wall of Force; and equally `look`, `look <target>`,
    /// `assess <target>`, which cost nothing and which the server MEASURED as
    /// running during a roundtime.
    None,
}
