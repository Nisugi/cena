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
/// parser already parses both into [`Amount`](cena_protocol::Amount), and
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
    /// Signed, for the reason [`Amount::current`](cena_protocol::Amount)
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
pub trait VitalsExt {
    /// One gauge by its wire id, for the bars with no named accessor.
    fn vital(&self, id: &str) -> Option<Vital>;

    /// Health.
    fn health(&self) -> Option<Vital> {
        self.vital("health")
    }

    /// Mana.
    fn mana(&self) -> Option<Vital> {
        self.vital("mana")
    }

    /// Stamina.
    fn stamina(&self) -> Option<Vital> {
        self.vital("stamina")
    }

    /// Spirit.
    fn spirit(&self) -> Option<Vital> {
        self.vital("spirit")
    }
}

impl VitalsExt for Vitals {
    fn vital(&self, id: &str) -> Option<Vital> {
        self.get(id).copied()
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
