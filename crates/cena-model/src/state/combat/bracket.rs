//! Brackets: the lines that open and close a multi-part action.
//!
//! Two kinds, ported from `defs/assaults.rb` and `defs/sequences.rb`, and the
//! distinction is the processor's (`assaults.rb:9-20`):
//!
//! - An **assault** is single-target, multi-round: flurry, barrage, pummel,
//!   guardant thrusts, thrash. The opener names the ONLY target the whole
//!   assault can strike; the rounds between usually name none; the attacker
//!   cannot act otherwise until the end line.
//! - A **sequence** is multi-target: mstrike, volley, and the spawned `AoE`
//!   casts (natures fury, earthen fury). Per-target attack events unfold
//!   inside it; the bracket bounds them and attributes spawned casts.
//!
//! Barrage's opener names no target; the processor backfills it from the
//! first named line inside the bracket.

use super::defs::{Role, defs};
use super::target::{self, Actor, Pick};
use crate::state::chunks::ChunkLine;

/// A single-target multi-round attack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(missing_docs)]
pub enum AssaultName {
    Flurry,
    Barrage,
    Pummel,
    GuardantThrust,
    Thrash,
}

impl AssaultName {
    /// Every assault.
    pub const ALL: [Self; 5] = [
        Self::Flurry,
        Self::Barrage,
        Self::Pummel,
        Self::GuardantThrust,
        Self::Thrash,
    ];

    /// The TSV spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Flurry => "flurry",
            Self::Barrage => "barrage",
            Self::Pummel => "pummel",
            Self::GuardantThrust => "guardant_thrust",
            Self::Thrash => "thrash",
        }
    }

    /// Parse the TSV spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.as_str() == text)
    }

    /// **Does this assault roll BEFORE each round line?**
    ///
    /// `processor.rb:44-50`: barrage rolls an aim SMR before each arrow;
    /// every other assault rolls after its round line, fixture-verified. While
    /// a barrage is open a held maneuver roll is ours.
    #[must_use]
    pub const fn rolls_before_round(self) -> bool {
        matches!(self, Self::Barrage)
    }
}

/// A multi-target bracket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(missing_docs)]
pub enum SequenceName {
    EarthenFury,
    Mstrike,
    NaturesFury,
    Volley,
}

impl SequenceName {
    /// Every sequence.
    pub const ALL: [Self; 4] = [
        Self::EarthenFury,
        Self::Mstrike,
        Self::NaturesFury,
        Self::Volley,
    ];

    /// The TSV spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EarthenFury => "earthen_fury",
            Self::Mstrike => "mstrike",
            Self::NaturesFury => "natures_fury",
            Self::Volley => "volley",
        }
    }

    /// Parse the TSV spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.as_str() == text)
    }

    /// **Are this sequence's rounds attack events of the same name?**
    ///
    /// `processor.rb:64-67`: a volley round that prints no initiation line of
    /// its own (a missed arrow: roll + dodge only) is still one.
    #[must_use]
    pub const fn rounds_are_attacks(self) -> bool {
        matches!(self, Self::Volley)
    }
}

/// A bracket's opening line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BracketStart<N> {
    /// Which bracket.
    pub name: N,
    /// The target the opener names, as captured. `None` for barrage.
    pub target_text: Option<String>,
    /// That target, resolved.
    pub target: Option<Actor>,
}

fn start<N>(
    family: &str,
    line: &ChunkLine,
    parse: fn(&str) -> Option<N>,
) -> Option<BracketStart<N>> {
    let text = line.text();
    let (def, caps) = defs().first_match_with_role(family, Role::Start, &text)?;
    Some(BracketStart {
        name: parse(&def.name)?,
        target_text: caps.name("target").map(|m| m.as_str().to_owned()),
        target: caps
            .name("target")
            .and_then(|m| target::link_in(line, m.range(), Pick::First)),
    })
}

fn end<N>(family: &str, line: &ChunkLine, parse: fn(&str) -> Option<N>) -> Option<N> {
    let text = line.text();
    let (def, _) = defs().first_match_with_role(family, Role::End, &text)?;
    parse(&def.name)
}

/// Does this line open an assault?
#[must_use]
pub fn assault_start(line: &ChunkLine) -> Option<BracketStart<AssaultName>> {
    start("assault", line, AssaultName::parse)
}

/// Does this line close an assault, completed or broken?
#[must_use]
pub fn assault_end(line: &ChunkLine) -> Option<AssaultName> {
    end("assault", line, AssaultName::parse)
}

/// Does this line open a sequence?
#[must_use]
pub fn sequence_start(line: &ChunkLine) -> Option<BracketStart<SequenceName>> {
    start("sequence", line, SequenceName::parse)
}

/// Does this line close a sequence?
#[must_use]
pub fn sequence_end(line: &ChunkLine) -> Option<SequenceName> {
    end("sequence", line, SequenceName::parse)
}
