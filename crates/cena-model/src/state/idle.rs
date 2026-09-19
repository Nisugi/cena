//! The server's idle warning: reading it, and clearing it.
//!
//! Split out of `state.rs` beside [`clock`](super::clock) and
//! [`reconnect`](super::reconnect), under Rule 4.1 (`plan/05:352-353`) -- move
//! code down, do not raise the cap.
//!
//! # What this is for
//!
//! `MAX_UNATTENDED_LOSSES = 2` exists because the game idle-kicks after ~30
//! minutes, and an idle kick is the one disconnect where **the connection worked
//! perfectly** -- so the supervisor's `worked` test resets the backoff ladder and
//! it reconnects at the one-second rung, idles for another 30 minutes, and is
//! kicked again. The cap stops that only after two full cycles.
//!
//! The server announces it in advance, in plain text. Lich strips the bells and
//! discards the line; `VellumFE` never mentions it. Full evidence and rationale
//! in `crates/cena-model/tests/idle_warning.rs`.
//!
//! # The three states, and why the field is private
//!
//! | [`GameState::idle_warned`] | [`GameState::idle_warned_at`] | Means |
//! |---|---|---|
//! | `false` | `None` | never warned, or answered since |
//! | `true` | `None` | warned before any prompt, so no server clock yet |
//! | `true` | `Some(t)` | warned at server second `t` |
//!
//! Three states, held in a named `IdleWarning` enum rather than an
//! `Option<Option<u32>>` -- that was the first shape and clippy's `option_option`
//! objected, correctly. Review pattern (A) in `plan/19` is an `Option` read as
//! two states where three exist, and a nested one makes a reader decode the
//! nesting to find the third. The field stays private and these methods are the
//! contract.

use super::{GameState, IdleWarning};

impl GameState {
    /// Has the server warned that this character is idle, without an answer
    /// since?
    ///
    /// The supervisor's question. `true` means the next disconnect is most likely
    /// an idle kick rather than a lost network, which is the difference between
    /// "back off and retry" and "stop, nobody is here".
    #[must_use]
    pub const fn idle_warned(&self) -> bool {
        !matches!(self.idle_warning, IdleWarning::None)
    }

    /// The server clock when the warning arrived, if there was one.
    ///
    /// `None` when not warned **or** when warned with no clock yet, which is why
    /// [`Self::idle_warned`] is the liveness test and this is only the timestamp.
    /// Conflating them would make a warning during the login burst -- before the
    /// first prompt -- read as no warning at all.
    #[must_use]
    pub const fn idle_warned_at(&self) -> Option<u32> {
        match self.idle_warning {
            IdleWarning::At(at) => Some(at),
            IdleWarning::None | IdleWarning::Unstamped => None,
        }
    }

    /// The character acted, so the warning is answered.
    ///
    /// Called by the session actor on every outbound write, because **only an
    /// outbound command answers an idle warning** and this model is inbound-only.
    ///
    /// The obvious alternative -- clear it on the next `<prompt>` -- is wrong, and
    /// measurably so: on `GSIV-Dicate` (2024-10-12) the session idled on for
    /// minutes after the warning while prompts arrived every few seconds, driven
    /// by `dialogData id='Buffs'` refreshes and a bystander's emote. A prompt
    /// means something happened, not that the player did something.
    ///
    /// Idempotent, and cheap enough to call unconditionally: it is one enum store
    /// on a path that is already doing a socket write.
    pub const fn answer_idle_warning(&mut self) {
        self.idle_warning = IdleWarning::None;
    }
}
