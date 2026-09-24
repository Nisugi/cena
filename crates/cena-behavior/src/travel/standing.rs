//! Standing up before a move (Vellum's `tick_prepare` and `AwaitStand`,
//! `executor.rs:891-915`; go2's stand loop, `go2.lic:2326-2364`).
//!
//! Moved down out of `travel.rs` when the wait between `stand`s took it past
//! its line cap (`plan/05` Rule 4.1: move code down, do not raise the cap).
//! It is [`Trip`]'s own state, so it stays an `impl Trip`.

use cena_map::{Action, Step, Walker};
use cena_session::MoveFeedback;

use super::{Said, Trip, mover};

/// How many times the trip will send `stand` before a move. Vellum's
/// `MAX_STAND_ATTEMPTS`.
pub const MAX_STANDS: u32 = 5;

/// How long a `stand` is given to take before another is sent. Vellum's
/// `STAND_TIMEOUT_MS` (`executor.rs:525`).
///
/// **Without it, [`MAX_STANDS`] was a count of ticks, not of tries.** The trip
/// answered `stand` on every tick until the posture changed, and a driver
/// ticks on every line: a review measured five `stand`s in five milliseconds
/// and then the move, sent while still kneeling -- the budget spent before the
/// game had answered the first.
pub const STAND_TIMEOUT_MS: u64 = 2_500;

/// What the game says when standing cannot be done *here*: go2's
/// `stand_regex` (`go2.lic:2328-2341`), the lines that mean "not now, and not
/// by trying again". Vellum reads the same set as `StandBlocked` and moves on
/// without standing; so does this.
const CANNOT_STAND_HERE: [&str; 5] = [
    "You cannot do that while mounted.",
    "There's not enough room to do that!",
    "There is not enough room to stand up in here.",
    "You'd tip the boat over!",
    "You are overburdened and cannot manage to stand.",
];

impl Trip {
    /// Vellum stands before it moves, unless the move is a swim or a pedal
    /// (`tick_prepare`). A walker whose posture is not known is left alone:
    /// the game will say "you must be standing" if it matters, and that has
    /// its own remedy. `None`: go on and cross.
    ///
    /// # One `stand` at a time (Vellum's `AwaitStand`, `executor.rs:891-915`)
    ///
    /// A `stand` is sent, and then **waited on**: the posture changing is the
    /// answer, [`STAND_TIMEOUT_MS`] without one earns another try, and a
    /// `...wait N` pushes the next try past the roundtime. Vellum checks
    /// `rt_remaining` before each resend; the trip has no clock of the game's,
    /// so it takes the seconds from the line that states them, as `Exchange`
    /// does.
    ///
    /// # After [`MAX_STANDS`], the move goes anyway
    ///
    /// Where this is **go2** and not Vellum. Vellum fails the trip ("can't
    /// stand up - travel aborted"); go2 sends one `stand` per step and moves
    /// whatever it answered (`go2.lic:2326-2364`). A move sent while down is
    /// answered "you must be standing", and that line has its own bounded
    /// remedy (`recovery`, `MAX_REMEDIES`) which gives the exit up if it
    /// cannot be mended -- so the walker is not stuck, and a posture the model
    /// misread costs one refused move instead of the whole trip.
    pub(super) fn stand_first(&mut self, walker: &Walker, steps: &[Step], ms: u64) -> Option<Said> {
        let down = walker.posture.as_deref().is_some_and(|is| is != "standing");
        if !down {
            self.stand_sent = None;
            self.stand_waived = false;
            return None;
        }
        let afloat = steps.iter().any(|step| match &step.action {
            Action::Move(command) => command.contains("swim") || command.contains("pedal"),
            _ => false,
        });
        if afloat || self.stand_waived {
            return None;
        }
        if let Some(sent) = self.stand_sent {
            let answered = ms.saturating_sub(sent) >= STAND_TIMEOUT_MS;
            if !answered || ms < self.stand_rt_until {
                return Some(Said::Hold);
            }
        }
        if self.stands >= MAX_STANDS {
            return None;
        }
        self.stands += 1;
        self.stand_sent = Some(ms);
        Some(Said::Send("stand".to_owned()))
    }

    /// What the game said to a `stand` still waiting on the posture. Nothing,
    /// when no `stand` is: a roundtime or a boat heard then is another
    /// command's.
    pub(super) fn heard_about_standing(
        &mut self,
        feedback: &[MoveFeedback],
        lines: &[String],
        ms: u64,
    ) {
        if self.stand_sent.is_none() {
            return;
        }
        for feedback in feedback {
            if let MoveFeedback::Wait(seconds) = feedback {
                self.stand_rt_until = ms + mover::wait_ms(*seconds);
            }
        }
        if lines
            .iter()
            .any(|line| CANNOT_STAND_HERE.iter().any(|said| line.contains(said)))
        {
            self.stand_waived = true;
        }
    }
}
