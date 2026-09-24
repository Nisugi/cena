//! The round trip: send a command and wait for its [`Outcome`], as **one
//! call** (`plan/12` §4.5), loud or quiet.
//!
//! Moved down out of `handle.rs` when `send_quietly` took that file past its
//! cap (`plan/05` Rule 4.1: move code down, do not raise the cap). The
//! transport -- the handle, the envelope, the inbox -- stays there; this is
//! the one path a command takes through it and back.

use super::handle::{Envelope, Inbox, SessionHandle};
use super::verdict::{CommandId, Origin, Outcome, Refusal};
use crate::lifecycle::Generation;
use tokio::sync::oneshot;

impl SessionHandle {
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
        self.round_trip(id, line, origin, deadline, matcher, false)
            .await
    }

    /// [`Self::send_and_await`], with the game's answer kept **out of the
    /// story**: a command Hydra runs for itself, whose report the player did
    /// not ask to read.
    ///
    /// The author, 2026-09-24, of the character sync: *"run it quietly but
    /// like infomon, infomon does a one line message for each stage and hides
    /// all the spam."* The one line is the caller's to say
    /// ([`Self::say`]); hiding the report is this. The session brackets the
    /// command's window with [`Event::Quiet`](crate::Event::Quiet), and every
    /// frame still reaches the model, the logs and every subscriber.
    pub async fn send_quietly(
        &self,
        id: CommandId,
        line: &str,
        origin: Origin,
        deadline: std::time::Duration,
        matcher: crate::queue::Matcher,
    ) -> Outcome {
        self.round_trip(id, line, origin, deadline, matcher, true)
            .await
    }

    async fn round_trip(
        &self,
        id: CommandId,
        line: &str,
        origin: Origin,
        deadline: std::time::Duration,
        matcher: crate::queue::Matcher,
        quiet: bool,
    ) -> Outcome {
        let (reply, answer) = oneshot::channel();
        let envelope = Envelope {
            id,
            line: line.to_owned(),
            origin,
            reply,
            generation: self.generation.get(),
            matcher,
            quiet,
        };
        self.submit_and_await(envelope, answer, deadline).await
    }

    /// Queue manual input for the connection the frontend actually observed.
    /// The generation is checked before any command, including a typed quit
    /// or one of Hydra's own, can act. This uses the ordinary manual queue and
    /// never claims or cancels behavior authority.
    pub async fn send_manual_at(
        &self,
        generation: Generation,
        line: &str,
        deadline: std::time::Duration,
    ) -> Outcome {
        // **The generation first, and here rather than only in the actor.**
        // A claimed line never reaches the actor, so the actor's check could
        // not protect it: a `;go2 bank` typed into a browser still showing
        // the previous connection ran against the new one (review finding 8).
        // `Disconnected` is what the actor answers a stale command, so both
        // paths say the same thing.
        //
        // This is a pre-check, not the fence: the cell can advance between
        // here and the actor, which is why the actor checks again.
        if generation != self.generation.get() {
            return Outcome::Disconnected;
        }
        // The player's own commands never reach the game, known or not
        // (`super::claimant`), so they are answered `Handled`: no window was
        // opened and no frame matched. An unknown one is handled too -- by
        // telling the player so.
        if let Some(claimed) = self.typed(line) {
            if claimed == super::Claimed::Unknown {
                let symbol = self.command_symbol().unwrap_or(super::COMMAND_SYMBOL);
                self.say(crate::notice::Notice::line(
                    crate::notice::NoticeKind::Error,
                    format!(
                        "I do not know {}{}.",
                        symbol,
                        line.trim_start().trim_start_matches(symbol).trim()
                    ),
                ));
            }
            return Outcome::Handled;
        }
        let (reply, answer) = oneshot::channel();
        let envelope = Envelope {
            // Browser input has no behavior command correlation id. The
            // generation, not this diagnostic id, is the authority fence.
            id: CommandId(0),
            line: line.to_owned(),
            origin: Origin::Manual,
            reply,
            generation,
            matcher: crate::queue::any_frame,
            quiet: false,
        };
        self.submit_and_await(envelope, answer, deadline).await
    }

    async fn submit_and_await(
        &self,
        envelope: Envelope,
        answer: oneshot::Receiver<Outcome>,
        deadline: std::time::Duration,
    ) -> Outcome {
        if !self.has_room_for_traffic() {
            // One slot short of full: refused so a `release` can still get
            // through. `Transient` is already "ask again", which is what a
            // caller should do.
            return Outcome::Refused(Refusal::Transient);
        }
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
}
