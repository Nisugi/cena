//! What a new connection has not been told: `plan/12` §5.2's invalidation.
//!
//! Split out of `state.rs` beside [`clock`](super::clock) and for the same
//! reason — `state.rs` is near its cap and the fields list is the part that
//! grows. Rule 4.1 (`plan/05:352-353`): move code down, do not raise the cap.
//!
//! # The classification is MEASURED, not designed
//!
//! `plan/12` §5.2 sorts facts into Invalidated / Retained / Suspect and says
//! the invalidated ones become `Unknown` "until re-observed". That was a design
//! assertion. It is now a measurement, taken from Cena's own logins on
//! 2026-09-18 (`plan/15` §2a.4a) by counting tags before the first client command:
//!
//! | In the login burst | Absent from it |
//! |---|---|
//! | room description (`compDef`), ten vitals `progressBar`s, `playerID`, `inv`, layout | `nav rm`, `compass`, `prompt`, hands, `roundTime`, all four effect dialogs |
//!
//! **Unanimous across all seven logins.** The absent facts arrive only after
//! the first command — they are answers to asking, not part of the login push.
//!
//! # `indicator` was in the Absent column and should not have been
//!
//! This table listed `indicator` as absent from the burst. `status.rs`'s
//! header says the opposite, from the same capture, and `plan/15` §2a.4a
//! settles it:
//!
//! > **Indicators arrive as ONE bulk declaration at login.** All ten of
//! > Lich's `ICONMAP` ids on a single line, `IconSTANDING` the only
//! > `visible="y"`.
//!
//! Two measurements of one capture disagreed inside one crate, and the wrong
//! one was load-bearing here: it is the stated reason for clearing the
//! indicators (review MO-12).
//!
//! **Clearing them is still right**, for the other reason `plan/12` §5.2
//! gives -- they are Invalidated, and a stale `stunned` surviving a
//! reconnect is exactly the belief that rule forbids. A field the burst
//! refills is merely cleared and immediately repopulated, which §5.2's own
//! text calls "pointless" rather than wrong. So the behaviour is unchanged
//! and only the justification is corrected.
//!
//! So the rule implemented here is: **clear what the burst does not re-send.**
//! A field the burst refills would be cleared and immediately repopulated,
//! which is merely pointless. A field the burst does *not* refill and that is
//! left alone is a **stale belief surviving a generation**, which is the whole
//! failure §5.2 exists to prevent.
//!
//! # Why Cena invalidates where Vellum does not
//!
//! VERIFIED that `VellumFE` keeps its game state on disconnect
//! (`reference/VellumFE/src/frontend/gui/app/server_pump.rs:392` sets
//! `connected = false` and clears only pending launches). That is correct for
//! Vellum and wrong here:
//!
//! > **AUTHOR, 2026-09-18:** *"cena is the full shebang. it's the one
//! > controlling everything so it's not vellum disconnecting from lich, etc. It
//! > would be a full reconnect and the game would send us the connection
//! > stuff."*
//!
//! Vellum disconnects from **Lich**, which stays logged in and holds the game
//! session — the character never left the world, so Vellum's state is merely
//! un-updated. Cena **is** the login, so a disconnect ends the session and
//! anything not re-sent is **unobserved rather than stale**. Lich, also a full
//! client, invalidates: `Inventory.reset!` exists specifically for "a session
//! reset / reconnect" and drops mirrored state so "no stale container mirror
//! survives" (`reference/lich-5/lib/common/inventory.rb:1014-1045`).

use super::{GameState, Room};

impl GameState {
    /// Forget everything a new connection has not yet been told.
    ///
    /// Called between generations, never during one. What survives is what the
    /// login burst re-sends unprompted; everything else returns to `Unknown`.
    ///
    /// # Not `Default::default()`
    ///
    /// Resetting the whole struct would discard two things that are **facts
    /// about the session rather than about the connection**:
    ///
    /// * [`unknown_tags`](GameState::unknown_tags) — criterion 8's evidence. A
    ///   tag the parser could not model is a fact about the *protocol*, and it
    ///   is most useful across a reconnect, not least.
    /// * [`vitals`](GameState::vitals) — MEASURED as re-sent in every burst, so
    ///   clearing them opens a window where health reads `Unknown` for no
    ///   reason at all.
    ///
    /// # The destructuring is the point
    ///
    /// Every field is named, mirroring [`PartialEq`]'s hand-written impl and
    /// for the identical reason it gives: *"adding a field is a compile error
    /// here, and whoever adds it has to decide which side it belongs on."*
    /// A future field that silently defaulted to "retained" would be a stale
    /// belief nobody chose to keep — which is the exact bug this method exists
    /// to prevent, arriving by omission instead of by decision.
    pub fn invalidate_for_reconnect(&mut self) {
        let Self {
            room,
            prompt,
            left_hand,
            right_hand,
            roundtime_ends,
            vitals,
            status,
            effects,
            game_time,
            game_time_received,
            unknown_tags,
            unknown_tag_counts,
            idle_warning,
            streams,
            pending,
            character,
            inventory,
        } = self;

        // --- Cleared: the burst does not carry these -----------------------

        // WHOLE, despite the burst carrying `compDef id='room desc'`.
        // `GameState::apply` already treats the room atomically -- on
        // `Frame::RoomId` it replaces the entire `Room`, because a description
        // without its id "describes somewhere the character no longer is".
        // Keeping a description whose id and exits are gone would recreate
        // exactly the inconsistency that rule prevents, and the burst re-sends
        // the description anyway, so nothing is lost.
        *room = Room::default();
        *prompt = None;
        *left_hand = None;
        *right_hand = None;

        // **RETAINED, and §5.2 is still satisfied.** This used to clear it,
        // citing §5.2's "`world.roundtime` after reconnect is `Unknown`, never
        // `0`". The intent was right and the mechanism was wrong, in a way a
        // review reproduced: `in_roundtime` maps a cleared `roundtime_ends` to
        // `Some(false)` as soon as a prompt restores the clock
        // (`clock.rs:93-96`, `is_some_and` on `None`), so "Unknown" became
        // "not in roundtime" -- with 20 real seconds left, letting a
        // `Gate::Roundtime` send through early.
        //
        // It is retained because **it is an absolute server epoch**, not a
        // relative or connection-scoped fact. A roundtime that ends at server
        // second N ends at N whether or not the socket survived; unlike the
        // room or the hands, nothing about it can have changed while we were
        // away. Keeping a true fact is not inventing a belief.
        //
        // What §5.2 actually forbids -- reporting `0`, i.e. "definitely not in
        // roundtime", on no evidence -- is unchanged: with no clock
        // `in_roundtime` still returns `None`, and a roundtime that has since
        // expired compares as expired rather than being asserted.
        let _ = roundtime_ends;

        // Both have a `clear` written FOR this path and, until now, no caller
        // (`status.rs`, `effects.rs`: "for `plan/12` §5.2's reconnect path").
        status.clear();
        effects.clear();

        // **Both halves of the clock, and this is the subtle one.**
        // `game_time_now` extrapolates from `game_time_received.elapsed()`, so
        // a clock carried across a thirty-second reconnect would report a
        // server time thirty seconds in the future -- and `in_roundtime`
        // compares against it. Keeping the reading while dropping the receipt
        // instant, or the reverse, would be worse than keeping both.
        *game_time = None;
        *game_time_received = None;

        // A fact about the CONNECTION that just ended, not about the character.
        // Carrying it across would make the first disconnect of the new
        // generation look like a second idle kick and stop a session that a real
        // network blip had merely interrupted.
        *idle_warning = super::IdleWarning::None;

        // Every stream buffer, and any half-assembled line. The login burst
        // re-sends the room and the inventory; a `thoughts` buffer from the
        // previous connection is a DIFFERENT SESSION'S chatter, and a pending
        // line is half of a sentence nobody will finish.
        streams.clear();
        pending.clear();

        // Experience, injuries, stance and encumbrance. MEASURED (`plan/15` §2a.4a):
        // NONE of the four dialogs is in the login burst -- they arrive only
        // after the first command, which is precisely §5.2's "invalidated" set.
        *character = super::Character::default();

        // Containers. The login burst DOES re-send worn items, but not the
        // contents of every container, and a stale container mirror is exactly
        // what Lich's `Inventory.reset!` exists to drop on "a session reset /
        // reconnect" (`lib/common/inventory.rb:1014-1045`).
        *inventory = super::Inventory::default();

        // --- Kept: see the method docs -------------------------------------
        let _ = vitals;
        let _ = (unknown_tags, unknown_tag_counts);
    }
}
