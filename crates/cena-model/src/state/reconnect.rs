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
//! | room (`compDef`), vitals `progressBar`s, `playerID`, `inv`, layout, **hands**, **`spell`**, **all ten `indicator`s** | `nav rm`, `compass`, `prompt`, `roundTime`, the four effect dialogs |
//!
//! # The rule this file used to state was WRONG, and the author corrected it
//!
//! > **AUTHOR, 2026-09-20:** *"The login burst is all the stuff needed to
//! > populate the ui on login. It doesn't mean delete stuff."*
//!
//! This file said, in as many words: **"clear what the burst does not
//! re-send."** That is not a rule about truth, it is a rule about what a fresh
//! client needs to draw its windows -- and those are different questions. The
//! burst is a UI population message. Absence from it is not evidence that a
//! fact has stopped being true.
//!
//! **The right question is: could this have changed while we were away, and
//! would a stale value mislead?**
//!
//! | Fact | Cleared | Because |
//! |---|---|---|
//! | `room` | yes | a character can be moved while disconnected |
//! | `status`, `effects` | yes | spells tick down in real time; a stale `stunned` is exactly the belief §5.2 forbids |
//! | `game_time` | yes | extrapolated from a local `Instant`, so a carried clock reports a server time in the future |
//! | `idle_warning` | yes | a fact about the connection that just ended |
//! | `streams`, `pending`, `chunk` | yes | half a sentence nobody will finish |
//! | hands | yes | **and the old reason was false** -- see below |
//! | `roundtime_ends` | no | an absolute server epoch; it ends when it ends |
//! | `vitals` | no | re-sent, and clearing opens a window where health reads `Unknown` |
//! | stats, identity | no | taught by a command; a reconnect does not change Strength |
//!
//! # Two measurements of the same capture were wrong the same way
//!
//! The table above used to put **hands** and **`indicator`** in the Absent
//! column. Both are wrong, and both were caught separately rather than as one
//! error:
//!
//! * `indicator` was corrected earlier (review MO-12) when `status.rs`'s
//!   header contradicted it -- *"all ten of Lich's `ICONMAP` ids on a single
//!   line"*.
//! * **hands** stood until 2026-09-20. MEASURED in two independent captures,
//!   the burst carries real contents rather than placeholders:
//!   `<left exist="364757584" noun="gift">plain gift` and `<right>Empty`,
//!   beside `<spell>None` and the ten indicators.
//!
//! Clearing the hands is still harmless -- the burst refills them in the same
//! breath -- but it was being done for a stated reason that is false, and the
//! **rule** derived from those two errors was load-bearing for every field in
//! this file. That is the cost of a proxy: "is it in the burst" was easy to
//! measure and answered a question nobody had asked.
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
    /// Called between generations, never during one. What survives is what
    /// **cannot have changed while we were away**, or what a command taught.
    /// See the module docs: "in the login burst" is NOT the test, and this
    /// method used to say it was.
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
            chunk,
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

        // **The hands, and the reason is not the one this file used to give.**
        // The old table put them in the "absent from the burst" column;
        // MEASURED 2026-09-20 in two captures, that is false -- the burst
        // sends `<left exist=... >plain gift` and `<right>Empty`, real
        // contents rather than placeholders.
        //
        // Cleared anyway, and only because the burst refills them immediately:
        // a character's hands do not empty because a socket dropped, so the
        // honest position is that this clear is POINTLESS rather than
        // necessary (`plan/12` §5.2's own word for a field the burst refills).
        // It is kept because "clear, then be told" has one ordering and
        // "keep, and hope the burst agrees" has two, and the difference costs
        // nothing here.
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

        // And the chunk those lines were accumulating into. A command whose
        // output was interrupted by the drop will never see its terminating
        // prompt, so the lines it did carry describe a report that cannot be
        // completed -- the same argument as `pending`, one level up.
        //
        // This is also the failure Lich handles badly and by accident: its
        // accumulator is guarded only by a mutex, so a disconnect mid-`info`
        // leaves the mutex locked and the hold array dirty, and the NEXT
        // block's start silently discards the rows while unlocking one level
        // too shallow (`infomon.rb:54`, `infomon/parser.rb:239`). Dropping it
        // explicitly here is the whole difference.
        *chunk = super::chunks::Chunk::default();

        // **PER GROUP, not wholesale.** This was
        // `*character = Character::default()`, which was right when the struct
        // held only the four dialogs -- experience, injuries, stance,
        // encumbrance, none of them in the burst (MEASURED, `plan/15` §2a.4a).
        //
        // M3 added `stats` and `identity`, and they are absent from the burst
        // for the OPPOSITE reason: not "unobserved after a reconnect" but
        // "never volunteered at all". They were taught by an `info` a person
        // typed, and nothing about reconnecting changes a character's
        // Strength. Wiping them would blank the character until someone
        // retyped the command.
        //
        // The test that a fact is invalidated is not "is it in the burst" but
        // "does the burst's silence mean anything". See
        // `Character::invalidate_for_reconnect` for the per-field split.
        character.invalidate_for_reconnect();

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
