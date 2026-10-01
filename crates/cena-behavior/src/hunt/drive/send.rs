//! The hunt driver's sending: settle roundtime, send through the gate with
//! what answers the line (`hunt/answer.rs`), and send again a line the game
//! held back.
//!
//! Moved down out of `drive.rs`, which had passed its cap (`plan/05` Rule 4.4:
//! a split parent's growth goes into its submodules).
//!
//! # The session ends the round trip at the answer (2026-10-01)
//!
//! The driver used to read on past the first prompt itself, and keep the
//! lines whose answers had not come as owed. Both are the session's now
//! (`cena_session::command::answer`, `plan/12` §4.4 as amended): a line
//! with a row in `hunt/answer.rs` goes by
//! [`send_answered`](cena_session::SessionHandle::send_answered) and comes
//! back [`Outcome::Answered`] with the game's line that answered it, so a
//! holding is known to be **this** line's, never an earlier one's (BE-A-3).
//! The driver alone sends a held line again; the machine reads no `...wait`
//! (BE-A-11).

use cena_session::command::answer::again_after;
use cena_session::{CommandId, Frame, Gate, Origin, Outcome, Refusal};

use super::{BEAT, Driver, HuntEnd, MAX_RESENDS, SEND_DEADLINE, SETTLE_CAP};
use crate::error::BehaviorError;
use crate::hunt::answer;
use crate::travel::TravelNotes;

impl<F: FnMut() -> CommandId, W: FnMut(&TravelNotes), L: FnMut(&[String])> Driver<'_, F, W, L> {
    /// Wait out roundtime and cast roundtime, up to the cap.
    pub(super) async fn settle(&mut self) -> Result<(), HuntEnd> {
        let cap = tokio::time::Instant::now() + SETTLE_CAP;
        while self.state.in_roundtime() == Some(true) || self.state.in_casttime() == Some(true) {
            if tokio::time::Instant::now() >= cap {
                return Ok(());
            }
            self.hold(BEAT).await?;
        }
        Ok(())
    }

    /// Settle, send through the gate, and fold what the game said until the
    /// line's round trip ends: at its answer for a line with a row in
    /// `hunt/answer.rs`, at the first prompt otherwise. A line the game held
    /// back (`...wait N seconds.`) is waited out and sent again, up to
    /// [`MAX_RESENDS`] times; every holding is taken out of the transcript,
    /// since none is the line's answer, and nobody else sends it again.
    /// Nothing is sent while the connection is away, and a connection lost
    /// while the line is out is waited out, not the end of the hunt.
    pub(super) async fn send(&mut self, line: &str, target: Option<i64>) -> Result<(), HuntEnd> {
        self.back().await?;
        let answers = answer::answers(line);
        for attempt in 0..=MAX_RESENDS {
            self.settle().await?;
            let mark = self.transcript.len();
            let id = (self.next_id)();
            let origin = Origin::Behavior(self.token);
            let gate = Gate::Act { target };
            let outcome = tokio::select! {
                biased;
                () = self.cancel.cancelled() => return Err(HuntEnd::Stopped(BehaviorError::Cancelled)),
                outcome = async {
                    match answers {
                        Some(answers) => {
                            self.handle
                                .send_answered(id, line, origin, SEND_DEADLINE, answers, gate)
                                .await
                        }
                        None => {
                            self.handle
                                .send_gated(
                                    id,
                                    line,
                                    origin,
                                    SEND_DEADLINE,
                                    |frame| matches!(frame, Frame::Prompt { .. }),
                                    gate,
                                )
                                .await
                        }
                    }
                } => outcome,
            };
            match BehaviorError::from_outcome(&outcome) {
                // The connection went while the line was out: waited out,
                // as a walk waits it out (`drive/walk.rs`), and the next
                // tick decides again once the session is back (the crate
                // review of 2026-10-01, BE-A-7).
                Some(BehaviorError::Disconnected) => {
                    self.link_lost();
                    return Ok(());
                }
                Some(gone) => return Err(HuntEnd::Stopped(gone)),
                None => {}
            }
            if let Outcome::Refused(refusal) = &outcome {
                if matches!(refusal, Refusal::TargetGone) {
                    self.machine.target_gone();
                }
                // A refusal is a skip: the next tick decides again. A beat, so
                // a refusal that repeats does not spin.
                self.hold(BEAT).await?;
                return self.drain().await;
            }
            self.drain().await?;
            let held = match &outcome {
                // The line's own answer: a holding is this line's.
                Outcome::Answered(answered) => again_after(answered),
                // A line with no row: what came before its prompt.
                Outcome::Confirmed(_) if answers.is_none() => self
                    .transcript
                    .get(mark..)
                    .unwrap_or_default()
                    .lines()
                    .find_map(again_after),
                // Unanswered: the session keeps its late answer its own,
                // and the next tick decides again from the state.
                _ => None,
            };
            let Some(pause) = held else {
                return Ok(());
            };
            // The holding is not the line's answer: what the machine reads
            // is the answer to the line that goes through.
            self.transcript.truncate(mark.min(self.transcript.len()));
            if attempt == MAX_RESENDS {
                return Ok(());
            }
            self.hold(pause).await?;
        }
        Ok(())
    }
}
