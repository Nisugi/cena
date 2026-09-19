//! The server clock, and what is derived from it.
//!
//! [`GameState::game_time_now`], [`GameState::in_roundtime`] and
//! [`GameState::roundtime_remaining`] -- one `impl` block, split out of
//! `state.rs` under Rule 4.1 (`plan/05:352-353`): **move code down, do not
//! raise the cap.** That file reached 402 lines against the 400 default when
//! `in_roundtime`'s correction was written down.
//!
//! # Why these three and not some other three
//!
//! Because they are the only methods on [`GameState`] that answer a question
//! about **now**. Everything else in `state.rs` reports what the wire said;
//! these compare it against a clock that keeps running between frames. That is
//! also why they are the methods a roundtime gate calls, and why their
//! `Option` semantics needed a page of justification apiece.

use super::GameState;

impl GameState {
    /// The server's clock **now**, extrapolated.
    ///
    /// The last prompt's timestamp plus how long ago it arrived on the local
    /// monotonic clock. Ported from Vellum
    /// (`core/state.rs`, `game_time_now`), and the single most important
    /// borrowed idea in this file.
    ///
    /// # Why extrapolate rather than read a clock
    ///
    /// **Timers must keep flowing between prompts.** A prompt is sent when
    /// something happens; nothing is sent when a roundtime merely ends
    /// (`plan/15` §2a.1, the author). MEASURED (§2a.4a): a session with a
    /// behavior firing once a second produced 58 prompts, and an idle one
    /// would produce none -- so anything that waits for a prompt to learn a
    /// roundtime ended waits forever in a quiet room.
    ///
    /// **Both sides stay in server time**, so clock skew cancels instead of
    /// needing correction. This is what made an elaborate skew-calibration
    /// design unnecessary: there is no comparison between two clocks anywhere
    /// in it, only an interval measured on one.
    ///
    /// Returns `None` until the first prompt arrives -- `plan/12` §5.2's
    /// `Unknown`, not a fabricated zero.
    #[must_use]
    pub fn game_time_now(&self) -> Option<u32> {
        let base = self.game_time?;
        let elapsed = self.game_time_received.map_or(0, |at| {
            u32::try_from(at.elapsed().as_secs()).unwrap_or(u32::MAX)
        });
        Some(base.saturating_add(elapsed))
    }

    /// Whether the character is in roundtime.
    ///
    /// Compares the extrapolated server clock against
    /// [`Self::roundtime_ends`], both in server epoch seconds.
    ///
    /// # This test is EXACT, not approximate
    ///
    /// MEASURED 2026-09-18 (`plan/15` §2a.4a): across three roundtimes the
    /// first plain `>` prompt landed **precisely** on `<roundTime value=>`, so
    /// `value` is the end instant rather than "somewhere inside that second".
    ///
    /// # `None` means unknown, and unknown is not `false`
    ///
    /// Returns `None` when **the clock** is unobserved -- no prompt has
    /// arrived, so there is nothing to compare against. A caller that treats
    /// that as "not in roundtime" is making exactly the assumption `plan/12`
    /// §5.2 forbids, and the type makes them write the decision down.
    ///
    /// # A roundtime that was never reported is OVER, not unknown
    ///
    /// **CORRECTED 2026-09-18.** This returned `None` when `roundtime_ends`
    /// was `None`, treating "no roundtime has ever been reported" as an
    /// unknown. That is wrong, and it is the normal state at login: a
    /// character who has done nothing has no roundtime, and the first
    /// `<roundTime>` may not arrive for minutes. Under the old reading every
    /// `send_now` refused for the whole of that window -- the feature disabled
    /// exactly when a fresh character is most likely to want a sigil.
    ///
    /// VERIFIED against Lich, which is the reference for what the wire means:
    /// `@roundtime_end = 0` at
    /// `reference/lich-5/lib/common/xmlparser.rb:62`, and `waitrt?` compares
    /// `roundtime_end - now > 0` (`lib/global_defs.rb:283`). An unreported
    /// roundtime is therefore *in the past*, not unheard-of. The same holds
    /// for Vellum's `expires_at` model.
    ///
    /// The asymmetry is the point: the **clock** is a fact about the
    /// connection that must be observed before anything can be compared, and
    /// the **roundtime** is a fact about the character that has a meaningful
    /// default. `plan/12` §5.2 is about not inventing beliefs; concluding "a
    /// roundtime nobody has ever mentioned is not running" invents nothing.
    #[must_use]
    pub fn in_roundtime(&self) -> Option<bool> {
        let now = self.game_time_now()?;
        Some(self.roundtime_ends.is_some_and(|ends| now < ends))
    }

    /// How many seconds of roundtime remain, or `None` if the clock is
    /// unknown.
    ///
    /// Saturates at zero rather than going negative: a roundtime that has
    /// passed has no remainder. A roundtime that was **never reported** also
    /// has none -- `Some(0)`, for the reason [`Self::in_roundtime`] gives at
    /// length. The two methods must agree, or a caller could be told it is not
    /// in roundtime and then handed a remainder.
    #[must_use]
    pub fn roundtime_remaining(&self) -> Option<u32> {
        let now = self.game_time_now()?;
        Some(
            self.roundtime_ends
                .map_or(0, |ends| ends.saturating_sub(now)),
        )
    }
}
