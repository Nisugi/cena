//! The round trip: send a command and wait for its [`Outcome`], as **one
//! call** (`plan/12` §4.5), loud or quiet.
//!
//! Moved down out of `handle.rs` when `send_quietly` took that file past its
//! cap (`plan/05` Rule 4.1: move code down, do not raise the cap). The
//! transport -- the handle, the envelope, the inbox -- stays there; this is
//! the one path a command takes through it and back.

use super::answer::Answers;
use super::handle::{Envelope, Inbox, SessionHandle};
use super::verdict::{CommandId, Gate, Origin, Outcome, Refusal};
use crate::lifecycle::Generation;
use tokio::sync::oneshot;

/// How a round trip goes out, beyond what it says: quietly, past which gate,
/// and answered by what.
#[derive(Clone, Copy)]
struct How {
    quiet: bool,
    gate: Gate,
    answers: Option<Answers>,
}

/// Where a line on the manual path came from.
#[derive(Clone, Copy)]
enum Typed {
    /// Hydra's own line on the manual path: a batch's, a relayed one.
    Not,
    /// The player typed it at a frontend, and no hook changed it.
    Player,
    /// The player typed something, and a script's input hook made it this
    /// line: still the player's typing for the game and for Lich, but a
    /// script's for Hydra's command line.
    Hooked,
}

impl Default for How {
    fn default() -> Self {
        Self {
            quiet: false,
            gate: Gate::None,
            answers: None,
        }
    }
}

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
        self.round_trip(id, line, origin, deadline, matcher, How::default())
            .await
    }

    /// [`Self::send_and_await`], checked once more by the session as the
    /// bytes go out: an action ([`Gate::Act`]) is refused -- not written --
    /// if the live model says roundtime, cast roundtime, stunned, webbed,
    /// dead, or its target gone (`plan/30` §3).
    pub async fn send_gated(
        &self,
        id: CommandId,
        line: &str,
        origin: Origin,
        deadline: std::time::Duration,
        matcher: crate::queue::Matcher,
        gate: Gate,
    ) -> Outcome {
        self.round_trip(
            id,
            line,
            origin,
            deadline,
            matcher,
            How {
                gate,
                ..How::default()
            },
        )
        .await
    }

    /// [`Self::send_gated`], **ended by the line's own answer**: the window
    /// stays open past prompts the game sent for something else, until one
    /// of its own lines `answers` the line or refuses it, and closes at the
    /// prompt after that as [`Outcome::Answered`] (`super::answer`). Nothing
    /// heard within [`WAIT`](super::answer::WAIT) is [`Outcome::Timeout`],
    /// and the line is owed its answer: a late one is not taken as the next
    /// line's.
    ///
    /// For a line whose answer has a shape; a line whose answer has none
    /// goes by [`Self::send_gated`], ended by the first prompt.
    pub async fn send_answered(
        &self,
        id: CommandId,
        line: &str,
        origin: Origin,
        deadline: std::time::Duration,
        answers: Answers,
        gate: Gate,
    ) -> Outcome {
        self.round_trip(
            id,
            line,
            origin,
            deadline,
            crate::queue::any_frame,
            How {
                gate,
                answers: Some(answers),
                ..How::default()
            },
        )
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
        self.round_trip(
            id,
            line,
            origin,
            deadline,
            matcher,
            How {
                quiet: true,
                ..How::default()
            },
        )
        .await
    }

    async fn round_trip(
        &self,
        id: CommandId,
        line: &str,
        origin: Origin,
        deadline: std::time::Duration,
        matcher: crate::queue::Matcher,
        How {
            quiet,
            gate,
            answers,
        }: How,
    ) -> Outcome {
        if origin == Origin::Manual {
            self.attendance.mark();
        }
        let (reply, answer) = oneshot::channel();
        let envelope = Envelope {
            id,
            line: line.to_owned(),
            origin,
            reply,
            generation: self.generation.get(),
            matcher,
            answers,
            quiet,
            gate,
            revocable: None,
        };
        self.submit_and_await(envelope, answer, deadline).await
    }

    /// An agent's line (`crate::operation::send`), as
    /// [`Self::send_and_await`] sends one, but fenced twice at the write: to
    /// `generation`, the connection it was allowed on, not the one current
    /// when this runs; and by `revocable`, which a stop takes back until the
    /// line is written (the integrated crate review of 2026-09-28, I1 and I2).
    pub(crate) async fn agent_round_trip(
        &self,
        generation: Generation,
        line: &str,
        origin: Origin,
        deadline: std::time::Duration,
        revocable: crate::agent::Revocable,
    ) -> Outcome {
        let (reply, answer) = oneshot::channel();
        let envelope = Envelope {
            id: CommandId(0),
            line: line.to_owned(),
            origin,
            reply,
            generation,
            // As typed input's: whatever the game sends before its next
            // prompt answers it; the prompt alone closes the window
            // unanswered (`queue.rs`, `close_window`).
            matcher: crate::queue::any_frame,
            answers: None,
            quiet: false,
            gate: Gate::None,
            revocable: Some(revocable),
        };
        self.submit_and_await(envelope, answer, deadline).await
    }

    /// The player's own line, typed at a frontend: asked of a script
    /// runner's input hooks first (`crate::script`, Lich's `UpstreamHook`),
    /// then Hydra's command line, then Lich or the game. A line the hooks
    /// swallow is [`Outcome::Handled`]; one they have not answered by
    /// [`HOOK_DEADLINE`](crate::script::HOOK_DEADLINE) goes as typed.
    ///
    /// **A line for the game is written at once** and answered
    /// [`Outcome::Sent`]: it does not wait behind the window of the line
    /// typed before it, so a held key sends as fast as it repeats (see
    /// `manual_at`, and `tests/typed_at_once.rs`).
    ///
    /// Hydra's own lines on the manual path -- `;multi`'s, a relayed `;to`,
    /// the sorter's look -- go by `send_manual_at`, past the hooks, as a
    /// Lich script's `put` never meets them: a hook turning a line into a
    /// `;multi` of itself would otherwise never end.
    ///
    /// **With the player's Lich running** (`crate::script::lich`), what the
    /// player types that Hydra does not take is Lich's, so its aliases and
    /// hooks have it, and Lich sends what it makes of it. Hydra's own lines
    /// reach Lich only with Lich's symbol, for the hooks' reason.
    pub async fn send_typed_at(
        &self,
        generation: Generation,
        line: &str,
        deadline: std::time::Duration,
    ) -> Outcome {
        let Some(ask) = self.hooks().typing() else {
            return self
                .manual_at(generation, line, deadline, Typed::Player)
                .await;
        };
        match tokio::time::timeout(crate::script::HOOK_DEADLINE, ask(line)).await {
            Ok(Ok(None)) => {
                // A person typed it, whatever the hooks made of it.
                self.attendance.mark();
                Outcome::Handled
            }
            // A hook that made another line of it made a script's line, for
            // Hydra's command line (the crate review of 2026-10-01, SE-B):
            // the player did not type `.agent level takeover` because a hook
            // turned `look` into it.
            Ok(Ok(Some(changed))) if changed != line => {
                self.manual_at(generation, &changed, deadline, Typed::Hooked)
                    .await
            }
            Ok(Ok(Some(_)) | Err(_)) | Err(_) => {
                self.manual_at(generation, line, deadline, Typed::Player)
                    .await
            }
        }
    }

    /// Queue manual input for the connection the frontend actually observed.
    /// The generation is checked before any command, including a typed quit
    /// or one of Hydra's own, can act. This uses the ordinary manual queue and
    /// never claims or cancels behavior authority.
    ///
    /// A line with Lich's symbol is the player's Lich's, and goes to it rather
    /// than the game (`crate::script::lich`).
    pub async fn send_manual_at(
        &self,
        generation: Generation,
        line: &str,
        deadline: std::time::Duration,
    ) -> Outcome {
        self.manual_at(generation, line, deadline, Typed::Not).await
    }

    /// [`Self::send_manual_at`]; `typed` says whether the player typed it at
    /// a frontend, which a running Lich has before the game, and whether a
    /// script's hook changed it.
    async fn manual_at(
        &self,
        generation: Generation,
        line: &str,
        deadline: std::time::Duration,
        typed: Typed,
    ) -> Outcome {
        let (claimant, typed) = match typed {
            Typed::Not => (Origin::Manual, false),
            Typed::Player => (Origin::Manual, true),
            Typed::Hooked => (Origin::Script, true),
        };
        // A person typed this, whatever becomes of it -- stale, claimed by
        // Hydra's command line, or sent (`attendance.rs`).
        self.attendance.mark();
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
        if let Some(claimed) = self.typed(line, claimant) {
            if claimed == super::Claimed::Unknown {
                let symbol = self.command_symbol().unwrap_or(super::COMMAND_SYMBOL);
                self.say(
                    crate::notice::Notice::line(
                        crate::notice::NoticeKind::Error,
                        format!(
                            "I do not know {}{}. {}help lists Hydra's commands.",
                            symbol,
                            line.trim_start().trim_start_matches(symbol).trim(),
                            symbol,
                        ),
                    )
                    .answering(),
                );
            }
            return Outcome::Handled;
        }
        // Past Hydra: a line with Lich's symbol is the player's Lich's, and
        // never the game's, as one with Hydra's is never the game's. What the
        // player typed is Lich's too while one runs (`crate::script::lich`).
        let lichs = line
            .trim_start()
            .starts_with(crate::script::lich::LICH_SYMBOL);
        if lichs || typed {
            match self.hand_to_lich(line) {
                Some(true) => return Outcome::Handled,
                Some(false) => return Outcome::Refused(Refusal::Transient),
                None if lichs => {
                    self.say(crate::notice::Notice::line(
                        crate::notice::NoticeKind::Error,
                        format!("Lich is not running for this character: {}", line.trim()),
                    ));
                    return Outcome::Handled;
                }
                None => {}
            }
        }
        // **What a person typed is written at once**, as `VellumFE` and Lich
        // write it, not queued behind the window of the line before. Queued,
        // each waited for the last one's prompt, so a held key sent one
        // command a round trip: MEASURED in the author's log of 2026-09-30,
        // 92 `look`s taking 12.7 s to go out, each stamped the millisecond
        // the reply before it arrived (the author: *"entirely
        // unacceptable"*). The game keeps its own type-ahead limit, and says
        // so when it is passed. Nothing waits on a typed line's answer: a
        // frontend shows what the game sends, whenever it comes, and the
        // prompt is booked to this line, not to a behavior's open window
        // (`actor/owed.rs`). A typed `quit` keeps the queue's path, whose
        // answer says the connection is ending.
        if typed && !crate::actor::ending::is_exit_intent(line) {
            return match self
                .send_at(generation, line, Origin::Manual, Gate::None)
                .await
            {
                super::Sent::Ok { .. } => Outcome::Sent,
                super::Sent::Refused(why) => Outcome::Refused(why),
                super::Sent::Dead => Outcome::Dead,
                super::Sent::Interrupted => Outcome::Disconnected,
            };
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
            answers: None,
            quiet: false,
            gate: Gate::None,
            revocable: None,
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
