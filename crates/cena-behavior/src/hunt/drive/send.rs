//! The hunt driver's sending: settle roundtime, send through the gate, and
//! read on until the line's own answer is heard (`hunt/answer.rs`), sending
//! again a line the game held back.
//!
//! Moved down out of `drive.rs`, which had passed its cap (`plan/05` Rule 4.4:
//! a split parent's growth goes into its submodules).
//!
//! # One owner for a line held back (the crate review of 2026-10-01)
//!
//! The driver alone sends a held line again; the machine reads no `...wait`
//! (BE-A-11). And it sends again only on a holding that cannot be an earlier
//! line's (BE-A-3): a line whose answer was not heard is **owed** one, and
//! the first line that [settles](answer::settles) it is its, whenever it
//! comes. What is read for the line now awaited starts after that. A holding
//! taken for an earlier line costs a resend, which the next tick makes good
//! from the state; one taken for this line when it was not would send twice
//! a line that went through.

use std::time::Duration;

use cena_session::{CommandId, Frame, Gate, Origin, Outcome, Refusal};
use tokio::time::Instant;

use super::{ANSWER_BEAT, BEAT, Driver, HuntEnd, MAX_RESENDS, SEND_DEADLINE, SETTLE_CAP};
use crate::error::BehaviorError;
use crate::hunt::answer;
use crate::travel::TravelNotes;

/// How long a line with a known answer stays owed it: a reply later than
/// this is taken as the next line's. Well past [`answer::WAIT`], which is
/// how long it was awaited before it was owed.
const OWED_FOR: Duration = Duration::from_secs(10);

/// The most lines owed at once; the oldest goes first.
const MOST_OWED: usize = 4;

/// A line written whose answer has not been heard.
#[derive(Debug)]
pub(super) struct Owed {
    line: String,
    until: Instant,
}

/// How a line's reading ended.
enum Read {
    /// Its answer was heard, and the game held it back for this long if
    /// it did.
    Answered(Option<Duration>),
    /// [`answer::WAIT`] passed first.
    Unanswered,
}

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

    /// Settle, send through the gate, and read on until the line's own
    /// answer is heard (`hunt/answer.rs`): the first prompt may close
    /// something the game said unasked. A line the game held back
    /// (`...wait N seconds.`) is waited out and sent again, up to
    /// [`MAX_RESENDS`] times; every holding is taken out of the transcript,
    /// since none is the line's answer, and nobody else sends it again.
    pub(super) async fn send(&mut self, line: &str, target: Option<i64>) -> Result<(), HuntEnd> {
        for attempt in 0..=MAX_RESENDS {
            self.settle().await?;
            let mark = self.transcript.len();
            self.settled_at = None;
            let id = (self.next_id)();
            let outcome = tokio::select! {
                biased;
                () = self.cancel.cancelled() => return Err(HuntEnd::Stopped(BehaviorError::Cancelled)),
                outcome = self.handle.send_gated(
                    id,
                    line,
                    Origin::Behavior(self.token),
                    SEND_DEADLINE,
                    |frame| matches!(frame, Frame::Prompt { .. }),
                    Gate::Act { target },
                ) => outcome,
            };
            if let Some(gone) = BehaviorError::from_outcome(&outcome) {
                return Err(HuntEnd::Stopped(gone));
            }
            if let Outcome::Refused(refusal) = &outcome {
                if matches!(refusal, Refusal::TargetGone) {
                    self.machine.target_gone();
                }
                // A refusal is a skip: the next tick decides again. A beat, so
                // a refusal that repeats does not spin.
                self.hold(BEAT).await?;
                return self.drain().map_err(HuntEnd::Stopped);
            }
            let written = Instant::now();
            self.drain().map_err(HuntEnd::Stopped)?;
            let pause = match self.answered(line, mark).await? {
                Read::Answered(None) => {
                    self.unheard(line, mark, written);
                    return Ok(());
                }
                Read::Unanswered => {
                    // Its answer may yet come, in the next line's reading.
                    // Unless this reading settled an earlier line: then the
                    // answer it took may have been this one's, and owing
                    // this too would make every line after it wait out
                    // `WAIT` in turn, each taking the one before's answer.
                    if self.settled_at.is_none() {
                        self.owe(line, written + OWED_FOR);
                    }
                    return Ok(());
                }
                Read::Answered(Some(pause)) => pause,
            };
            // The holding is not the line's answer: what the machine reads
            // is the answer to the line that goes through.
            let from = self.reading_from(mark);
            self.transcript.truncate(from);
            if attempt == MAX_RESENDS {
                return Ok(());
            }
            self.hold(pause).await?;
        }
        Ok(())
    }

    /// Fold events until the main window holds `line`'s answer since
    /// `mark` (and since the last line that settled an earlier one), or
    /// [`answer::WAIT`] passes.
    async fn answered(&mut self, line: &str, mark: usize) -> Result<Read, HuntEnd> {
        let deadline = tokio::time::Instant::now() + answer::WAIT;
        loop {
            // A transcript cleared under the mark (the hunt fell behind and
            // took its state afresh) has nothing to read.
            let heard = self
                .transcript
                .get(self.reading_from(mark)..)
                .unwrap_or_default();
            if answer::heard(line, heard) {
                return Ok(Read::Answered(answer::again_after(heard)));
            }
            if tokio::time::Instant::now() >= deadline {
                return Ok(Read::Unanswered);
            }
            self.hold(ANSWER_BEAT).await?;
        }
    }

    /// Where the reading for the line now awaited starts: after its mark,
    /// and after the last line that settled an earlier one.
    fn reading_from(&self, mark: usize) -> usize {
        self.settled_at.map_or(mark, |at| at.max(mark))
    }

    /// A line with no row in `hunt/answer.rs` is answered by the first
    /// prompt; when nothing at all came before that prompt, its reply has
    /// not, and a holding that comes next is likely its. Owed for
    /// [`answer::WAIT`], settled only by a refusal.
    fn unheard(&mut self, line: &str, mark: usize, written: Instant) {
        let nothing = self
            .transcript
            .get(self.reading_from(mark)..)
            .is_none_or(|heard| heard.trim().is_empty());
        if !answer::known(line) && nothing {
            self.owe(line, written + answer::WAIT);
        }
    }

    fn owe(&mut self, line: &str, until: Instant) {
        if self.owed.len() == MOST_OWED {
            self.owed.pop_front();
        }
        self.owed.push_back(Owed {
            line: line.to_owned(),
            until,
        });
    }

    /// A whole main-window line, as the fold reads it: the answer to the
    /// oldest line still owed one, if it settles it.
    pub(super) fn owed_heard(&mut self, said: &str) {
        let now = Instant::now();
        while self.owed.front().is_some_and(|owed| owed.until <= now) {
            self.owed.pop_front();
        }
        if self
            .owed
            .front()
            .is_some_and(|owed| answer::settles(&owed.line, said))
        {
            self.owed.pop_front();
            self.settled_at = Some(self.transcript.len());
        }
    }

    /// The connection dropped or the hunt fell behind: nothing is owed an
    /// answer that will come.
    pub(super) fn owe_nothing(&mut self) {
        self.owed.clear();
        self.settled_at = None;
        self.said.clear();
    }
}
