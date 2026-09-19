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

use super::{Envelope, Event, SessionActor};
use crate::command::{Outcome, Sent};
use cena_platform::ByteSource;
use cena_protocol::Frame;

impl<S: ByteSource> SessionActor<S> {
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
    pub(super) async fn handle_inbox(&mut self, message: crate::command::Inbox) {
        match message {
            crate::command::Inbox::Command(envelope) => self.admit(*envelope),
            crate::command::Inbox::Claim { token, reply } => {
                let _ = reply.send(self.queue.claim(token));
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
            }
        }
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
        if self.source.write_all(&message).await.is_err() {
            return Sent::Dead;
        }
        self.recorder.outbound(&message);
        self.log_wire(false, &message);
        self.log(&format!("send_now {origin:?} {line}"));
        let _ = self.events.send(Event::Sent {
            line: line.to_owned(),
            origin,
        });
        Sent::Ok { at }
    }

    pub(super) fn admit(&mut self, envelope: Envelope) {
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

    /// Send whatever the queue is ready to send. Returns `false` if the
    /// session should end.
    pub(super) async fn pump(&mut self) -> bool {
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
            if self.source.write_all(&message).await.is_err() {
                let _ = envelope.reply.send(Outcome::Dead);
                return false;
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
        true
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
            if self.queue.window_is_open() && !matches!(frame, Frame::Prompt { .. }) {
                self.queue.offer(&frame);
            }
            let terminator = self.state.apply(&frame);
            let _ = self.events.send(Event::Frame(Box::new(frame)));
            if terminator {
                // The next `Frame::Prompt` closes the window (`plan/12` §4.4).
                self.queue.close_window();
            }
        }
    }
}
