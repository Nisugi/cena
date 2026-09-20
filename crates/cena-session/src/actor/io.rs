//! The actor's I/O: sending the queue to the wire, and folding what comes
//! back.
//!
//! Split out of `actor.rs` under Rule 4.1 (`plan/05:352-353`) -- **move code
//! down, do not raise the cap.** That file went to 424 lines against the 400
//! default when the readiness gate landed, and its module header had already
//! named this split in advance: "if this file grows, `run_command` and
//! `ingest` move to `actor/io.rs` and the loop stays." This is that split,
//! taken as written.
//!
//! Admission (`handle_inbox`/`admit`) moved here too, under the same rule,
//! when enforcing `plan/12` §4.2's authority took `actor.rs` to 408 lines.
//! It belongs beside the rest of one turn's work: the loop and the session's
//! shape stay in `actor.rs`, what a message DOES lives here.
//!
//! What stays in `actor.rs` is the select loop and the session's shape; what
//! moves here is the two halves of one turn -- bytes out ([`SessionActor::pump`])
//! and bytes in ([`SessionActor::ingest`]).

use super::{Envelope, Event, SessionActor, WRITE_DEADLINE};
use crate::command::{Origin, Outcome, Sent};
use cena_platform::ByteSource;
use cena_protocol::Frame;

/// What Cena sends to log out (`plan/16` §5b).
///
/// Lich recognises either `exit` or `quit`, optionally wrapped in `<c>`
/// (`reference/lich-5/lib/common/shutdown_intent.rb:7`:
/// `/\A\s*(?:<c>)?\s*(?:exit|quit)\s*\z/i`). `quit` is chosen because it is
/// the word the author used and the one a player types; `exit` is the same
/// thing to both Lich and the game.
const EXIT_COMMAND: &str = "quit";

/// How long a typed exit waits for the server's EOF.
///
/// The same bound `SessionHandle::quit` uses by default. A typed `quit` has no
/// caller holding a deadline -- the player is not awaiting a `Farewell` -- so
/// the actor supplies one rather than waiting unbounded, which `plan/12` §5.5
/// forbids.
const QUIT_EOF_DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);

/// Is this line the player asking to log out?
///
/// Lich's own test (`reference/lich-5/lib/common/shutdown_intent.rb:7`):
///
/// ```text
/// /\A\s*(?:<c>)?\s*(?:exit|quit)\s*\z/i
/// ```
///
/// Reproduced without a regex crate -- `cena-session` has no `regex`
/// dependency and one line of trimming is not worth acquiring one (Rule -1).
/// The `<c>` prefix is Lich's client-command wrapper, kept because a frontend
/// porting Lich's input path may pass it through.
fn is_exit_intent(line: &str) -> bool {
    let line = line.trim();
    let line = line.strip_prefix("<c>").unwrap_or(line).trim();
    line.eq_ignore_ascii_case("quit") || line.eq_ignore_ascii_case("exit")
}

impl<S: ByteSource> SessionActor<S> {
    /// Write one message, bounded by [`WRITE_DEADLINE`].
    ///
    /// Returns `false` if the write failed **or timed out**, which callers
    /// treat identically: both mean this connection can no longer be written
    /// to. A timeout is not recoverable here for the reason
    /// `WRITE_DEADLINE` records -- a partially written command has already
    /// broken the single-write rule, so the stream cannot be trusted.
    async fn write_bounded(&mut self, message: &[u8]) -> bool {
        // The character acted, which answers the server's idle warning. Here
        // rather than at the three call sites because **this is the one
        // chokepoint they all pass through** -- and a fix applied at one of three
        // send paths is a mistake already made once in this file (see
        // `handle_inbox`'s note on `WRITE_DEADLINE` being a half-measure).
        //
        // Before the write, not after: a write that fails still means someone
        // tried, and the session is attended either way. The supervisor's
        // question is "is anyone here", not "did the packet land".
        self.state.answer_idle_warning();
        matches!(
            tokio::time::timeout(WRITE_DEADLINE, self.source.write_all(message)).await,
            Ok(Ok(()))
        )
    }

    /// Route one inbox message.
    ///
    /// Claims and releases arrive on the SAME channel as commands (`plan/12`
    /// §4.2 is an ordering rule), so a claim cannot overtake a command already
    /// queued behind it.
    ///
    /// **`async` because of `SendNow` alone.** Every other arm is a queue
    /// operation that cannot block. `SendNow` writes to the socket in the turn
    /// it arrives -- that is the feature -- so routing became a wait. It is a
    /// short one: the same single `write_all` [`Self::pump`] does.
    /// Returns an [`EndReason`](super::EndReason) if handling the message made
    /// the connection unusable.
    ///
    /// **It used to return `()`, and that made `WRITE_DEADLINE` a half-measure.**
    /// `actor.rs`'s comment says a timed-out write "ENDS THE CONNECTION", and
    /// only `pump` did: `send_now` returned `Sent::Dead` and `begin_quit`
    /// returned `Farewell::Unsent`, and the loop carried on reading a stream the
    /// same comment calls untrustworthy. Found by review, against the commit
    /// that added the deadline.
    pub(super) async fn handle_inbox(
        &mut self,
        message: crate::command::Inbox,
    ) -> Option<super::EndReason> {
        match message {
            crate::command::Inbox::Command(envelope) => {
                // **A typed `quit` means what it says.**
                //
                // Sent as an ordinary command, `quit` reaches the game, the
                // server closes, and the actor reports `PeerClosed` -- which
                // `warrants_reconnect()` accepts, and which the sweep counts
                // as attendance because somebody typed. The supervisor then
                // logs the character straight back in, on every attempt to
                // leave (review SE-3).
                //
                // `begin_quit` is the same bytes with the session's own
                // bookkeeping: it arms the EOF deadline and marks `quitting`,
                // so the close is read as "the server closed because we
                // asked" rather than as a drop. `plan/16` §5b is precisely
                // that distinction.
                //
                // Latent today -- the binary has no typed-game-command
                // surface -- and live the day a frontend adds one, which is
                // when it would be hardest to diagnose.
                if is_exit_intent(&envelope.line) {
                    // The caller asked for a command outcome, and the honest
                    // one is `Disconnected`: §5.1 defines it as "this
                    // connection is ending", which is precisely what was just
                    // asked for. `Confirmed` would need a matching frame and
                    // there is none -- the server answers a quit by closing.
                    //
                    // The farewell goes nowhere because nobody is holding a
                    // `Farewell` receiver: this arrived as a command, not as
                    // `Inbox::Quit`.
                    let (tx, _rx) = tokio::sync::oneshot::channel();
                    let ok = self.begin_quit(QUIT_EOF_DEADLINE, tx).await;
                    let _ = envelope.reply.send(if ok {
                        Outcome::Disconnected
                    } else {
                        Outcome::Dead
                    });
                    if !ok {
                        return Some(super::EndReason::WriteFailed);
                    }
                } else {
                    self.admit(*envelope);
                }
            }
            crate::command::Inbox::Claim { token, reply } => {
                // **Roll back a claim nobody will hear about.** Found by
                // review: `claim` mutates and the send result was discarded,
                // so a caller that dropped its future between sending and
                // receiving left the token holding authority PERMANENTLY --
                // and every later claimant got `AuthorityHeld` naming a token
                // whose owner no longer exists.
                //
                // `oneshot::Sender::send` returns `Err` exactly when the
                // receiver is gone, which is the same condition, so the
                // rollback is the send's own error path rather than a
                // separate liveness check.
                //
                // Only a SUCCESSFUL claim is rolled back. An `AuthorityHeld`
                // reply that goes unheard changed nothing, and releasing on it
                // would take the authority away from whoever legitimately
                // holds it.
                let outcome = self.queue.claim(token);
                let granted = outcome.is_ok();
                if reply.send(outcome).is_err() && granted {
                    self.queue.release(token);
                }
            }
            crate::command::Inbox::Release(token) => self.queue.release(token),
            // Handled inline rather than queued: queueing it is the exact
            // defect `send_now` exists to remove (`plan/16` §1.3).
            crate::command::Inbox::SendNow {
                line,
                origin,
                generation,
                gate,
                reply,
            } => {
                let outcome = self.send_now(&line, origin, generation, gate).await;
                let _ = reply.send(outcome);
                // Set by `send_now`'s write path only -- NOT inferred from
                // `Sent::Dead`, which a caller also sees for a generation
                // mismatch or a closed transport. The flag names one specific
                // thing: the bytes could not be written, so the stream is
                // untrustworthy.
                if std::mem::take(&mut self.write_broke_the_stream) {
                    return Some(super::EndReason::WriteFailed);
                }
            }
            crate::command::Inbox::Quit { timeout, reply } => {
                if !self.begin_quit(timeout, reply).await {
                    return Some(super::EndReason::WriteFailed);
                }
            }
        }
        None
    }

    /// Send the exit command and start waiting for the server's EOF
    /// (`plan/16` §5b.3).
    ///
    /// **Does not end the loop.** The loop keeps reading, because the read is
    /// what observes the close -- see [`SessionActor::quitting`]. What this
    /// does is put the command on the wire and arm the deadline.
    ///
    /// A second quit while one is pending is ignored rather than re-sent: the
    /// server has already been asked, and sending `quit` twice against a
    /// type-ahead buffer of 2 would spend a slot for nothing (`plan/16` §5.2b).
    /// The newer caller is answered when the first one resolves, so nobody is
    /// left waiting on a reply that never comes.
    /// Returns `false` if the write failed, which means **the connection is
    /// gone** and the caller must end it rather than keep reading.
    pub(super) async fn begin_quit(
        &mut self,
        timeout: std::time::Duration,
        reply: tokio::sync::oneshot::Sender<crate::command::Farewell>,
    ) -> bool {
        if let Some(pending) = self.quitting.as_mut() {
            // Already asked. Whoever resolves first answers both -- but only
            // one sender fits, so the later caller is told the same thing
            // immediately rather than being dropped silently.
            let _ = reply.send(if pending.reply.is_some() {
                crate::command::Farewell::Acknowledged
            } else {
                crate::command::Farewell::Unsent
            });
            return true;
        }

        // The write goes through the same one-write path every command uses:
        // two writes can emit two TLS records and the server drops the command
        // (`cena_platform::bytes::ByteSource::write_all`).
        let mut message = Vec::with_capacity(EXIT_COMMAND.len() + 1);
        message.extend_from_slice(EXIT_COMMAND.as_bytes());
        message.push(b'\n');
        if !self.write_bounded(&message).await {
            // Nothing to say goodbye to. Lich raises `IOError` here
            // (`orderly_shutdown.rb:181`); Cena reports it and lets the caller
            // cancel, because a transport that cannot be written to is already
            // the state a shutdown was trying to reach.
            self.log("quit: could not send, transport gone");
            let _ = reply.send(crate::command::Farewell::Unsent);
            return false;
        }
        self.recorder.outbound(&message);
        self.log_wire(false, &message);
        // Published like any other send, so an observer sees the session's
        // last act rather than it vanishing.
        let _ = self.events.send(super::Event::Sent {
            line: EXIT_COMMAND.to_owned(),
            origin: Origin::Manual,
        });
        self.log(&format!("quit: sent, awaiting EOF within {timeout:?}"));
        self.quitting = Some(super::Quitting {
            // `checked_add`, because `Instant + Duration` PANICS on overflow
            // and `timeout` is caller-supplied. `SessionHandle::quit` doubles
            // its own backstop before passing it, so a caller near the top of
            // the range gets there in one multiply. A panic here takes down
            // the actor on the one path whose entire job is an orderly exit
            // (review SE-11).
            //
            // Saturating means "no deadline in any practical sense", which is
            // the honest reading of a caller asking to wait ~584 years.
            deadline: tokio::time::Instant::now()
                .checked_add(timeout)
                .unwrap_or_else(|| {
                    tokio::time::Instant::now() + std::time::Duration::from_hours(24)
                }),
            reply: Some(reply),
        });
        true
    }

    /// Send a line immediately, subject only to the roundtime gate.
    ///
    /// **Writes to the socket directly.** It does not enter [`CommandQueue`]
    /// and does not call [`Self::pump`], because every gate in that path is
    /// one `plan/16` §1.4 says must not apply here.
    ///
    /// # What is deliberately NOT checked
    ///
    /// * **Whether a window is open.** Batching several instant actions ahead
    ///   of the command they modify is the whole feature (`plan/16` §1.1).
    ///
    ///   **But batching is not guaranteed to succeed, and this method does not
    ///   promise it.** How many commands the server accepts at once is an
    ///   **account entitlement**: MEASURED 3 on the author's premium account,
    ///   and **1 on free-to-play, which has no type-ahead buffer at all**
    ///   (`plan/16` §5.2b-bis). On a free account every second command sent
    ///   before the first completes is refused, so the batch is not smaller --
    ///   it does not exist.
    ///
    ///   So this method's contract is "does not *wait*", never "will be
    ///   *accepted*". A caller that needs the batch to land must check for the
    ///   server's refusal; a caller that treats `Sent::Ok` as "the game ran
    ///   it" is reading a guarantee that was never offered here (see the
    ///   `Sent` docs).
    /// * **The authority token.** An instant action is not a sequence, so
    ///   `plan/12` §4.1's rule that input is not a claimant applies unchanged
    ///   -- and a behavior holding the authority does not thereby own the
    ///   character's reflexes.
    ///
    /// # What IS checked
    ///
    /// The readiness gate, and roundtime. Readiness for the same reason
    /// [`Self::admit`] checks it: `plan/12` §5.3 gates *behaviors*, and a
    /// behavior firing a sigil before the session is `Ready` is the case that
    /// rule exists for. Manual and script origins are not gated, exactly as in
    /// `admit`, because the player is never locked out of their character.
    async fn send_now(
        &mut self,
        line: &str,
        origin: crate::command::Origin,
        generation: crate::lifecycle::Generation,
        gate: crate::command::Gate,
    ) -> Sent {
        use crate::command::{Gate, Refusal};

        if generation != self.generation {
            return Sent::Interrupted;
        }
        // An instant action bypasses the QUEUE, not the socket. Once `quit` is
        // on the wire this is the same write-into-a-closing-socket the queue
        // path was fixed for (review SE-2).
        if self.quitting.is_some() {
            return Sent::Dead;
        }
        if origin.is_behavior() && !self.lifecycle.behaviors_may_run() {
            return Sent::Refused(Refusal::Transient);
        }
        // The gate, and the reason it returns three different things.
        let at = match gate {
            Gate::None => None,
            Gate::Roundtime => match self.state.in_roundtime() {
                Some(true) => return Sent::Refused(Refusal::Roundtime),
                // Unknown is NOT permission (`plan/12` §5.2). `Transient`
                // because a prompt will arrive and then the answer is knowable
                // -- it is "ask again", not "never".
                None => return Sent::Refused(Refusal::Transient),
                // `in_roundtime` returning `Some` means the clock is known, so
                // this cannot be `None`.
                Some(false) => self.state.game_time_now(),
            },
        };

        let mut message = Vec::with_capacity(line.len() + 1);
        message.extend_from_slice(line.as_bytes());
        message.push(b'\n');
        // Same single write as `pump`: two writes can emit two TLS records and
        // the server drops the command (`cena_platform::bytes::ByteSource`).
        if !self.write_bounded(&message).await {
            // See `handle_inbox`: this ends the connection, because a write that
            // timed out may have put a partial command on the wire and
            // `plan/10`'s single-write rule makes that unrecoverable.
            self.write_broke_the_stream = true;
            return Sent::Dead;
        }
        // **This command's response is now owed, and its prompt is not the
        // in-flight command's terminator.**
        //
        // Any prompt closed the single round-trip window, so three sigils
        // followed by `send_and_await("attack")` resolved the attack on the
        // FIRST sigil's prompt: `Confirmed("You feel a surge.")` under
        // `any_frame`, or a timeout under a strict matcher with the real
        // answer arriving a window late. VERIFIED on the wire before this
        // fix, and the batching shape is the author's own documented usage
        // (review SE-5).
        //
        // A counter rather than attribution: `send_now` has none to protect
        // (`handle.rs`), and it does not need any. What it needs is for the
        // window to survive the prompts that belong to commands sent before
        // it -- which is a COUNT, and the prompts arrive in order.
        self.send_now_prompts_owed = self.send_now_prompts_owed.saturating_add(1);
        self.recorder.outbound(&message);
        self.log_wire(false, &message);
        self.log(&format!("send_now {origin:?} {line}"));
        let _ = self.events.send(Event::Sent {
            line: line.to_owned(),
            origin,
        });
        Sent::Ok { at }
    }

    /// Accept a command, or refuse it because the session is not `Ready`.
    ///
    /// **This is `plan/12` §5.3's readiness gate**: "Behaviors may not start
    /// until `Ready`." A gate that is only a method nobody calls is the wish
    /// `plan/05` §0 warns about, so it is enforced at the one place a
    /// behavior's command can enter the session.
    ///
    /// **Manual input is NOT gated.** §5.3 gates *behaviors*; §4.1 says the
    /// player is never locked out of their character. Refusing a typed command
    /// during `Syncing` would be exactly that lockout, and it is not what
    /// either section asks for.
    pub(super) fn admit(&mut self, envelope: Envelope) {
        // Refused rather than queued: the session is leaving, so a command
        // accepted here could only ever be answered `Dead` when the actor
        // stops. `Disconnected` is the honest verdict -- this connection is
        // ending and a later one may work (review SE-2).
        if self.quitting.is_some() {
            let _ = envelope.reply.send(Outcome::Disconnected);
            return;
        }
        if envelope.origin.is_behavior() && !self.lifecycle.behaviors_may_run() {
            let _ = envelope
                .reply
                .send(Outcome::Refused(crate::command::Refusal::Transient));
            return;
        }
        // §4.2, ENFORCED here rather than described in the queue: a behavior
        // that does not hold the authority is REFUSED, never queued behind the
        // holder. "Silent queueing is how you get an attack that fires four
        // seconds after the fight ended."
        //
        // `Permanent`, not `Transient`: retrying changes nothing while another
        // behavior holds the token, and a `Transient` would invite exactly the
        // spin this rule exists to prevent.
        //
        // Manual input is never checked -- it is not a claimant (§4.1) and
        // `Origin::Manual::token()` is `None`.
        if let Some(token) = envelope.origin.token()
            && self.queue.authority() != Some(token)
        {
            let _ = envelope
                .reply
                .send(Outcome::Refused(crate::command::Refusal::Permanent));
            return;
        }
        self.queue.admit(envelope);
    }

    /// Send whatever the queue is ready to send.
    ///
    /// Returns `Some(reason)` if the session should end, `None` to carry on.
    /// It was a `bool` until Milestone 2 needed the *reason* a write failure
    /// ended a session, not merely that one had.
    /// Answer everything still queued when the session decides to leave.
    ///
    /// Not silently dropped: each of these has a caller waiting on a
    /// `oneshot`, and dropping the sender gives them a `RecvError` that says
    /// nothing. `Disconnected` is the accurate answer -- `plan/12` §5.1 gives
    /// it the meaning "this connection is ending, a retry may work", which is
    /// exactly the situation -- and it is distinct from `Dead`, which would
    /// tell a behavior the session is gone for good.
    fn refuse_queued_after_quit(&mut self) {
        while let Some(envelope) = self.queue.take_next() {
            let _ = envelope.reply.send(Outcome::Disconnected);
        }
    }

    pub(super) async fn pump(&mut self) -> Option<super::EndReason> {
        // **Nothing is written once the session has asked to leave.**
        //
        // `quit` rides the command channel so it is ordered behind everything
        // already QUEUED -- which `handle.rs` says, and which is true of the
        // inbox. It is not true of `CommandQueue`: a command admitted earlier
        // and parked behind an open window is no longer an inbox message, and
        // when the window closes this loop writes it. VERIFIED before the fix:
        // the wire read `["look", "attack", "quit", "stow all"]`.
        //
        // A write after the server has been asked to close tends to draw an
        // RST; the read arm maps any error to `ReadFailed`, which warrants a
        // reconnect, and the `quit` itself counts as attendance -- so a
        // deliberate exit could log the character back in. `plan/16` §5b is
        // about telling "the server closed because we asked" from "the
        // connection dropped", and a stray write blurs exactly that
        // (review SE-2).
        if self.quitting.is_some() {
            self.refuse_queued_after_quit();
            return None;
        }
        while let Some(envelope) = self.queue.take_next() {
            // `plan/12` §5.2: anything from a prior generation is discarded.
            // It cannot fire in Step 2 -- nothing reconnects -- but the check
            // is one line and the alternative is retrofitting it onto a live
            // reconnect path later.
            if envelope.generation != self.generation {
                let _ = envelope.reply.send(Outcome::Interrupted);
                continue;
            }
            // ONE write of the finished message. See
            // `cena_platform::bytes::ByteSource::write_all`: two writes can
            // emit two TLS records and the server drops the command.
            let mut message = Vec::with_capacity(envelope.line.len() + 1);
            message.extend_from_slice(envelope.line.as_bytes());
            message.push(b'\n');
            if !self.write_bounded(&message).await {
                let _ = envelope.reply.send(Outcome::Dead);
                return Some(super::EndReason::WriteFailed);
            }
            self.recorder.outbound(&message);
            self.log_wire(false, &message);
            // Published before the window opens: a behavior observes a manual
            // command as an event (`plan/12` §4.1) and is not cancelled by it.
            let _ = self.events.send(Event::Sent {
                line: envelope.line.clone(),
                origin: envelope.origin,
            });
            self.queue.open_window(
                envelope.id,
                envelope.generation,
                envelope.reply,
                envelope.matcher,
            );
        }
        None
    }

    /// Feed bytes to the parser, fold the frames, publish them, and close a
    /// window on the terminator.
    ///
    /// The bytes go to [`Parser::push_bytes`], never to `parse_line`: that is
    /// the read boundary, and it is what rejoins a tag split across two reads
    /// (`crates/cena-protocol/src/parser/read.rs:16-22`).
    pub(super) fn ingest(&mut self, chunk: &[u8]) {
        self.recorder.inbound(chunk);
        self.log_wire(true, chunk);
        for frame in self.parser.push_bytes(chunk) {
            // Offered to the waiter AND published. `plan/12` §4.4:
            // "observation never competes with attribution."
            //
            // **Not while an instant action's reply is outstanding.** Found by
            // review: `send_now_prompts_owed` protected the TERMINATOR but not
            // the frames before it, so text answering a sigil was offered to a
            // waiting `attack`'s matcher -- reproduced as
            // `Confirmed("You feel a surge.")` attributed to the attack.
            //
            // The counter already says whose text this is. A non-zero count
            // means at least one instant action was sent after the waiting
            // command and its prompt has not arrived, so everything up to that
            // prompt is the instant action's response. Skipping only the
            // terminator left the frames in between attributed to whoever
            // happened to be waiting.
            //
            // This is `plan/12` §4.4's own rule read the other way round:
            // observation must not compete with attribution, and attributing
            // ANOTHER command's text is the sharper failure -- a matcher that
            // sees nothing times out and retries, while one that matches the
            // wrong text returns a confident wrong answer. `offer` records the
            // FIRST match and never revises it (`queue.rs:283`), so the
            // instant action's text wins permanently once it lands.
            //
            // # NOT UNIT-TESTED, stated rather than faked
            //
            // Three attempts failed, and all three would have shipped as false
            // coverage:
            //
            //  1. `send_now.rs` with `release_one` -- `AnsweringSource`
            //     delivers one fixed reply per command, text and prompt as a
            //     single unit, so the owed prompt is always spent on the same
            //     delivery that carries the text. The defect cannot occur.
            //  2. The same, asserting the attack stays unresolved -- passed
            //     under the defect for the same reason.
            //  3. `command_queue.rs` with `ReplaySource`, which CAN split text
            //     from prompt -- but it drains every chunk as fast as it is
            //     read, so the sigil's text arrives before the attack is even
            //     sent. The result was a `Timeout`, i.e. the test measuring
            //     the scheduler rather than the rule.
            //
            // What is needed is a source that withholds bytes until told AND
            // can deliver text without its prompt. `AnsweringSource` has the
            // first, `ReplaySource` the second, neither has both. Building one
            // is the honest fix and it is more than this change should carry;
            // until then this is **enforced by review**, and the mutation that
            // removes `owned_by_send_now` leaves the suite green.
            let owned_by_send_now = self.send_now_prompts_owed > 0;
            if self.queue.window_is_open()
                && !owned_by_send_now
                && !matches!(frame, Frame::Prompt { .. })
            {
                self.queue.offer(&frame);
            }
            let terminator = self.state.apply(&frame);
            let _ = self.events.send(Event::Frame(Box::new(frame)));
            if terminator {
                // A prompt owed to an earlier `send_now` is NOT this window's
                // terminator. Spend one and leave the window open; the
                // in-flight command's own prompt is still coming (SE-5).
                if self.send_now_prompts_owed > 0 {
                    self.send_now_prompts_owed -= 1;
                    continue;
                }
                // The next `Frame::Prompt` closes the window (`plan/12` §4.4).
                self.queue.close_window();
            }
        }
    }
}
