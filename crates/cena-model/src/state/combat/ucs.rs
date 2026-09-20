//! Unarmed Combat System lines: position tier, tierup, smite.
//!
//! Ports `Definitions::UCS` (`ucs.rb`). Five of its six patterns read the
//! creature's `exist` out of the tag (`<a exist="([0-9]+)"`), so they are
//! hand-ported (`defs::HAND_PORTED`): the sentence without the tag, and the
//! creature from the line's first link.
//!
//! # Position is a tier, stored as a number
//!
//! `ucs.rb:42-45`: *"Positioning tier words -> ordinal, so the recorder can
//! persist positioning as a number ... and queries can compare outbound vs
//! inbound tiers directly."* [`PositionTier`] carries that ordinal.

use super::defs::defs;
use super::target::{self, Actor};
use crate::state::chunks::ChunkLine;

/// How well positioned: `decent` < `good` < `excellent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PositionTier {
    /// Tier 1.
    Decent,
    /// Tier 2.
    Good,
    /// Tier 3.
    Excellent,
}

impl PositionTier {
    /// Every tier, ascending.
    pub const ALL: [Self; 3] = [Self::Decent, Self::Good, Self::Excellent];

    /// The ordinal Lich records: 1, 2, 3.
    #[must_use]
    pub const fn ordinal(self) -> u8 {
        match self {
            Self::Decent => 1,
            Self::Good => 2,
            Self::Excellent => 3,
        }
    }

    /// The word the game uses.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Decent => "decent",
            Self::Good => "good",
            Self::Excellent => "excellent",
        }
    }

    /// Parse the game's word, case-insensitively.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|t| t.as_str().eq_ignore_ascii_case(text.trim()))
    }
}

/// The followup a tierup opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(missing_docs)]
pub enum UcsAttack {
    Jab,
    Grapple,
    Punch,
    Kick,
}

impl UcsAttack {
    /// Every attack.
    pub const ALL: [Self; 4] = [Self::Jab, Self::Grapple, Self::Punch, Self::Kick];

    /// The game's word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Jab => "jab",
            Self::Grapple => "grapple",
            Self::Punch => "punch",
            Self::Kick => "kick",
        }
    }

    /// Parse the game's word, case-insensitively.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|a| a.as_str().eq_ignore_ascii_case(text.trim()))
    }
}

/// One UCS line, classified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UcsLine {
    /// `You have good positioning against a kobold.`
    Position {
        /// Our tier.
        tier: PositionTier,
        /// Against whom.
        target: Option<Actor>,
    },
    /// `The triton brawler has decent positioning against you.` -- the
    /// creature's tier against US, printed inside its own attack block.
    PositionInbound {
        /// Its tier.
        tier: PositionTier,
        /// Who.
        attacker: Option<Actor>,
    },
    /// `Strike leaves foe vulnerable to a followup jab attack!`
    Tierup {
        /// Which followup.
        attack: UcsAttack,
    },
    /// `A crimson mist suddenly surrounds ...` -- smite applied.
    SmiteApplied {
        /// The creature.
        target: Option<Actor>,
    },
    /// `... held in the corporeal plane` -- smite holding.
    SmiteHeld {
        /// The creature.
        target: Option<Actor>,
    },
    /// `... returns to an ethereal state` -- smite gone.
    SmiteRemoved {
        /// The creature.
        target: Option<Actor>,
    },
}

/// The substrings every UCS pattern contains one of (`ucs.rb:46`).
pub const GATE: [&str; 3] = [
    "positioning against",
    "vulnerable to a followup",
    "crimson mist",
];

/// The line's first creature link, positive id.
fn creature(line: &ChunkLine) -> Option<Actor> {
    target::any_link(line).filter(|a| a.id.is_some_and(|id| id > 0))
}

impl UcsLine {
    /// Classify one line as a UCS event.
    #[must_use]
    pub fn classify(line: &ChunkLine) -> Option<Self> {
        let text = line.text();
        if !GATE.iter().any(|g| text.contains(g)) {
            return None;
        }
        let (def, caps) = defs().first_match("ucs", &text)?;
        match def.name.as_str() {
            "position" => Some(Self::Position {
                tier: PositionTier::parse(caps.name("tier")?.as_str())?,
                target: creature(line),
            }),
            "position_inbound" => Some(Self::PositionInbound {
                tier: PositionTier::parse(caps.name("tier")?.as_str())?,
                attacker: creature(line),
            }),
            // Lich's pattern has an unnamed group: `(jab|grapple|punch|kick)`.
            "tierup" => Some(Self::Tierup {
                attack: UcsAttack::parse(caps.get(1)?.as_str())?,
            }),
            "smite_applied" => Some(Self::SmiteApplied {
                target: creature(line),
            }),
            "smite_held" => Some(Self::SmiteHeld {
                target: creature(line),
            }),
            "smite_removed" => Some(Self::SmiteRemoved {
                target: creature(line),
            }),
            _ => None,
        }
    }
}
