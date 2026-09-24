//! [`SessionCore`]: everything that survives a reconnect.
//!
//! **The boundary of this struct IS `plan/12` §5.2's contract, expressed as
//! ownership.** What lives here spans generations; what the actor owns does
//! not. That makes the classification structural rather than a rule someone has
//! to remember — a field put in the wrong place is a bug you can see.
//!
//! # What is deliberately NOT here
//!
//! **`Parser`.** Its `pending` buffer holds an unterminated trailing fragment,
//! and Lich clears the equivalent on reconnect so that *"a fragment left open
//! before a reconnect/session reset does not bleed into the next session"*
//! (`reference/lich-5/lib/games.rb:432`). A connection dropped mid-tag would
//! otherwise splice its fragment onto the next generation's first read — one
//! corrupt frame per reconnect, visible only on a real mid-tag drop.
//!
//! Cena needs no `Parser::reset()` for this: **a new connection gets a new
//! actor, and a new actor gets `Parser::new()`**, which resets `pending` along
//! with every other per-connection field at once. The absence of a field is the
//! design decision here, which is why it is written down — an absent field with
//! no comment reads as an oversight.
//!
//! **`CommandQueue`.** Same reasoning: a queue holds waiters for one
//! connection, and they are answered by `shutdown` before the actor returns.
//! Carrying them forward would deliver a previous connection's answers.

use crate::State;
use crate::command::{Farewell, Inbox, Outcome, Sent};
use crate::lifecycle::GenerationCell;
use crate::observation::{EventPublisher, ObservationRequests};
use cena_model::GameState;
use cena_platform::{Recorder, SessionSink};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// The parts of a session that outlive any one connection.
#[derive(Debug)]
pub struct SessionCore {
    /// The one command channel.
    ///
    /// Durable so that a [`SessionHandle`](crate::SessionHandle) cloned in
    /// generation 0 still reaches the actor in generation 3. A fresh channel
    /// per connection would invalidate every handle a caller holds.
    pub(super) commands: mpsc::Receiver<Inbox>,
    /// The event stream.
    ///
    /// Durable so a `broadcast::Receiver` taken in generation 0 keeps
    /// receiving. `VellumFE` does exactly this on reconnect — it builds a new
    /// command channel and **keeps the existing `server_rx`**
    /// (`reference/VellumFE/src/frontend/tui/runtime.rs:546-600`). Cena keeps
    /// both, because Cena's handle is durable where Vellum's is not.
    pub(super) events: EventPublisher,
    pub(super) observations: ObservationRequests,
    pub(super) lifecycle: State,
    /// Which session this is.
    ///
    /// The most durable thing here: a session keeps its id for its whole life,
    /// where `generation` advances on every reconnect. `(id, generation)` names
    /// one connection of one character.
    pub(super) id: crate::lifecycle::SessionId,
    /// What the session knows.
    ///
    /// Carried across and then **invalidated** between generations
    /// ([`GameState::invalidate_for_reconnect`]), never dropped: §5.2's
    /// Retained class is most of it, and the login burst re-sends the rest.
    pub(super) state: GameState,
    /// Everything that crossed the wire, across every generation.
    ///
    /// Durable because criterion 9 is "verified in the replay", which requires
    /// the reconnect itself to be *in* the recording. A per-connection recorder
    /// could not express one.
    pub(super) recorder: Recorder,
    /// Where this session's wire traffic is written, if anywhere. Durable so a
    /// log spans the reconnect rather than restarting at it.
    pub(super) sink: Option<SessionSink>,
    /// Cloned into each connection's actor, so one hunt spans a reconnect.
    pub(super) combat: Option<crate::combat_recorder::worker::RecorderHandle>,
    /// Cloned into each connection's actor, so one log spans a reconnect.
    /// The handle's own slot, so `say` and the actor write to one log.
    pub(super) player_log: crate::player_log::tap::Slot,
    /// Where the character's facts are stored, if anywhere. Given to each
    /// connection's actor, which does the reading and writing.
    pub(super) character_dir: Option<std::path::PathBuf>,
    /// Where `<cmdlist>` pushes are stored, if anywhere.
    pub(super) menu_dir: Option<std::path::PathBuf>,
    /// The shared generation every handle reads.
    pub(super) generation: GenerationCell,
    /// What a person has done through this session's handles
    /// (`command/attendance.rs`).
    pub(super) attendance: crate::command::attendance::Attendance,
    /// How much of [`Self::attendance`] `after_connection` has already
    /// counted, so what a person did between two losses is told apart.
    pub(super) attendance_seen: u64,
    /// The session's command authority, which every connection's queue
    /// shares (`command/authority.rs`, SE-4).
    pub(super) authority: crate::command::authority::Authority,
    /// Stops the **session**, not one connection.
    ///
    /// This is the distinction that makes a supervisor possible: the actor's
    /// own cancel ends its connection, and cancelling this ends the whole
    /// supervised session including any reconnect it was about to attempt.
    pub(super) cancel: CancellationToken,
}

impl SessionCore {
    /// Answer and discard every message parked since the last connection ended.
    ///
    /// # The defect this closes
    ///
    /// [`commands`](Self::commands) is durable **on purpose** -- a handle taken
    /// in generation 0 must still reach the actor in generation 3. The cost is
    /// that between one `actor.run()` returning and the next starting, the
    /// receiver sits un-polled while handles keep sending successfully. Those
    /// messages were addressed to a connection that no longer exists, and the
    /// next actor used to drain them as if they were fresh.
    ///
    /// The worst case is `quit`. Its caller's wait is bounded, so a `quit`
    /// issued during a reconnect answers `Unsent` -- documented as *"assume the
    /// game was not told"*. The `Inbox::Quit` stayed in the channel anyway, and
    /// **the next successful login was followed immediately by a logout**.
    /// VERIFIED before the fix by the recorder's own transcript: `<prompt>`,
    /// then `quit\n`, then the next generation's `<prompt>`.
    ///
    /// Review finding SE-1, reported HIGH. `plan/12` §5.1 already said what
    /// should happen -- commands during `Reconnecting` *"fail immediately with
    /// `Disconnected`"* -- so this implements a stated rule rather than
    /// inventing one.
    ///
    /// # Why each message is ANSWERED rather than dropped
    ///
    /// A dropped `oneshot::Sender` resolves the caller's `await` to an error,
    /// which every call site already maps to a pessimistic default. Answering
    /// explicitly says the same thing in the vocabulary the caller reads, and
    /// keeps a dropped sender meaning "the actor is gone" rather than "this was
    /// swept".
    ///
    /// # Ordering
    ///
    /// Called immediately before the receiver is handed to a new actor, so the
    /// window it clears is exactly "since the last connection ended". Anything
    /// arriving after this point belongs to the new connection and is handled
    /// normally.
    /// # A person's swept command still counts as attendance
    ///
    /// It was counted at the handle when it was typed
    /// (`command/attendance.rs`), so sweeping it here loses nothing: a player
    /// typing DURING a reconnect is never an empty chair. A behavior's swept
    /// command is not a person, and is never counted.
    ///
    /// # Discarding a command is NOT evidence that nobody is there
    ///
    /// `io.rs`'s `write_bounded` answers the idle warning **before** the write,
    /// and says why: *"a write that fails still means someone tried, and the
    /// session is attended either way. The supervisor's question is 'is anyone
    /// here', not 'did the packet land'."*
    ///
    /// A command parked during a reconnect is that case exactly -- somebody
    /// typed while the client was reconnecting -- so sweeping it answers the
    /// idle warning too. Found by `idle_kick.rs`'s attended-session test going
    /// red on the first version of this sweep: discarding the command erased
    /// the only proof a human was present, and the supervisor then stopped an
    /// attended session as idle. **That failure was the fix telling me it was
    /// half-written.**
    pub(super) fn discard_stale_inbox(&mut self) -> usize {
        let mut swept = 0;
        while let Ok(message) = self.commands.try_recv() {
            swept += 1;
            self.refuse_while_disconnected(message);
        }
        swept
    }

    /// Answer one message that arrived while there is no connection.
    ///
    /// # Answered as it arrives, not when the next connection does
    ///
    /// `plan/12` §5.1 says commands during `Reconnecting` *"fail immediately
    /// with `Disconnected`"*, and `State::Reconnecting`'s docs claimed they
    /// did. They did not: the inbox was swept only after a connect SUCCEEDED,
    /// so with the network down a `send_and_await` waited out its own deadline
    /// and came back `Timeout` -- 30 s in the review's probe, and `Timeout`
    /// tells a behavior the command may have run -- while `send_now` hit its
    /// 5 s backstop. The supervisor now calls this from its connect and
    /// backoff waits as well as from the sweep (review finding 6).
    ///
    /// A person's command was already counted as attendance where it was
    /// typed -- see [`Self::discard_stale_inbox`].
    pub(super) fn refuse_while_disconnected(&mut self, message: Inbox) {
        if matches!(
            message,
            Inbox::Command(_) | Inbox::SendNow { .. } | Inbox::Quit { .. }
        ) {
            self.state.answer_idle_warning();
        }
        match message {
            // §5.1: a command aimed at a dead connection fails with
            // `Disconnected`, which a behavior reads as "this connection
            // died and a retry may work" -- not `Dead`, which is permanent.
            Inbox::Command(envelope) => {
                let _ = envelope.reply.send(Outcome::Disconnected);
            }
            Inbox::SendNow { reply, .. } => {
                let _ = reply.send(Sent::Dead);
            }
            // **The one that logs the character out.** The caller has
            // already been told `Unsent`; this makes that true.
            Inbox::Quit { reply, .. } => {
                let _ = reply.send(Farewell::Unsent);
            }
            // The authority is the SESSION's (`command/authority.rs`, SE-4),
            // so a claim or a release between connections is answered as one
            // during a connection would be, and holds for the next. Rolled
            // back if nobody hears the grant, as the actor does.
            Inbox::Claim { token, reply } => {
                let outcome = self.authority.claim(token);
                let granted = outcome.is_ok();
                if reply.send(outcome).is_err() && granted {
                    self.authority.release(token);
                }
            }
            Inbox::Release(token) => self.authority.release(token),
        }
    }
}
