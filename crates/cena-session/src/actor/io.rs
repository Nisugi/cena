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
use crate::command::Outcome;
use cena_platform::ByteSource;
use cena_protocol::Frame;

impl<S: ByteSource> SessionActor<S> {
    /// Route one inbox message.
    ///
    /// Claims and releases arrive on the SAME channel as commands (`plan/12`
    /// §4.2 is an ordering rule), so a claim cannot overtake a command already
    /// queued behind it.
    pub(super) fn handle_inbox(&mut self, message: crate::command::Inbox) {
        match message {
            crate::command::Inbox::Command(envelope) => self.admit(*envelope),
            crate::command::Inbox::Claim { token, reply } => {
                let _ = reply.send(self.queue.claim(token));
            }
            crate::command::Inbox::Release(token) => self.queue.release(token),
        }
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
