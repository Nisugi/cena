//! The vitals gauges: health, mana, stamina, spirit.
//!
//! Split from `state.rs` under Rule 4.1 (*move code down, do not raise the
//! cap*) when [`Vital`] gained its two numbers and pushed that file past 550.

/// Every vitals gauge, by bar id.
///
/// `BTreeMap`, not `HashMap`, and that is load-bearing for criterion 7: a
/// `HashMap`'s iteration order varies run to run, so a replay that asserted
/// over one would be non-deterministic by construction. Enforced by
/// `crates/cena-session/tests/replay_determinism.rs`, not by an arch rule --
/// there is no `BTreeMap` rule, and this used to claim there was.
pub type Vitals = std::collections::BTreeMap<String, Vital>;

/// One gauge: what the wire said, all of it.
///
/// **This used to be a bare `u32` percent**, which threw away the two
/// numbers a consumer actually wants. `<progressBar id='health' value='95'
/// text='health 213/223'/>` carries the current value and the maximum, the
/// parser already parses both into [`Amount`](cena_protocol::frame::Amount), and
/// the model discarded them -- so "can I afford this spell" was unanswerable
/// from `GameState` and a Heal behavior could not see 213 of 223.
///
/// `current`/`max` stay `Option`: a label-only bar (`text='numbed'`, or
/// `mindState`) states no amount, and fabricating one is what
/// `payload::Amount`'s own doc refuses. Percent is always present because
/// `value=` always is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Vital {
    /// `value=`: the game's own percentage, NOT recomputed from the pair.
    ///
    /// The wire rounds, so `current`/`max` need not reproduce it exactly.
    /// Whichever the game sent is the one a bar should render.
    pub percent: u32,
    /// Current, when the bar's text states a pair.
    ///
    /// Signed, for the reason [`Amount`](cena_protocol::frame::Amount)'s own `current`
    /// gives: negative health is real and is the interesting case.
    pub current: Option<i32>,
    /// Maximum, when the bar's text states a pair.
    pub max: Option<i32>,
}

impl Vital {
    /// A gauge that stated only a percentage.
    #[must_use]
    pub const fn percent_only(percent: u32) -> Self {
        Self {
            percent,
            current: None,
            max: None,
        }
    }

    /// `current` and `max` together, when the bar stated both.
    #[must_use]
    pub const fn amount(self) -> Option<(i32, i32)> {
        match (self.current, self.max) {
            (Some(current), Some(max)) => Some((current, max)),
            _ => None,
        }
    }
}

/// The four gauges a consumer asks for by name.
///
/// **Named, because a magic string is not an API.** Reading a vital meant
/// `state.vitals.get("health")`, so every caller had to know the wire's own
/// spelling and a typo returned `None` rather than failing to compile. The
/// ids are the wire's (`<progressBar id='health'>`), VERIFIED against the
/// golden corpus:
///
/// ```sh
/// grep -o "progressBar id='[a-z]*'" crates/cena-protocol/tests/fixtures/*.xml \
///   | sort | uniq -c
/// # health 6, mana 7, stamina 7, spirit 3, health2 5 (a GROUP MEMBER's --
/// # see tests/vitals_are_the_players.rs), plus mindState/nextLvlPB/
/// # pbarStance/encumlevel, which are the character model's, not vitals'.
/// ```
///
/// **Inherent on `GameState`, not a trait on the map.** These were
/// `VitalsExt`, an extension trait with exactly one implementor -- the
/// `BTreeMap` alias -- which `plan/05` §-1 rules out. A newtype would have
/// kept `state.vitals.health()` but broken the ~30 sites that use the map as
/// a map; the gauges are the character's, so the character answers.
impl super::GameState {
    /// One gauge by its wire id, for the bars with no named accessor.
    #[must_use]
    pub fn vital(&self, id: &str) -> Option<Vital> {
        self.vitals.get(id).copied()
    }

    /// Health.
    #[must_use]
    pub fn health(&self) -> Option<Vital> {
        self.vital("health")
    }

    /// Mana.
    #[must_use]
    pub fn mana(&self) -> Option<Vital> {
        self.vital("mana")
    }

    /// Stamina.
    #[must_use]
    pub fn stamina(&self) -> Option<Vital> {
        self.vital("stamina")
    }

    /// Spirit.
    #[must_use]
    pub fn spirit(&self) -> Option<Vital> {
        self.vital("spirit")
    }
}

impl super::GameState {
    /// File one `<progressBar>` as a gauge, unless it is someone else's.
    ///
    /// An appraisal opens `<dialogData id="injuries-{existID}">` carrying its
    /// own `health2` bar, so keying on `bar.id` alone let an appraised
    /// target's health overwrite the character's
    /// (`tests/vitals_are_the_players.rs`).
    pub(super) fn record_vital(&mut self, bar: &cena_protocol::frame::ProgressBar) {
        let is_own = bar
            .dialog
            .as_deref()
            .is_none_or(|d| !d.starts_with("injuries-"));
        if is_own {
            self.vitals.insert(
                bar.id.clone(),
                Vital {
                    percent: bar.percent,
                    current: bar.amount.map(|a| a.current),
                    max: bar.amount.map(|a| a.max),
                },
            );
        }
    }
}

impl crate::GameState {
    /// Apply one `<progressBar>`.
    ///
    /// **Here rather than in `state.rs`'s `apply` under Rule 4.1** -- move code
    /// down, do not raise the cap. This arm was 84 lines of the dispatcher's
    /// 105, which tripped clippy's `too_many_lines`; the bars are vitals, and
    /// this is the module that owns them.
    pub(super) fn apply_progress_bar(&mut self, bar: &cena_protocol::frame::ProgressBar) {
        // ONLY the player's own bars. `plan/12` §7.1 scopes this to
        // the character's vitals, and `<progressBar>` is also how the
        // game ships OTHER creatures' health: an appraisal opens
        // `<dialogData id="injuries-{existID}">` carrying its own
        // `health2` bar (wiki `:243`). Keying on `bar.id` alone let a
        // target's health overwrite the player's -- the model would
        // report the character at 12% because something they appraised
        // was.
        //
        // The parser already distinguishes them (`bar.dialog` carries
        // the enclosing `dialogData` id); this is the model choosing
        // to keep that. `minivitals` and `injuries` are the player;
        // anything suffixed `injuries-<id>` is a third party and is
        // published to observers without entering the character's
        // state.
        // EFFECTS FIRST. The same `<progressBar>` shape carries
        // vitals, stance and effects; the enclosing dialog id is the
        // only thing that tells them apart (MEASURED 2026-09-18: one
        // burst carried `minivitals`, `combat`, `stance`, `Buffs`,
        // `Cooldowns` and `Active Spells` bars, all as progressBars).
        if let Some(dialog) = bar.dialog.as_deref()
            && crate::effects::is_effect_dialog(dialog)
        {
            // `time_remaining_secs` is a DURATION -- the wire sends
            // `time='00:01:59'`. Adding it to the server clock once,
            // here, is what makes it comparable later; the duration
            // itself goes stale immediately because the game only
            // re-sends an effect when it changes.
            //
            // **`game_time`, not `game_time_now()`.**
            //
            // `game_time_now()` extrapolates: it adds
            // `game_time_received.elapsed()`, a reading of the LOCAL
            // monotonic clock. Baking that into `ends_at` put a local
            // measurement inside a value `PartialEq` compares -- and
            // `game_time_received` is destructured to `_` in that impl
            // specifically to keep local readings out of it. The
            // exclusion was correct and was being routed around
            // through this field (review MO-1).
            //
            // The cost was replay equality. Live, a refill arriving 90
            // seconds after its prompt gave `base + 90 + secs`;
            // replayed from the same recording, the frames arrive
            // back-to-back and give `base + secs`. Same bytes,
            // unequal state -- against criterion 7, which is what M2's
            // golden corpus rests on.
            //
            // Using the raw server clock needs no local reading at
            // all: the server sent the time and the duration, and
            // their sum is what the server said. The extrapolation
            // was never adding information, only the delay between
            // two frames the game sent together.
            //
            // `game_time_now()` remains right for READING the clock --
            // `in_roundtime` needs to know what time it is now. It is
            // wrong for STAMPING a fact the server already dated.
            //
            // **And no clock is not "no end".** Before the first prompt of a
            // connection `game_time` is `None`, and this used to store
            // `ends_at: None` -- the spelling for an indefinite effect -- so
            // every buff in the login burst read as permanent (review).
            // `insert_lasting` holds the duration until a prompt anchors it.
            let effect = crate::effects::Effect {
                category: dialog.to_owned(),
                text: bar.text.clone(),
                ends_at: None,
                percent: bar.percent,
            };
            match bar.time_remaining_secs {
                Some(secs) => {
                    self.effects
                        .insert_lasting(bar.id.clone(), effect, secs, self.game_time);
                }
                None => self.effects.insert(bar.id.clone(), effect),
            }
            return;
        }
        // Step 3's dialogs, before vitals and for the same reason
        // effects come before both: one `<progressBar>` shape carries
        // gauges, stance, encumbrance and advancement, and the enclosing
        // dialog is the only thing that tells them apart.
        if let Some(dialog) = bar.dialog.as_deref()
            && self
                .character
                .apply_bar(dialog, &bar.id, &bar.text, bar.percent)
        {
            return;
        }
        self.record_vital(bar);
    }
}
