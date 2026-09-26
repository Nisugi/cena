//! Active effects: spells, buffs, debuffs and cooldowns, with when they end.
//!
//! # Liveness is an EXPIRY, not a presence
//!
//! The obvious model -- "it is in the list, so it is active" -- is wrong, and
//! both references say so independently:
//!
//! ```ruby
//! # reference/lich-5/lib/gemstone/effects.rb
//! def active?(effect)
//!   expiration(effect).to_f > Time.now.to_f
//! end
//! ```
//!
//! ```rust,ignore
//! // reference/VellumFE/src/data/widget.rs
//! pub fn remaining_seconds(&self, now_server: i64) -> Option<i64> {
//!     self.expires_at.map(|at| (at - now_server).max(0))
//! }
//! ```
//!
//! Vellum's comment says why: *"The protocol only re-sends effects on change,
//! so the `time` string goes stale."* A quiet minute means no update, and an
//! effect whose timer has run out is still sitting in the dialog. Treating
//! presence as liveness would report it as active.
//!
//! So an entry stores an **absolute end time in server seconds**, and
//! [`Effects::active`] compares it against
//! [`GameState::game_time_now`](crate::GameState::game_time_now). That is the
//! same comparison `in_roundtime` makes, against the same clock, which is why
//! the clock had to be built first.
//!
//! # One collection, not four
//!
//! > **AUTHOR, 2026-09-18:** *"I would expect our parsing of the buff would add
//! > it to the buffs that are already recorded. It's all the same thing so the
//! > information should be together."*
//!
//! Said about the *source* of an entry -- a dialog refill versus text this
//! client parsed itself -- and it applies just as well to the category. A
//! behavior asking "is 515 up?" must not have to know whether the game files
//! it under `Buffs` or `Active Spells`. The category is a **field**, so it can
//! be filtered on, never a separate collection to search.
//!
//! MEASURED (2026-09-18) that this matters: casting 515 put **`Rapid Fire` in
//! `Buffs` and `Rapid Fire Recovery` in `Cooldowns` simultaneously**, under
//! different ids. One lookup finds both.
//!
//! # Ids are spell numbers
//!
//! MEASURED: `515` Rapid Fire, `101` Spirit Warding I, `215` Heroism, `601`
//! Natural Colors. So confirming an instant action fired is an **id lookup**,
//! not a text match -- which is what makes `plan/16`'s confirm-by-effect
//! reliable rather than a string-matching exercise.
//!
//! Not every id is a spell number: the same capture carried `37594784` and
//! `199554666`. They are kept verbatim; nothing here parses an id.

use std::collections::{BTreeMap, BTreeSet};

/// One active effect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Effect {
    /// Which dialog it arrived in: `Buffs`, `Debuffs`, `Cooldowns`,
    /// `Active Spells`.
    pub category: String,
    /// Display name, e.g. `"Rapid Fire"`.
    pub text: String,
    /// The absolute server second this ends, when the wire gave a duration.
    ///
    /// **Derived on arrival**, not read from the wire: the game sends
    /// `time='00:01:59'`, a *duration*, which is stale the moment it lands.
    /// Adding it to the server clock once, at arrival, is what keeps it
    /// comparable later -- Vellum's `expires_at`, and the reason its comment
    /// exists.
    ///
    /// `None` for an effect with no parseable duration. Vellum notes
    /// "Indefinite" and stack counts; those never tick, and an effect that
    /// never ticks must not be treated as expired.
    ///
    /// **Also `None`, briefly, for a timed effect with no clock to stamp it
    /// against** -- one that arrived before the first `<prompt>` of a
    /// connection, or was kept across a reconnect. Its duration is then held
    /// by [`Effects`] and anchored on the next prompt; until then read it
    /// through [`Effects::active`] / [`Effects::remaining`], which know the
    /// difference. Reading this field alone would take a 2-minute buff for an
    /// indefinite one (review).
    pub ends_at: Option<u32>,
    /// The bar percentage the game last reported, 0-100.
    ///
    /// `u32` to match `ProgressBar::percent`, which the parser widened
    /// deliberately: the wire is not guaranteed to stay inside 0-100, and a
    /// narrowing cast at the boundary would be a silent truncation.
    pub percent: u32,
}

/// Every effect the game has reported, keyed by its wire id.
///
/// `BTreeMap`, not `HashMap`: iteration order is part of criterion 7's
/// determinism.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Effects {
    /// Keyed by **wire id alone**, not by `(category, id)`.
    ///
    /// # The risk, and what measurement said about it
    ///
    /// Lich keys per dialog (`xmlparser.rb`, `@dialogs[kind][id]`). If one
    /// spell were listed in two dialogs, a refill of one would re-tag it and
    /// `in_category` would omit it from the other, though that dialog was
    /// never cleared (review MO-2).
    ///
    /// MEASURED over a full live session
    /// (`2026-09-01_16-30-27.xml`, 8.9 MB, the author's own capture):
    ///
    /// ```text
    /// Active Spells : 29 ids
    /// Buffs         : 10 ids
    /// Debuffs       :  0
    /// Cooldowns     :  0
    /// overlap, any pair: NONE
    /// ```
    ///
    /// **AUTHOR, 2026-09-19:** *"there's no overlap on the effects, if it
    /// shows in both it would be something buff + cooldown"*.
    ///
    /// That names the one real overlap, and MEASURED in
    /// `2026-09-04_09-15-47.xml` it is **not an id collision**:
    ///
    /// ```text
    /// Buffs      605       "Barkskin"
    /// Cooldowns  19032922  "Barkskin"
    /// ```
    ///
    /// Same spell, two dialogs, **two different wire ids** -- the buff's
    /// spell number and the cooldown's own identifier. Two facts about one
    /// spell, filed separately by the game itself.
    ///
    /// So keying by id alone is safe *because the server already
    /// disambiguates*, and `(category, id)` would add a key component the
    /// wire has made redundant -- while forcing every lookup to supply a
    /// category its callers do not have (`active(id)`, `remaining(id)`).
    ///
    /// The review's scenario (MO-2) assumed a shared id across dialogs.
    /// Checked over two full sessions, no pair of dialogs shares one: 29/10
    /// in one capture, 20/13/1/9 in another, overlap NONE in both. If a
    /// shared id is ever observed, this is the line to change and those are
    /// the measurements to re-run.
    by_id: BTreeMap<String, Effect>,
    /// Categories the game has declared a COMPLETE list for, this generation.
    ///
    /// **Review MO-3.** Without this, `active()` answers `None` both for an
    /// effect nobody ever mentioned and for one the game has just said is
    /// gone -- and those are opposite facts. A rebuff behavior following
    /// §5.2's "unknown means ask, never assume" can then never learn that a
    /// spell dropped, because the answer never changes from `None`.
    ///
    /// The distinction is the same one [`Room::saw_players`] and
    /// [`PsmSet::has_table`] draw, and it needs recording for the same reason:
    /// an empty collection cannot say whether it is empty because nothing is
    /// there or because nobody looked.
    ///
    /// [`Room::saw_players`]: crate::Room::saw_players
    /// [`PsmSet::has_table`]: crate::PsmSet::has_table
    ///
    /// # Why `clear='t'` is the signal, and an ordinary refill is not
    ///
    /// MEASURED in a live capture: `<dialogData id='Buffs' clear='t'>` arrives
    /// **empty**, immediately followed by the populated element -- the game
    /// saying "forget what you had, here is everything". That is a complete
    /// list, and absence from it is meaningful.
    ///
    /// A `Buffs` element **without** `clear` is not. The same capture has
    /// `Buffs` arriving 10 times with only 5 clears, so the other 5 are
    /// incremental and an id missing from one of those says nothing at all.
    ///
    /// Scoped per category, because the four dialogs refill independently: a
    /// `Buffs` clear tells you nothing about `Cooldowns`.
    observed: BTreeSet<String>,
    /// Durations waiting for a server clock to anchor them, by id.
    ///
    /// # Why an effect can arrive with no clock
    ///
    /// `ends_at` is `prompt second + time=`, and the prompt is the only thing
    /// that teaches the clock (`GameState::apply`'s `Frame::Prompt` arm).
    /// The login burst sends the effect dialogs BEFORE its first prompt, and
    /// a reconnect deliberately forgets the clock (`reconnect.rs`). Either
    /// way an effect with `time='00:02:00'` arrived with nothing to add it
    /// to -- and was stored with `ends_at: None`, the spelling for
    /// *indefinite*. So a two-minute buff read as permanent: `active()` said
    /// `Some(true)` forever and `remaining()` said `None` (review).
    ///
    /// The duration is kept here instead, relative, and [`Self::anchor`]
    /// turns it absolute against the first prompt that arrives. No local
    /// clock is read, so replay equality (MO-1) holds.
    pending: BTreeMap<String, u32>,
}

impl Effects {
    /// Record or replace an effect.
    pub fn insert(&mut self, id: String, effect: Effect) {
        self.pending.remove(&id);
        self.by_id.insert(id, effect);
    }

    /// Record an effect the wire gave a duration, stamping it if the server
    /// clock is known and holding the duration for [`Self::anchor`] if not.
    ///
    /// `effect.ends_at` is overwritten either way: this is the one place the
    /// duration becomes an end time.
    pub fn insert_lasting(&mut self, id: String, mut effect: Effect, secs: u32, now: Option<u32>) {
        if let Some(now) = now {
            effect.ends_at = Some(now.saturating_add(secs));
            self.insert(id, effect);
        } else {
            effect.ends_at = None;
            self.by_id.insert(id.clone(), effect);
            self.pending.insert(id, secs);
        }
    }

    /// Give every held duration an end time, against the server second a
    /// prompt just stated. Called on every prompt; a no-op when nothing waits.
    pub fn anchor(&mut self, now: u32) {
        for (id, secs) in std::mem::take(&mut self.pending) {
            if let Some(effect) = self.by_id.get_mut(&id) {
                effect.ends_at = Some(now.saturating_add(secs));
            }
        }
    }

    /// Turn every end time back into a duration, measured at `then` -- the
    /// last server second this connection stated.
    ///
    /// For a reconnect. `ends_at` is an ABSOLUTE server second, and the
    /// server's clock keeps running while the character is logged off; the
    /// spell does not (author, 2026-09-20). Keeping the absolute value would
    /// charge the offline gap against every buff -- a 10-minute outage would
    /// expire a 5-minute spell that had not ticked at all. Holding what was
    /// LEFT, and anchoring it on the new connection's first prompt, is what
    /// "kept across a reconnect" has to mean for a timer.
    ///
    /// `then` is the raw prompt second, not the extrapolated clock: that
    /// reads a local `Instant`, which must stay out of compared state (MO-1).
    /// The cost is at most the seconds between the last prompt and the drop.
    pub(crate) fn unanchor(&mut self, then: u32) {
        for (id, effect) in &mut self.by_id {
            if let Some(ends) = effect.ends_at.take() {
                self.pending.insert(id.clone(), ends.saturating_sub(then));
            }
        }
    }

    /// Whether a listed effect is live at `now_server`, whichever way its
    /// time is held.
    fn live(&self, id: &str, effect: &Effect, now_server: u32) -> bool {
        match (effect.ends_at, self.pending.get(id)) {
            (Some(ends), _) => now_server < ends,
            // Stated with time left and not yet anchored: live, unless the
            // time it was stated with was already none.
            (None, Some(secs)) => *secs > 0,
            (None, None) => true,
        }
    }

    /// Drop every effect in one category, and record that the game is about
    /// to state the complete list.
    ///
    /// For `<dialogData id='Buffs' clear='t'>`, which MEASURED arrives as an
    /// **empty** element immediately before the populated one -- clear, then
    /// refill. Scoped to the category because the four dialogs refill
    /// independently: a `Buffs` refill says nothing about `Cooldowns`.
    ///
    /// **Marking the category observed is what fixes MO-3.** After this, an id
    /// absent from the category is absent because the game said so, which
    /// [`Self::active`] can report as `Some(false)` rather than `None`.
    pub fn clear_category(&mut self, category: &str) {
        self.by_id.retain(|_, e| e.category != category);
        let by_id = &self.by_id;
        self.pending.retain(|id, _| by_id.contains_key(id));
        self.observed.insert(category.to_owned());
    }

    /// Has the game declared a complete list for this category?
    ///
    /// True from the first `clear='t'` for it until the effects are
    /// invalidated. See [`Self::active_in`].
    #[must_use]
    pub fn saw_category(&self, category: &str) -> bool {
        self.observed.contains(category)
    }

    /// The effect with this id, whether or not it has expired.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Effect> {
        self.by_id.get(id)
    }

    /// Whether this effect is active **at `now_server`**.
    ///
    /// Returns `None` when the effect is not listed at all -- unknown, not
    /// "inactive" (`plan/12` §5.2). An effect with no end time is active while
    /// listed: it never ticks, so there is nothing to compare.
    ///
    /// **This cannot distinguish "gone" from "never seen"**, because it is not
    /// told which category to look in and an id it has never seen belongs to
    /// no category. [`Self::active_in`] is the one that can, and it is what a
    /// behavior deciding whether to rebuff should call (review MO-3).
    #[must_use]
    pub fn active(&self, id: &str, now_server: u32) -> Option<bool> {
        let effect = self.by_id.get(id)?;
        Some(self.live(id, effect, now_server))
    }

    /// Whether this effect is active, **knowing which list it would be in**.
    ///
    /// The MO-3 fix. Three answers instead of two:
    ///
    /// | | meaning |
    /// |---|---|
    /// | `Some(true)` | listed and not expired |
    /// | `Some(false)` | either listed and expired, **or** absent from a category the game has stated in full |
    /// | `None` | the category has never been stated -- genuinely unknown |
    ///
    /// The middle row is the one `active()` cannot produce. Without it, a
    /// rebuff behavior reading `None` as "ask again" never learns a spell
    /// dropped, and one reading `None` as `false` recasts live spells after a
    /// reconnect. Both failures are real and they pull in opposite directions,
    /// which is why the answer has to be three-valued rather than defaulted
    /// either way.
    ///
    /// The caller supplies the category because it knows which one it cares
    /// about: a rebuff behavior looking for spell 515 knows it would be in
    /// `Active Spells`, and no data structure can recover that for an id it
    /// has never seen.
    #[must_use]
    pub fn active_in(&self, category: &str, id: &str, now_server: u32) -> Option<bool> {
        if let Some(effect) = self.by_id.get(id) {
            // **Only answers for THIS category.** An id found under another
            // dialog says nothing about this one, and the categories refill
            // independently.
            if effect.category == category {
                return Some(self.live(id, effect, now_server));
            }
        }
        // Absent here. That is a fact only if the game has stated this
        // category in full.
        self.saw_category(category).then_some(false)
    }

    /// Whether an effect **named** `name` is active in `category`: the
    /// three answers of [`Self::active_in`], found by name rather than id.
    ///
    /// Lich's `normalize_lookup` (`lib/util/util.rb:27-32`), which
    /// `PSMS.available?` asks of `Cooldowns` and `Debuffs` (`psms.rb:142-145`):
    /// both sides folded, then compared whole. Several listed under one name
    /// answer `Some(true)` if any is live.
    ///
    /// **The folding goes one step past Lich's.** `normalize_lookup` lowercases
    /// and turns `:` and `_` into spaces; this also drops `'` and turns `-`
    /// into a space, which is what Lich's other folder, `normalize_name`
    /// (`util.rb:59-66`), does to the long names in its PSM tables. A long name
    /// has already lost its apostrophe, so the dialog's text must lose it too
    /// to meet it. VERIFIED the two meet for one row: the Cooldowns dialog
    /// lists `Volley` (`crates/cena-behavior/tests/fixtures/arch_kill.xml:155`), and
    /// the weapon table's long name is `volley` (`psms/weapon.rb:167`).
    #[must_use]
    pub fn active_named(&self, category: &str, name: &str, now_server: u32) -> Option<bool> {
        let name = fold_name(name);
        let mut found = self
            .in_category(category)
            .filter(|(_, effect)| fold_name(&effect.text) == name)
            .peekable();
        if found.peek().is_none() {
            return self.saw_category(category).then_some(false);
        }
        Some(found.any(|(id, effect)| self.live(id, effect, now_server)))
    }

    /// Seconds left on this effect at `now_server`, saturating at zero.
    ///
    /// `None` when the effect is unlisted or has no end time. A duration not
    /// yet anchored to a clock reports what the game stated, which is the
    /// best answer available and not `None` -- `None` means *indefinite*.
    #[must_use]
    pub fn remaining(&self, id: &str, now_server: u32) -> Option<u32> {
        match self.by_id.get(id)?.ends_at {
            Some(ends) => Some(ends.saturating_sub(now_server)),
            None => self.pending.get(id).copied(),
        }
    }

    /// Every effect, in id order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Effect)> {
        self.by_id.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Every effect in one category, in id order.
    pub fn in_category<'a>(
        &'a self,
        category: &'a str,
    ) -> impl Iterator<Item = (&'a str, &'a Effect)> {
        self.iter().filter(move |(_, e)| e.category == category)
    }

    /// How many effects are listed, expired or not.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Whether nothing is listed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// Forget everything, **including which categories were stated**.
    ///
    /// # This has NO CALLER, and that is deliberate as of 2026-09-20
    ///
    /// It was written for `plan/12` §5.2's reconnect path and called from
    /// `reconnect.rs`. That call is gone: effects are now **kept** across a
    /// generation, because a logged-off character is out of the world and
    /// spell durations do not run down while nobody is playing (author,
    /// 2026-09-20).
    ///
    /// Kept as API rather than deleted, because `observed` makes its contract
    /// worth stating: **both fields must go together**. Clearing `by_id` alone
    /// would leave every category marked observed with nothing in it, so
    /// [`Self::active_in`] would answer a confident `Some(false)` for every id
    /// in a list that had been emptied rather than restated -- the §5.2
    /// failure arriving through the very mechanism added to prevent it.
    ///
    /// If a future path does need to forget effects, this is the one to call.
    pub fn clear(&mut self) {
        self.by_id.clear();
        self.observed.clear();
        self.pending.clear();
    }
}

/// The four dialog ids that carry effects.
///
/// MEASURED 2026-09-18, all four in one burst. Lich names the same four
/// (`lib/gemstone/effects.rb`: `Registry.new("Active Spells")`, `"Buffs"`,
/// `"Debuffs"`, `"Cooldowns"`).
pub const EFFECT_DIALOGS: [&str; 4] = ["Active Spells", "Buffs", "Debuffs", "Cooldowns"];

/// An effect's name folded for comparison: [`Effects::active_named`].
fn fold_name(name: &str) -> String {
    name.to_lowercase()
        .replace('\'', "")
        .replace([':', '_', '-'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether a dialog id carries effects rather than something else.
///
/// The same `<progressBar>` shape carries vitals (`minivitals`), stance
/// (`combat`, `stance`) and another creature's health (`injuries-<id>`), so
/// the dialog id is the only thing that says which is which.
#[must_use]
pub fn is_effect_dialog(dialog: &str) -> bool {
    EFFECT_DIALOGS.contains(&dialog)
}
