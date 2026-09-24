//! Setting the combat stance: the sending half of Lich's `stance.rb`
//! (`reference/lich-5/lib/gemstone/stance.rb:78-160`), which `cena-model`'s
//! `character/stance.rs` left for M6 (`plan/20` §0c, `plan/30` §3).
//!
//! Pure: what to send, whether it took, and which stance is safest. The
//! sending itself is the driver's, through the action contract -- settle
//! roundtime and cast roundtime, check, send (`plan/30` §3).
//!
//! # Confirmed by the bar, not by the sentence
//!
//! Lich waits for one of four sentences (`ACCEPTED`, `DECLINED`). Here the
//! answer is the stance bar the game sends after any change --
//! `<progressBar id='pbarStance' ... text='guarded (80%)'/>`, typed state --
//! so [`landed`] asks the model rather than matching prose (`plan/12` §3a).
//! A refusal (*"Cast Roundtime in effect"*) leaves the bar where it was, and
//! reads as not landed.

use cena_session::{GameState, PsmCategory, Stance};

/// What a player asked for: a stance by name, or a percent to defense.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Want {
    /// `stance guarded`: anywhere in the named stance's band.
    Named(Stance),
    /// `stance 80`: exactly this percent -- only with Stance Perfection, and
    /// otherwise the stance whose band holds it.
    Percent(u32),
}

impl Want {
    /// Read `guarded`, `gua`, or `80`.
    ///
    /// A percent must be a multiple of ten from 0 to 100: Lich's
    /// `normalize_percent` (`stance.rb:163-170`) refuses anything else, and
    /// the rule is about what may be **asked for**. The reader accepts any
    /// percent because the game can knock a stance off a round number.
    ///
    /// # Errors
    ///
    /// Neither a known stance (three letters or more) nor such a percent.
    pub fn parse(text: &str) -> Result<Self, String> {
        let text = text.trim();
        if let Ok(percent) = text.parse::<u32>() {
            return if percent <= 100 && percent % 10 == 0 {
                Ok(Self::Percent(percent))
            } else {
                Err(format!(
                    "a stance percent is a multiple of ten from 0 to 100, not {percent}"
                ))
            };
        }
        Stance::parse(text)
            .map(Self::Named)
            .ok_or_else(|| format!("{text:?} is not a stance"))
    }

    /// The stance this lands in.
    #[must_use]
    pub fn stance(self) -> Stance {
        match self {
            Self::Named(stance) => stance,
            // Every percent 0..=100 is in one band (`Stance::from_percent`),
            // and `parse` admits no other.
            Self::Percent(percent) => Stance::from_percent(percent).unwrap_or(Stance::Defensive),
        }
    }
}

/// Whether the character is where `want` asks: the named stance, or the
/// exact percent. What confirms a change, and what makes one unnecessary.
#[must_use]
pub fn landed(want: Want, state: &GameState) -> bool {
    match want {
        Want::Named(stance) => state.character.stance_typed() == Some(stance),
        Want::Percent(percent) => state.character.stance_percent == Some(percent),
    }
}

/// The command that gets there, or `None` when there is nothing to send:
/// already there, or dead (`stance.rb`'s `change` returns early for both).
///
/// A percent is sent as `cman stance N` when Stance Perfection is trained,
/// which sets it exactly; otherwise as its band's name.
#[must_use]
pub fn command(want: Want, state: &GameState) -> Option<String> {
    if landed(want, state) || state.status.dead() {
        return None;
    }
    Some(match want {
        Want::Percent(percent) if perfection(state) => format!("cman stance {percent}"),
        _ => format!("stance {}", want.stance().as_str()),
    })
}

/// Whether Stance Perfection is trained (`CMan.known?('stance_perfection')`,
/// whose mnemonic -- Lich's `short_name` -- is `stance`, `psms/cman.rb:504`).
#[must_use]
pub fn perfection(state: &GameState) -> bool {
    state
        .character
        .psms
        .get(PsmCategory::CombatManeuver, "stance")
        .is_some_and(|psm| psm.ranks > 0)
}

/// The most defensive stance that can be taken now: `defensive`, or
/// `guarded` while a cast roundtime runs (`stance.rb`'s `safest`), when
/// `defensive` is refused.
///
/// `guarded` when the clock is unknown too: it cannot be shown that no cast
/// roundtime runs, and `guarded` is taken either way (`plan/12` §5.2).
#[must_use]
pub fn safest(state: &GameState) -> Stance {
    if state.in_casttime() == Some(false) {
        Stance::Defensive
    } else {
        Stance::Guarded
    }
}
