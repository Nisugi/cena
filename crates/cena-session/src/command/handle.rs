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

/// How long to wait for the **actor** to answer a message that it answers in
/// the turn it receives.
///
/// Not a game deadline: a live actor replies in microseconds. It bounds the
/// cases where nothing answers in time: an actor stalled in a write (up to
/// `WRITE_DEADLINE`), or a message that lands in the instant between one
/// connection ending and the supervisor's next wait. Without it `send_now`
/// and `claim` could wait forever, and `look` awaits `claim` outside its own
/// cancel select, so `stop` could not stop a behavior.
///
/// It used to be the ONLY answer during a reconnect, because nothing read the
/// inbox while the supervisor climbed its ladder. The supervisor now answers
/// the inbox during its connect and backoff waits (review finding 6), so this
/// is a backstop in the strict sense.
///
/// Five seconds is far longer than any live reply and far shorter than an
/// outage, which is what makes it a backstop rather than a policy.
const ACTOR_REPLY_DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);
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
    /// Whether the command's report is kept out of the story
    /// ([`SessionHandle::send_quietly`]).
    pub quiet: bool,
    /// The last check the session makes as the command goes out
    /// ([`SessionHandle::send_gated`]).
    pub gate: super::verdict::Gate,
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
/// literally criterion 3 (`plan/12:538`).
#[derive(Clone, Debug)]
pub struct SessionHandle {
    pub(super) sender: tokio::sync::mpsc::Sender<Inbox>,
    /// **Shared, not copied.** A handle cloned before a reconnect must keep
    /// working afterwards; see [`GenerationCell`] for why that does not weaken
    /// `plan/12` §4.4's discard rule.
    pub(super) generation: crate::lifecycle::GenerationCell,
    /// Where [`Self::say`] publishes. The session's own publisher, which
    /// lives as long as the session does, reconnects included -- so a notice
    /// reaches the fenced observer stream as well as the legacy one.
    events: crate::observation::EventPublisher,
    /// The player log, once one is attached. Shared with every clone.
    log: crate::player_log::tap::Slot,
    /// Who runs the player's own commands, once anything does. Shared
    /// with every clone, as the log is.
    desk: super::claimant::Slot,
    /// What a person has done through this handle (`attendance.rs`).
    pub(super) attendance: super::attendance::Attendance,
    /// The session's command authority (`authority.rs`).
    pub(super) authority: super::authority::Authority,
}

impl SessionHandle {
    /// Wrap a sender and a bare event channel: a handle with no session
    /// behind it, for tests. Notices reach `events` and nothing else.
    #[must_use]
    pub fn new(
        sender: tokio::sync::mpsc::Sender<Inbox>,
        generation: crate::lifecycle::GenerationCell,
        events: tokio::sync::broadcast::Sender<crate::Event>,
    ) -> Self {
        let events = crate::observation::EventPublisher::from_legacy(events, generation.clone());
        Self::publishing_to(sender, generation, events)
    }

    /// The handle a session builds: notices go through the session's own
    /// publisher, so they are numbered and fenced like every other event.
    ///
    /// Review finding 7: sessions built their handle with `new` and the raw
    /// legacy sender, so a frontend on the fenced stream
    /// (`SessionObserver::subscribe`) never saw a notice.
    pub(crate) fn publishing_to(
        sender: tokio::sync::mpsc::Sender<Inbox>,
        generation: crate::lifecycle::GenerationCell,
        events: crate::observation::EventPublisher,
    ) -> Self {
        Self {
            sender,
            generation,
            events,
            log: crate::player_log::tap::Slot::default(),
            desk: super::claimant::Slot::default(),
            attendance: super::attendance::Attendance::default(),
            authority: super::authority::Authority::default(),
        }
    }

    /// The session's authority cell, for its queues and its supervisor.
    pub(crate) fn authority_cell(&self) -> super::authority::Authority {
        self.authority.clone()
    }

    /// What a person has done through this session, for its supervisor.
    pub(crate) fn attendance(&self) -> super::attendance::Attendance {
        self.attendance.clone()
    }

    /// The slot this handle and all its clones read the player log from.
    pub(crate) fn log_slot(&self) -> crate::player_log::tap::Slot {
        std::sync::Arc::clone(&self.log)
    }

    /// Register who runs the player's typed commands, and with what
    /// symbol (`super::claimant`). Once per session: a second call is
    /// ignored and answers `false`.
    #[must_use]
    pub fn set_desk(&self, desk: super::claimant::Desk) -> bool {
        self.desk.set(desk).is_ok()
    }

    /// Change the command symbol of the desk already registered; `false` if
    /// none is. See `Desk::set_symbol`.
    #[must_use]
    pub fn set_command_symbol(&self, symbol: char) -> bool {
        self.desk
            .get()
            .map(|desk| desk.set_symbol(symbol))
            .is_some()
    }

    /// Which session this handle reaches: the id every event and snapshot of
    /// that session names. A frontend serving several keys its views by it.
    #[must_use]
    pub fn session(&self) -> crate::lifecycle::SessionId {
        self.events.session()
    }

    /// What this session marks a command with, if anything runs them.
    #[must_use]
    pub fn command_symbol(&self) -> Option<char> {
        self.desk.get().map(super::claimant::Desk::symbol)
    }

    /// Give a typed line to whoever runs commands (`super::claimant`).
    /// `None`: it is the
    /// game's, and the caller sends it as it always has.
    ///
    /// **Every manual path asks this first** ([`Self::send_manual_at`]),
    /// so a frontend gets the player's commands without knowing what any
    /// of them are -- and one that sends by another route does not
    /// silently lose them.
    #[must_use]
    pub fn typed(&self, line: &str) -> Option<super::Claimed> {
        self.desk.get()?.claim(line)
    }

    /// Say something to the player (`crate::notice`).
    ///
    /// **Not a command, and not through the command inbox.** That channel is
    /// bounded and is the game's: a behavior reporting why it stopped must
    /// not be refused because the queue it was filling is full, and must not
    /// take a slot a `release` needs. It is published straight to the event
    /// streams every frontend already reads -- the legacy one and the fenced
    /// one, through the session's single publication point.
    ///
    /// It cannot fail in a way the caller could act on: with nobody
    /// listening there is nobody to tell.
    pub fn say(&self, notice: crate::notice::Notice) {
        // Logged HERE because a notice never passes through the actor. Before
        // the send, which takes the notice by value.
        if let Some(log) = self.log.get() {
            log.notice(self.generation(), &notice);
        }
        let _ = self.events.send(crate::Event::Notice(notice));
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
    /// party that may never answer. This waits on the *actor*, which answers in
    /// the same turn it receives the message.
    ///
    /// # That reasoning was true until the supervisor existed
    ///
    /// It used to end "a timeout here would be a deadline on a wait that cannot
    /// hang", on the grounds that an absent actor drops the sender and resolves
    /// immediately as [`Sent::Dead`]. **A supervised session breaks the
    /// premise**: between generations there is no actor, and the sender is very
    /// much alive because the *supervisor* holds the receiver. Nothing read the
    /// inbox during a retry ladder, so the wait was unbounded -- and a message
    /// parked there was delivered to the NEXT connection, long after its caller
    /// gave up.
    ///
    /// So it is bounded by `ACTOR_REPLY_DEADLINE`, which is generous by design:
    /// it is not a game timeout, it is a backstop for "nobody is home". The
    /// supervisor now answers `Dead` during its waits itself (review finding
    /// 6), so the backstop is for an actor stalled in a write.
    ///
    /// # Errors
    ///
    /// Never. Like `send_and_await`, the failure modes are values.
    pub async fn send_now(&self, line: &str, origin: Origin, gate: Gate) -> Sent {
        if origin == Origin::Manual {
            self.attendance.mark();
        }
        let (reply, answer) = oneshot::channel();
        let message = Inbox::SendNow {
            line: line.to_owned(),
            origin,
            generation: self.generation.get(),
            gate,
            reply,
        };
        if !self.has_room_for_traffic() {
            return Sent::Refused(Refusal::Transient);
        }
        match self.sender.try_send(message) {
            Ok(()) => {}
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                return Sent::Refused(Refusal::Transient);
            }
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => return Sent::Dead,
        }
        // Bounded: see `ACTOR_REPLY_DEADLINE`. `Dead` for a timeout is the
        // honest answer for a message the actor has not reached: it checks
        // `reply.is_closed()` before writing, so a message whose caller gave
        // up here is dropped rather than written late. That check was missing
        // when this comment was first written, which made it false for a
        // message queued behind a slow write (review finding 4).
        //
        // It is NOT a guarantee for a message whose write had already begun:
        // a write can take up to `WRITE_DEADLINE`, as long as this wait, and
        // bytes in flight cannot be recalled. That residue is one message, the
        // one being written when the wait expired.
        tokio::time::timeout(ACTOR_REPLY_DEADLINE, answer)
            .await
            .unwrap_or(Ok(Sent::Dead))
            .unwrap_or(Sent::Dead)
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
        if !self.has_room_for_traffic() {
            return Err(crate::queue::AuthorityHeld(token));
        }
        if self.sender.try_send(Inbox::Claim { token, reply }).is_err() {
            return Err(crate::queue::AuthorityHeld(token));
        }
        // Bounded for the same reason `send_now` is: between generations
        // nothing reads the inbox, and `look` awaits this OUTSIDE its cancel
        // select (`look.rs:155`), so an unbounded wait here made `stop` unable
        // to stop a behavior during an outage.
        tokio::time::timeout(ACTOR_REPLY_DEADLINE, answer)
            .await
            .unwrap_or(Ok(Err(crate::queue::AuthorityHeld(token))))
            .unwrap_or(Err(crate::queue::AuthorityHeld(token)))
    }

    /// Give the authority back. Ignored if this token does not hold it.
    ///
    /// Fire-and-forget: a release has no answer worth waiting for, and a
    /// behavior releasing during cleanup must not block. §4.3 is explicit that
    /// cleanup "cannot send commands" -- this is not a command.
    pub fn release(&self, token: crate::queue::AuthorityToken) {
        // **Uses the RESERVED slot.** Every other producer stops one short of
        // capacity ([`Self::has_room_for_traffic`]), so the final slot is
        // reachable only from here.
        //
        // # Why a release that is dropped is not a small bug
        //
        // MEASURED: 60 concurrent `send_and_await` calls with no scheduler
        // yield between them refused 28 -- the inbox genuinely fills, because
        // the actor takes **one message per loop turn** and a turn can now
        // spend up to `WRITE_DEADLINE` in a single write. A release lost in
        // that window leaves the token holding authority **for the rest of the
        // session**: every later behavior gets `AuthorityHeld`, with no
        // recovery path and no error recorded anywhere.
        //
        // # Why not `send().await`
        //
        // `plan/12` §4.3 is explicit that cleanup "cannot send commands" and
        // must not block, and a behavior being cancelled has a 250ms grace
        // budget. Awaiting a full queue during cleanup would trade a stuck
        // token for a stuck shutdown -- and it would do so exactly when the
        // session is already struggling, which is when the queue is full.
        //
        // # Why one slot is enough
        //
        // `CommandQueue::release` ignores a token that does not hold the
        // authority (`queue.rs:143-147`), so it is idempotent: a second release
        // arriving before the first drains is either the same token (a no-op)
        // or a non-holder (ignored). One slot cannot be exhausted by a caller
        // that behaves, and cannot be abused by one that does not.
        let _ = self.release_reached_the_channel(token);
    }

    /// [`Self::release`], reporting whether the channel accepted it.
    ///
    /// **Exists for the test**, and says so rather than pretending to be a
    /// general API: `release` returns `()` because §4.3's cleanup has nothing
    /// useful to do with a failure, but a test asserting the reserved slot
    /// works needs to see the send succeed. Making the ordinary path return a
    /// value nobody checks would be worse -- a `#[must_use]` nobody can act on.
    ///
    /// Returns `false` only if the inbox is genuinely full past its reserve, or
    /// the session is gone.
    #[must_use]
    pub fn release_reached_the_channel(&self, token: crate::queue::AuthorityToken) -> bool {
        self.sender.try_send(Inbox::Release(token)).is_ok()
    }

    /// Whether the inbox has room for **ordinary traffic**, keeping one slot
    /// back for [`Self::release`].
    ///
    /// # Why the reserve is enforced here and not by tokio
    ///
    /// `Sender::try_reserve` takes a permit from the same capacity, at the
    /// moment of need -- which is exactly when the channel is full, so it
    /// cannot reserve *for* a later release. `OwnedPermit` can be held across a
    /// behavior's lifetime but **consumes the `Sender`**, and `SessionHandle`
    /// is `Clone` and shared by every caller.
    ///
    /// So the reservation is ours: producers of ordinary traffic stop one short
    /// of capacity, and the last slot is reachable only from `release`. That
    /// keeps `plan/12` §5.5's "bounded channels everywhere" -- the bound is
    /// unchanged -- and costs one slot of throughput.
    /// Send one ordinary-traffic message through the reserve gate, reporting
    /// whether it was accepted. **For the reserve's test**, which needs the
    /// producer-side arithmetic without an actor draining behind it.
    #[doc(hidden)]
    #[must_use]
    pub fn try_send_traffic_for_test(&self) -> bool {
        if !self.has_room_for_traffic() {
            return false;
        }
        self.sender
            .try_send(Inbox::Release(crate::queue::AuthorityToken(u64::MAX)))
            .is_ok()
    }

    /// Slots still free in the inbox. `#[doc(hidden)]`: an assertion aid, not a
    /// number a caller should branch on.
    #[doc(hidden)]
    #[must_use]
    pub fn capacity_for_debug(&self) -> usize {
        self.sender.capacity()
    }

    pub(super) fn has_room_for_traffic(&self) -> bool {
        // `capacity()` is the number of slots still free.
        self.sender.capacity() > 1
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
    /// # `timeout` bounds the WHOLE operation
    ///
    /// **It did not, and that was a hang.** It was passed to the actor and
    /// started only once an actor received the message, which left two
    /// unbounded waits in front of it:
    ///
    /// * `send(..).await` blocks while the inbox is full;
    /// * `answer.await` had no deadline of its own.
    ///
    /// Neither matters while a connection is up. **During a reconnect there is
    /// no actor at all** -- the supervisor is climbing its retry ladder and is
    /// not reading the inbox -- so a quit issued then waited forever, and
    /// `main` awaits the quit *before* cancelling. An ordinary shutdown during
    /// a network outage wedged the client.
    ///
    /// So the deadline is applied here, around everything, as well as being
    /// passed to the actor for its own EOF wait. A caller gets an answer within
    /// `timeout` whatever the session is doing.
    pub async fn quit(&self, timeout: std::time::Duration) -> Farewell {
        // **The outer bound is deliberately LOOSER than the inner one.** Equal
        // deadlines made this outer timeout win the race against the actor's
        // own, and every unanswered quit came back `Unsent` -- destroying the
        // distinction §5b.3 exists for: `Unsent` means the game was never told,
        // `TimedOut` means it was told and may have acted on it. A test caught
        // it, and collapsing the two would have been a lie in the log.
        //
        // Doubling gives the actor room to report its own verdict first. This
        // outer bound is the backstop for the case where NOTHING is listening,
        // not a second opinion on a live connection.
        let backstop = timeout.saturating_mul(2);
        match tokio::time::timeout(backstop, self.quit_inner(timeout)).await {
            Ok(farewell) => farewell,
            // Nobody took the message, or nobody answered it. Either way the
            // caller is free to cancel -- which is what makes this the safe
            // answer rather than a lie: `Unsent` means "assume the game was not
            // told", and during a reconnect it genuinely was not.
            Err(_elapsed) => Farewell::Unsent,
        }
    }

    async fn quit_inner(&self, timeout: std::time::Duration) -> Farewell {
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
