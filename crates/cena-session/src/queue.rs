//! The command queue and the authority token: `plan/12` §4, and the one
//! correction in it that everything else depends on.
//!
//! # The correction
//!
//! `plan/12` §4.1 was **CORRECTED 2026-09-18**. An earlier draft made manual
//! input priority 1 and said it *preempts* the authority holder, which
//! combined with §4.3 meant typing `say hi` mid-hunt would abort Hunt. Neither
//! reference does that: eohunter has no upstream hook at all, and Lich
//! interleaves manual commands with a running engine rather than killing it.
//!
//! **The rule is: manual input is never *queued behind* automation -- not that
//! it *revokes* automation.** That is one method, [`CommandQueue::take_next`],
//! draining `manual` before `held`. It is not a priority system, and building
//! one would be the superseded design.
//!
//! # How a round trip ends
//!
//! §4.4: "A round-trip owns the frame stream from the moment its bytes are
//! written until its **terminator**, which is the next `Frame::Prompt`."
//!
//! VERIFIED against the wire rather than taken on trust, because everything
//! rests on it. Driving `crates/cena-protocol/tests/fixtures/room.xml` through
//! the real `cena_protocol::Parser`:
//!
//! ```text
//! total frames = 71
//! prompt-delimited windows = 2, sizes = [56, 6], trailing unterminated = 9
//! ```
//!
//! The 56-frame first window is exactly the room render, so criterion 2's
//! whole payload arrives inside one prompt window. And prompts are **not** a
//! heartbeat: `prompt.xml`'s four `<prompt time=>` attributes are
//! `[1764475407, 1764475408, 1764475408, 1764475757]`, a **349-second gap**,
//! which rules out a periodic prompt. The game prompts in *response*.
//!
//! **But the timeout arm is load-bearing, not a safety net.** That same probe
//! measured **9 trailing frames with no terminating prompt** in a real
//! fixture. A design where only a prompt can resolve a waiter hangs there.
//! `Outcome::Timeout` means "no match within the window", never "the command
//! did not happen" (§4.4).

use crate::command::answer::{self, Answers};
use crate::command::{CommandId, Envelope, Origin, Outcome};
use crate::lifecycle::Generation;
use std::collections::VecDeque;
use tokio::sync::oneshot;
use tokio::time::Instant;

/// The single token that says who may run a *sequence*.
///
/// `plan/12` §4.1: "The session holds a command authority, a single token.
/// Only its holder may run a sequence. Everything else observes." One token,
/// not a priority queue of claimants -- §4.2 puts Hunt-vs-Heal arbitration
/// inside a supervisor behavior, not at the transport layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthorityToken(pub u64);

/// An open round-trip window: one command's bytes are on the wire and its
/// terminator has not arrived.
#[derive(Debug)]
pub struct InFlight {
    /// For the log. Not for wire matching -- the game carries no ids.
    pub id: CommandId,
    /// The connection this belongs to (`plan/12` §5.2).
    pub generation: Generation,
    /// Where the answer goes.
    pub reply: oneshot::Sender<Outcome>,
    /// What this caller is waiting for. `plan/12` §4.4: "frames are offered to
    /// the waiter's matcher". The matcher belongs to the CALLER -- the queue
    /// has no opinion about which frame answers a `look`.
    pub matcher: Matcher,
    /// The FIRST frame the matcher accepted, which is what
    /// [`Outcome::Confirmed`] carries. First, not last: a window holds every
    /// frame until the prompt, so last-wins returned whatever happened to
    /// arrive nearest the terminator.
    pub matched: Option<Box<cena_protocol::Frame>>,
    /// What answers the line, when the caller named it
    /// ([`crate::command::answer`]): the window then closes at the prompt
    /// after its answer, not at the first prompt. `None`: the first prompt
    /// closes it, and `matcher` picks the frame.
    pub answers: Option<Answers>,
    /// The game's line that answered it, once one has.
    pub answer: Option<String>,
    /// When it goes unanswered: [`answer::WAIT`] after it was written.
    pub until: Instant,
}

/// A line whose window closed before its answer came, still owed it
/// ([`answer::OWED_FOR`]).
#[derive(Debug)]
struct Owed {
    answers: Answers,
    until: Instant,
}

/// A predicate deciding whether a frame answers a command.
///
/// `plan/12` §4.4 makes attribution *temporal* -- the game carries no command
/// ids -- so the window bounds WHEN an answer may arrive and the matcher
/// decides WHICH frame it was. Without one, `Outcome::Confirmed` carries an
/// arbitrary frame: every frame overwrote the previous, so a `look` resolved
/// with whatever landed last before the prompt (measured: `Confirmed(Compass)`
/// rather than the room).
pub type Matcher = fn(&cena_protocol::Frame) -> bool;

/// Accepts any frame at all.
///
/// The honest default for a command whose answer has no distinguishing shape.
/// It is NOT "no matcher" -- it says the caller genuinely does not care, which
/// is a different claim from the old behaviour of silently keeping the last.
#[must_use]
pub const fn any_frame(_frame: &cena_protocol::Frame) -> bool {
    true
}

/// The session's command queue.
///
/// **At most one open window.** A queue exists to prevent overlapping round
/// trips (`plan/12` §4's opening line), and two open windows would mean two
/// waiters competing for the same prompt -- the attribution bug §4.4 spends a
/// page on.
#[derive(Debug, Default)]
pub struct CommandQueue {
    /// Who holds the authority, if anyone: the **session's** cell, shared
    /// with the next connection's queue (`command/authority.rs`, SE-4).
    authority: crate::command::authority::Authority,
    /// Manual commands. Jump the head. **Never touch `authority`.**
    manual: VecDeque<Envelope>,
    /// The authority holder's own commands.
    held: VecDeque<Envelope>,
    /// The one open window.
    in_flight: Option<InFlight>,
    /// Commands dropped because their caller stopped waiting. See
    /// [`Self::abandoned`].
    abandoned: u64,
    /// Lines whose windows closed unanswered, oldest first: the next line
    /// that answers the oldest is its, not the open window's.
    owed: VecDeque<Owed>,
}

/// The most lines owed an answer at once; the oldest goes first.
const MOST_OWED: usize = 4;

impl CommandQueue {
    /// An empty queue with no authority granted.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Grant the authority.
    ///
    /// # Errors
    ///
    /// [`AuthorityHeld`] if someone already has it. `plan/12` §4.2: a behavior
    /// that wants the authority while another holds it gets an error and does
    /// not queue behind it -- "silent queueing is how you get an attack that
    /// fires four seconds after the fight ended."
    pub fn claim(&mut self, token: AuthorityToken) -> Result<(), AuthorityHeld> {
        self.authority.claim(token)
    }

    /// Give the authority back.
    pub fn release(&mut self, token: AuthorityToken) {
        self.authority.release(token);
    }

    /// Who holds the authority.
    #[must_use]
    pub fn authority(&self) -> Option<AuthorityToken> {
        self.authority.holder()
    }

    /// What sent a command of `origin`, as its echo names it: a behavior's
    /// run, by the name it was given (`SessionHandle::name_behavior`).
    /// `None` for anything else, for a run nobody named, and for a `quiet`
    /// command: its report is kept out of the story, and an echo of it would
    /// put back what quiet hid.
    #[must_use]
    pub fn sender_name(&self, origin: crate::command::Origin, quiet: bool) -> Option<String> {
        match origin {
            crate::command::Origin::Behavior(token) if !quiet => self.authority.name_of(token),
            _ => None,
        }
    }

    /// Hold the authority in `cell`, the session's, rather than this queue's
    /// own: what makes it outlive a connection (SE-4).
    pub(crate) fn share_authority(&mut self, cell: crate::command::authority::Authority) {
        self.authority = cell;
    }

    /// Accept a command into the queue.
    ///
    /// **Manual input is not a claimant** and this method never reads or
    /// writes `authority` -- that is §4.1's correction, enforced by the code
    /// not having the branch rather than by a comment asking for it.
    pub fn admit(&mut self, envelope: Envelope) {
        match envelope.origin {
            // A script command queues exactly where manual input does: the
            // author's rule is that a cross-character command runs "as if they
            // just sent it" (`plan/16` §5a.1). It is a distinct variant so a
            // LOG can tell the two apart, not because it queues differently.
            Origin::Manual
            | Origin::Script
            | Origin::Trigger
            | Origin::Lich
            | Origin::Hydra
            | Origin::Agent(None) => {
                self.manual.push_back(envelope);
            }
            Origin::Behavior(_) | Origin::Agent(Some(_)) => self.held.push_back(envelope),
        }
    }

    /// The next command to send, or `None` if a window is open or nothing is
    /// waiting.
    ///
    /// **Manual before held.** Four lines, and the whole of criterion 5's
    /// "jumps the queue".
    ///
    /// Named `take_next` rather than `next` because a bare `next(&mut self)
    /// -> Option<T>` on a non-iterator is `clippy::should_implement_trait`:
    /// a reader reasonably expects `for envelope in queue`, and this type is
    /// not an iterator -- it returns `None` while a window is open and yields
    /// again once it closes.
    pub fn take_next(&mut self) -> Option<Envelope> {
        // **A window whose caller has gone is a CLOSED window.** Only a
        // terminator frame closes one (`actor/io.rs`), so a game that never
        // sends another prompt -- a stalled server, a dropped reply -- wedged
        // the queue permanently. REPRODUCED by review: after one timed-out
        // `look`, a player's `flee` never reached the wire in 300 virtual
        // seconds.
        //
        // That is `plan/12` §4.1's guarantee broken -- "the player is never
        // locked out" -- and in combat it is the worst version of it. The
        // abandoned-command skip below could not help, because it sits behind
        // this early return.
        //
        // Dropping the flight does NOT resolve a waiter: there is nobody left
        // to resolve. §4.4's "a window nobody is waiting on is discarded"
        // already covers this case; what is new is noticing it here rather than
        // only when a terminator eventually arrives.
        if self
            .in_flight
            .as_ref()
            .is_some_and(|flight| flight.reply.is_closed())
        {
            // Its answer may still come, and is not the next line's.
            if let Some(flight) = self.in_flight.take()
                && let (Some(answers), None) = (flight.answers, flight.answer)
            {
                self.owe(answers);
            }
            self.abandoned += 1;
        }
        if self.in_flight.is_some() {
            return None;
        }
        // **Skip commands nobody is waiting for.** A caller whose
        // `send_and_await` timed out dropped the receiving half of its
        // `oneshot`, and sending its command anyway is the failure this guards:
        // in a game that is an attack firing after the player or behavior gave
        // up on it, spending a roundtime on an intention that was withdrawn.
        //
        // **This does not contradict `plan/12` §4.4.** §4.4 says a timeout is
        // not "the command did not happen", and that stays true -- it is about
        // a command already ON THE WIRE whose answer never came. This drops
        // only commands that were never written, where "it did not happen" is
        // simply accurate. The two cases are distinguished by exactly this
        // point in the code: past here the bytes go out, before it they never
        // did.
        //
        // `is_closed` is the check rather than a liveness flag we maintain,
        // because the receiver's drop IS the signal and `oneshot` already
        // tracks it. A flag would be a second source of truth that could
        // disagree.
        loop {
            let next = self.manual.pop_front().or_else(|| self.held.pop_front())?;
            if next.reply.is_closed() {
                self.abandoned += 1;
                continue;
            }
            // **A preempted holder's queued commands are not sent** (`plan/12`
            // §4.3). They were admitted while it held the authority; it no
            // longer does, and an attack from a behavior that was stopped is
            // the thing a stop exists to prevent.
            if let Some(token) = next.origin.token()
                && self.authority.holder() != Some(token)
            {
                let _ = next
                    .reply
                    .send(Outcome::Refused(crate::command::Refusal::Permanent));
                continue;
            }
            return Some(next);
        }
    }

    /// How many queued commands were dropped because their caller stopped
    /// waiting.
    ///
    /// Counted rather than silently discarded: a session dropping commands is
    /// either a behavior using timeouts too tightly or a game that has stopped
    /// answering, and both are things someone debugging needs to see. Nothing
    /// reads this yet beyond the tests -- it is the number a diagnostic would
    /// want, recorded where it happens.
    #[must_use]
    pub const fn abandoned(&self) -> u64 {
        self.abandoned
    }

    /// Open a window for a command whose bytes have just gone out; `answers`
    /// is what answers it, if the caller named that ([`InFlight::answers`]).
    pub fn open_window(
        &mut self,
        id: CommandId,
        generation: Generation,
        reply: oneshot::Sender<Outcome>,
        matcher: Matcher,
        answers: Option<Answers>,
    ) {
        self.in_flight = Some(InFlight {
            id,
            generation,
            reply,
            matcher,
            matched: None,
            answers,
            answer: None,
            until: Instant::now() + answer::WAIT,
        });
    }

    /// One of the game's own lines in the main window, finished, its markup
    /// removed (never a line a person said): the answer to the oldest line
    /// still owed one if it settles it, else the open window's if it answers
    /// that. First answer wins.
    pub fn hear(&mut self, line: &str) {
        let now = Instant::now();
        while self.owed.front().is_some_and(|owed| owed.until <= now) {
            self.owed.pop_front();
        }
        if self
            .owed
            .front()
            .is_some_and(|owed| answer::answered_by(owed.answers, line))
        {
            self.owed.pop_front();
            return;
        }
        if let Some(flight) = self.in_flight.as_mut()
            && flight.answer.is_none()
            && flight
                .answers
                .is_some_and(|answers| answer::answered_by(answers, line))
        {
            flight.answer = Some(line.trim().to_owned());
        }
    }

    /// When the open window goes unanswered, if it waits for an answer.
    #[must_use]
    pub fn window_deadline(&self) -> Option<Instant> {
        self.in_flight
            .as_ref()
            .filter(|flight| flight.answers.is_some())
            .map(|flight| flight.until)
    }

    /// The open window's [`answer::WAIT`] has passed: it closes with what it
    /// heard, or unanswered and owed its answer.
    pub fn expire_window(&mut self) {
        if self
            .window_deadline()
            .is_some_and(|until| until <= Instant::now())
        {
            self.resolve();
        }
    }

    fn owe(&mut self, answers: Answers) {
        if self.owed.len() == MOST_OWED {
            self.owed.pop_front();
        }
        self.owed.push_back(Owed {
            answers,
            until: Instant::now() + answer::OWED_FOR,
        });
    }

    /// Whether a window is open.
    #[must_use]
    pub fn window_is_open(&self) -> bool {
        self.in_flight.is_some()
    }

    /// Offer a frame to the open window, if there is one.
    ///
    /// The frame is offered **and also published** by the caller -- §4.4:
    /// "observation never competes with attribution". This method does not
    /// consume the frame.
    pub fn offer(&mut self, frame: &cena_protocol::Frame) {
        if let Some(flight) = self.in_flight.as_mut() {
            // FIRST match wins, and a non-match changes nothing. The previous
            // version assigned unconditionally, so `matched` held the last
            // frame before the prompt rather than the answer.
            if flight.matched.is_none() && (flight.matcher)(frame) {
                flight.matched = Some(Box::new(frame.clone()));
            }
        }
    }

    /// Close the open window with the terminator, resolving its waiter.
    ///
    /// A window with a matched frame resolves [`Outcome::Confirmed`]; one with
    /// none resolves [`Outcome::Timeout`], which §4.4 defines as "no match
    /// within the window" and explicitly **not** "the command did not happen".
    ///
    /// **A window waiting for its answer stays open** past a prompt that came
    /// before it (`crate::command::answer`): the prompt after the answer is
    /// the one that closes it, as [`Outcome::Answered`].
    pub fn close_window(&mut self) {
        if self
            .in_flight
            .as_ref()
            .is_some_and(|flight| flight.answers.is_some() && flight.answer.is_none())
        {
            return;
        }
        self.resolve();
    }

    /// Close the open window and answer its waiter with what it got.
    fn resolve(&mut self) {
        let Some(flight) = self.in_flight.take() else {
            return;
        };
        let outcome = match (flight.answers, flight.answer, flight.matched) {
            (Some(_), Some(line), _) => Outcome::Answered(line),
            (Some(answers), None, _) => {
                self.owe(answers);
                Outcome::Timeout
            }
            (None, _, Some(frame)) => Outcome::Confirmed(frame),
            (None, _, None) => Outcome::Timeout,
        };
        // A dropped receiver means the caller stopped waiting -- e.g. its own
        // `send_and_await` deadline fired first. That is not an error here:
        // §4.4's late-response rule says a window that nobody is waiting on
        // still closes, it just has nowhere to deliver.
        let _ = flight.reply.send(outcome);
    }

    /// Answer every waiter, in flight and queued, then clear.
    ///
    /// # The `&Outcome` parameter, restored -- and why that is not a reversal
    ///
    /// An earlier version sent `Outcome::Dead` to each waiter and was cut back
    /// to a plain drop, because dropping a `oneshot::Sender` already wakes its
    /// receiver and `SessionHandle::send_and_await` maps that to
    /// `Outcome::Dead`. The explicit send produced the same observable value by
    /// a longer route, and its parameter "only ever received one value --
    /// Rule -1's 'no config option with one value' in argument form."
    ///
    /// **That was correct at the time and the test has now changed.** Milestone
    /// 2 gives it a second caller: a supervised session answers
    /// [`Outcome::Disconnected`] because another connection is coming, and an
    /// unsupervised one answers [`Outcome::Dead`] because none is. Two callers,
    /// two values, so Rule -1 is satisfied by the same standard that failed it
    /// before -- not by relaxing it.
    ///
    /// The drop-based route cannot express this at all: dropping the sender
    /// always yields `Dead`, so the distinction has to be *sent*.
    ///
    /// # Why it still happens at a NAMED POINT
    ///
    /// Unchanged from the version before: the waiters are answered in
    /// `shutdown`, before the actor returns, rather than whenever the actor's
    /// memory happens to be released. That is what criterion 6 is about.
    pub fn answer_all_waiters(&mut self, outcome: &Outcome) {
        // Nothing owed an answer gets one from a connection that is gone.
        self.owed.clear();
        if let Some(flight) = self.in_flight.take() {
            let _ = flight.reply.send(outcome.clone());
        }
        for envelope in self.manual.drain(..).chain(self.held.drain(..)) {
            let _ = envelope.reply.send(outcome.clone());
        }
    }
}

/// Someone else holds the authority. `plan/12` §4.2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthorityHeld(pub AuthorityToken);

impl std::fmt::Display for AuthorityHeld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the command authority is held by {:?}", self.0)
    }
}

impl std::error::Error for AuthorityHeld {}
