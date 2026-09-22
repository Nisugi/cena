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
//! # What the burst actually carries, MEASURED
//!
//! Two captures, 2026-09-20, counted from `<app>` to the first client command:
//!
//! ```text
//! nav rm   left   right   spell   indicator x10   compDef   crtrStatus
//! dialogData: expr, injuries, minivitals, combat, espMasterData
//! progressBar x10   streamWindow x5   deleteContainer x4
//! ```
//!
//! **Absent: `compass`, `prompt`, `roundTime`.** That is the whole list.
//!
//! An earlier version of this table claimed the burst omitted the hands, the
//! indicators, `nav rm` and the `expr` dialog. All four are wrong, and they
//! were found one at a time over months, which is why the measurement above is
//! a single command over a whole burst rather than a list of remembered facts.
//!
//! # Two corrections from the author, and the second undoes the first
//!
//! > **AUTHOR, 2026-09-20:** *"The login burst is all the stuff needed to
//! > populate the ui on login. It doesn't mean delete stuff."*
//!
//! That retired the rule this file used to state -- *"clear what the burst
//! does not re-send"* -- which was a rule about rendering masquerading as a
//! rule about truth.
//!
//! > **AUTHOR, 2026-09-20:** *"actually time stops for 99.9% of things when
//! > you're offline, you can absorb experience extremely slowly if you enable
//! > that option, you can't really change rooms when you're logged off."*
//!
//! And that retired the rule I replaced it with. I had written *"could this
//! have changed while we were disconnected?"* and then answered it as though
//! the world kept running without the character in it: the table said *"a
//! character can be moved while disconnected"* and *"spells tick down in real
//! time"*. **Neither is true.** A logged-off character is out of the world.
//! Time stops for essentially everything.
//!
//! # So the default is KEEP, and clearing is the exception
//!
//! | Fact | Kept? | Why |
//! |---|---|---|
//! | `room.id`, `description`, `exits` | **kept** | a logged-off character does not walk anywhere, and the place does not rearrange |
//! | `room.creatures`, `players`, `objects` | **cleared** | other people are still online; the roster moves while we are away, and each entry is a targetable `exist` id |
//! | `status`, `effects` | **kept** | durations do not run down while out of the world |
//! | hands | **kept** | nothing empties them, and the burst re-sends them anyway |
//! | `roundtime_ends` | kept | an absolute server epoch; unchanged by the socket |
//! | `vitals` | kept | re-sent in every burst |
//! | stats, identity | kept | taught by a command (step 9) |
//! | `game_time` | **cleared** | extrapolated from a LOCAL `Instant`, so a carried clock reports a server time in the future -- the reading is fine, the extrapolation is not |
//! | `idle_warning` | **cleared** | a fact about the connection that just ended, not about the character |
//! | `streams`, `pending`, `chunk` | **cleared** | half a sentence nobody will finish; the next connection's bytes are not a continuation |
//! | `stream_windows` | **cleared** | MEASURED: 15 of 16 declarations arrive before the first prompt, so the burst re-teaches them; and a stale `ifClosed` drops text rather than showing it stale |
//! | `inventory` | **cleared** | see its own comment: container CONTENTS are not re-sent, and Lich drops them for the same reason |
//! | `inventory_snapshot` | kept | a logged-off character gains and loses nothing; it is not in the login burst, and it is point-in-time by contract either way |
//! | `learned_commands` | kept | what a menu coordinate MEANS is a fact about the game, and the push is not repeated on reconnect |
//!
//! The three that remain cleared have nothing to do with elapsed game time.
//! Two are facts about the dead connection, and one is a local clock that
//! cannot be extrapolated across a gap.
//!
//! # The experience exception, stated because the author named it
//!
//! *"you can absorb experience extremely slowly if you enable that option"* --
//! so the `expr` dialog is the one thing that genuinely can move while
//! offline. It does not need special handling: the burst carries
//! `dialogData id='expr'` with the current level and mind state, so the
//! authoritative value arrives unprompted and overwrites whatever was kept.
//! Keeping it is safe for the same reason keeping the room is -- the server
//! corrects it immediately.
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

use super::GameState;
use super::Group;

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
            arrivals,
            cooldowns,
            messages,
            overwatch,
            known_spells,
            bounty,
            targeting,
            maneuvers,
            cast_time_ends,
            prompt,
            left_hand,
            right_hand,
            roundtime_ends,
            vitals,
            objectives,
            status,
            effects,
            game_time,
            game_time_received,
            unknown_tags,
            unknown_tag_counts,
            idle_warning,
            streams,
            stream_windows,
            tally,
            combat,
            creatures,
            pending,
            chunk,
            character,
            inventory,
            inventory_snapshot,
            group,
            containers,
            bank,
            learned_commands,
        } = self;

        // --- KEPT: a logged-off character is out of the world -------------
        //
        // Time stops offline (author, 2026-09-20), so none of these can have
        // moved on without us. Each is also re-sent by the burst, so keeping a
        // stale one is corrected within milliseconds anyway -- but the reason
        // to keep is that it is still TRUE, not that it gets overwritten.

        // **The PLACE is kept; who else is standing in it is not.** This is
        // the one field where the corrected rule splits a struct.
        //
        // `id`, `description` and `exits` describe somewhere that cannot have
        // changed: a logged-off character does not walk anywhere, and the room
        // itself does not rearrange. The burst re-sends them regardless
        // (`<nav rm>`, `compDef id='room desc'`).
        //
        // `creatures`, `players` and `objects` are the opposite. **Other
        // people are still online**, so the roster moves whether or not we are
        // watching -- and each entry carries an `exist` id a behavior can
        // target. A stale handle to a creature that wandered off is not a
        // cosmetic error; it is a live reference to something that is not
        // there. Found by `noun_resolution.rs`, which failed when this kept
        // the room whole.
        room.forget_contents();

        // **The group is cleared, for the same reason `room.players` is.** Its
        // members are other people, each held by an `exist` id a behavior can
        // target, and they leave while we are gone. A stale member is a live
        // handle to someone who is not there.
        //
        // Lich reaches the same answer from the other direction: `Group.check`
        // CLEARS and re-runs the `group` command (`gemstone/group.rb:152-157`)
        // rather than trusting what it held, and `<indicator id='IconJOINED'>`
        // -- which `GameState::apply` already stores -- says whether you are
        // grouped at all. So the burst answers "are you in a group" and only a
        // command answers "with whom".
        *group = Group::default();

        // **The stow and ready lists are KEPT**, and the contrast with the
        // group above is the point. Both hold `exist` ids, so the shallow
        // rule -- "an id is a live handle, clear it" -- would clear these too.
        //
        // What differs is whose the ids are. A group member is another player
        // who leaves while we are gone. A stow container is YOUR backpack, and
        // it is still on your back; the list itself is a per-character setting
        // the SERVER holds, which is why `stow list` restates it rather than
        // rebuilding it. Nothing about a dropped socket changes either.
        //
        // The failure this avoids is silent: neither list is re-sent by the
        // login burst -- only `stow list` and `ready list` teach them -- so
        // clearing here would leave a behavior with no stow container and no
        // event ever coming to restore one.
        let _ = containers;

        // **The bank balance is KEPT.** Silver on deposit is not connection
        // state: nobody spends it while we are logged off, and the figure is
        // re-read only by a `bank account` command, never by the burst. The
        // same reasoning as the two lists above, and the same failure avoided
        // -- clearing would leave a behavior believing the account empty with
        // no event coming to correct it.
        let _ = bank;

        // **The arrival counter is KEPT, and it is not a game fact.** It
        // counts rooms this SESSION has entered, so resetting it would make
        // the first arrival after a reconnect compare equal to a count taken
        // before -- exactly the false "did not move" it exists to prevent.
        //
        // It wraps, so growing across generations costs nothing.
        let _ = arrivals;

        // **Spell cooldowns are CLEARED**, and the contrast with the two
        // lists and the bank balance above is the point. Those are facts
        // about you that nothing changes while you are gone. These are stamps
        // on OTHER PEOPLE, taken against a clock this session was keeping --
        // and both assumptions break at once: the members wander off, and
        // `game_time_now` restarts from whatever the new connection reports.
        //
        // Keeping them would have a behavior skip a groupmate who is long
        // since castable, which is the quiet failure: nothing looks wrong, a
        // buff just never lands.
        cooldowns.clear();

        // **Messages are KEPT**, and the contrast with the cooldowns
        // immediately above is worth stating. A cooldown is a CLAIM ABOUT NOW
        // -- "this person cannot receive that spell yet" -- and it stops being
        // true while we are gone. A message is a record that someone said
        // something at a time when we were listening, and that stays true
        // forever.
        //
        // Nothing re-sends them, either: the burst carries no history. So
        // clearing would lose the only copy of something already observed,
        // which is the opposite of what invalidation is for.
        let _ = messages;

        // **The hidden-creature room is CLEARED**, with the cooldowns and for
        // the same reason: it is a claim about NOW -- "something is hiding in
        // room 7503251" -- and a creature does not wait there while we are
        // logged off. `Room::forget_contents` above already drops the
        // creatures we could see; this drops the one we could not.
        //
        // Lich reaches the same answer from the other side: `Overwatch.clear`
        // exists precisely so the tracker can be reset when it may be stale
        // (`overwatch.rb:24`).
        overwatch.clear();

        // The spell list. The login burst re-sends it whole, and until it
        // does "is 101 known" must answer "not told", not the last answer.
        known_spells.clear();

        // The bounty. The guild re-states it on `bounty`, and nothing sends it
        // unasked -- so a stored answer could only ever be staler than asking.
        bounty.clear();

        // What you can attack, and any cast in flight. Both are about a world
        // this character has left: the login burst re-sends the `combat`
        // dialog, and a cast cannot survive the disconnect. `roundtime_ends`
        // is KEPT beside them for the reason recorded above -- the server
        // enforces it across the gap -- and a cast time is not that: it is one
        // spell's preparation, and the spell is gone.
        targeting.clear();
        // A cooldown with no known duration cannot be reasoned about across a
        // gap of unknown length (`maneuvers.rs`).
        maneuvers.clear();
        *cast_time_ends = None;

        // The hands. Nothing empties them because a socket dropped, and the
        // burst sends real contents -- `<left exist=...>plain gift`,
        // `<right>Empty` -- not placeholders.
        let _ = (left_hand, right_hand);

        // Indicators and spell effects. This used to clear both, citing
        // "spells tick down in real time". They do not tick while the
        // character is out of the world, and the burst re-declares all ten
        // indicators in one line anyway.
        let _ = effects;

        // **Indicators are kept; the text-derived statuses are NOT.** The
        // reasoning above is the burst re-declaring all ten indicators in one
        // line -- and it does, which is why `status` was kept whole. It does
        // not hold for `afflictions.rs`'s six: NO indicator exists for
        // `silenced`, `bound`, `calmed`, `cutthroat`, `sleeping` or `thorned`,
        // so nothing re-states them and a kept belief is a belief nothing can
        // ever correct. A character silenced when the socket dropped would read
        // silenced forever.
        //
        // Found by a test that asserted the whole map was cleared, which was
        // wrong in the other direction -- the indicators SHOULD survive.
        for id in crate::state::afflictions::Affliction::ALL {
            status.forget(id.id());
        }

        // The quest and bounty list. A logged-off character completes no
        // quests and is offered none, and the list is not in the login burst
        // -- the game sends it when something changes or when asked. Clearing
        // it would leave `is_known()` false with no way back until the next
        // change, which is worse than a stale entry and also less true.
        let _ = objectives;

        // The whole-inventory snapshot. A logged-off character neither gains
        // nor loses items, so the tree is as true after the reconnect as
        // before -- and it is NOT in the login burst, so clearing it would
        // leave `is_known()` false until the user asked again by hand. The
        // freshness contract already says this is point-in-time rather than
        // live (`inventory_snapshot.rs`), which is what makes keeping it
        // honest: a consumer that needs current data re-requests, reconnect
        // or no reconnect.
        //
        // Contrast `inventory`, the passive container model, which IS
        // cleared: it mirrors windows the server reopens on login.
        let _ = inventory_snapshot;

        // Dictionary rows the server taught us. A fact about the GAME -- what
        // the coordinate 2524,12785 means -- not about the connection, and
        // the push is not repeated on reconnect: it arrives when the server
        // decides the client is behind. Clearing it would silently drop back
        // to the shipped table's older copy of a row the server has since
        // changed.
        let _ = learned_commands;

        // An absolute server epoch: a roundtime that ends at server second N
        // ends at N whether or not the socket survived. §5.2 forbids reporting
        // `0` on no evidence, which is unchanged -- with no clock
        // `in_roundtime` returns `None`, and an expired roundtime compares as
        // expired rather than being asserted.
        let _ = roundtime_ends;

        // --- CLEARED: facts about the CONNECTION, not about the character ---
        //
        // Three things, none of which is about elapsed game time.

        // **Both halves of the clock.** `game_time_now` extrapolates from
        // `game_time_received.elapsed()`, a LOCAL `Instant`, so a clock
        // carried across a thirty-second reconnect reports a server time
        // thirty seconds in the future -- and `in_roundtime` compares against
        // it. The reading was true; the extrapolation is what breaks, and
        // keeping one half without the other would be worse than keeping both.
        *game_time = None;
        *game_time_received = None;

        // A fact about the CONNECTION that just ended. Carrying it across
        // would make the first disconnect of the new generation look like a
        // second idle kick and stop a session a network blip had merely
        // interrupted.
        *idle_warning = super::IdleWarning::None;

        // Every stream buffer and any half-assembled line. A `thoughts` buffer
        // from the previous connection is a DIFFERENT SESSION'S chatter, and a
        // pending line is half a sentence nobody will finish -- the next
        // connection's bytes are not its continuation.
        streams.clear();
        pending.clear();

        // **Cleared, and MEASURED to be safe.** A `streamWindow` declaration
        // looks like a durable fact about the game, but it is a fact about the
        // connection's UI negotiation, and the burst re-teaches it: of the 16
        // declared ids, **15 arrive before the first prompt** (measured on
        // `GSIV-Nisugi/2026-09-01_10-02-47.xml`, 19 tags in the first 55KB).
        //
        // The one that does not is `charprofile`, declared only when `profile`
        // is run -- which this method's own rule sends to the cleared side: a
        // command-taught declaration from a connection that ended says nothing
        // about the next one.
        //
        // Keeping them instead would be defensible and is still wrong: a stale
        // `ifClosed` decides whether text is DROPPED, so a wrong one loses game
        // text silently rather than showing something stale.
        *stream_windows = super::stream_windows::Windows::default();

        // A held cast or pre-flare belongs to a chunk the old connection
        // never finished; an assault bracket cannot outlive its fight.
        combat.invalidate_for_reconnect();
        // What combat did to a creature is still true of it; who is standing
        // in the room is not (the rule `room.forget_contents` states).
        creatures.invalidate_for_reconnect();

        // **The counters are NOT reset.** They describe the SESSION -- how
        // much text this process has routed and how much the scrollback
        // dropped -- not the connection, so they belong with `unknown_tags`
        // on the retained side for the same reason: a fact about our own
        // behaviour is most useful across a reconnect, not least.
        let _ = tally;

        // And the chunk those lines were accumulating into. A command whose
        // output was interrupted will never see its terminating prompt, so the
        // lines it did carry describe a report that cannot be completed.
        //
        // This is also the failure Lich handles badly and by accident: its
        // accumulator is guarded only by a mutex, so a disconnect mid-`info`
        // leaves the mutex locked and the hold array dirty, and the NEXT
        // block's start silently discards the rows while unlocking one level
        // too shallow (`infomon.rb:54`, `infomon/parser.rb:239`). Dropping it
        // explicitly here is the whole difference.
        *chunk = super::chunks::Chunk::default();

        // The prompt, which the burst does NOT re-send (it is one of the three
        // absent tags, with `compass` and `roundTime`). It is a rendering of
        // the connection's last moment rather than a fact about the character.
        *prompt = None;

        // The character model, per group (M3 step 9). The four dialogs are in
        // the burst after all -- `expr`, `injuries` -- so this is no longer
        // about absence either; see `Character::invalidate_for_reconnect`.
        character.invalidate_for_reconnect();

        // Containers. The burst sends `deleteContainer` and re-declares worn
        // items, but NOT the contents of every container, so a stale container
        // mirror is the one inventory fact that survives without being
        // corrected. Lich's `Inventory.reset!` exists for exactly this
        // (`lib/common/inventory.rb:1014-1045`).
        *inventory = super::Inventory::default();

        // --- Kept: see the method docs -------------------------------------
        let _ = vitals;
        let _ = (unknown_tags, unknown_tag_counts);
    }
}
