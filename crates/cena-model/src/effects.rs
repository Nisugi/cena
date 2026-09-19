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

use std::collections::BTreeMap;

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
    by_id: BTreeMap<String, Effect>,
}

impl Effects {
    /// Record or replace an effect.
    pub fn insert(&mut self, id: String, effect: Effect) {
        self.by_id.insert(id, effect);
    }

    /// Drop every effect in one category.
    ///
    /// For `<dialogData id='Buffs' clear='t'>`, which MEASURED arrives as an
    /// **empty** element immediately before the populated one -- clear, then
    /// refill. Scoped to the category because the four dialogs refill
    /// independently: a `Buffs` refill says nothing about `Cooldowns`.
    pub fn clear_category(&mut self, category: &str) {
        self.by_id.retain(|_, e| e.category != category);
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
    #[must_use]
    pub fn active(&self, id: &str, now_server: u32) -> Option<bool> {
        let effect = self.by_id.get(id)?;
        Some(effect.ends_at.is_none_or(|ends| now_server < ends))
    }

    /// Seconds left on this effect at `now_server`, saturating at zero.
    ///
    /// `None` when the effect is unlisted or has no end time.
    #[must_use]
    pub fn remaining(&self, id: &str, now_server: u32) -> Option<u32> {
        Some(self.by_id.get(id)?.ends_at?.saturating_sub(now_server))
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

    /// Forget everything, for `plan/12` §5.2's reconnect path.
    pub fn clear(&mut self) {
        self.by_id.clear();
    }
}

/// The four dialog ids that carry effects.
///
/// MEASURED 2026-09-18, all four in one burst. Lich names the same four
/// (`lib/gemstone/effects.rb`: `Registry.new("Active Spells")`, `"Buffs"`,
/// `"Debuffs"`, `"Cooldowns"`).
pub const EFFECT_DIALOGS: [&str; 4] = ["Active Spells", "Buffs", "Debuffs", "Cooldowns"];

/// Whether a dialog id carries effects rather than something else.
///
/// The same `<progressBar>` shape carries vitals (`minivitals`), stance
/// (`combat`, `stance`) and another creature's health (`injuries-<id>`), so
/// the dialog id is the only thing that says which is which.
#[must_use]
pub fn is_effect_dialog(dialog: &str) -> bool {
    EFFECT_DIALOGS.contains(&dialog)
}
