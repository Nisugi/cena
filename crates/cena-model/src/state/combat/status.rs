//! Status lines: a condition beginning or ending.
//!
//! Ports `Definitions::Statuses.parse` (`statuses.rb`): thirty statuses, each
//! with add patterns and remove patterns, matched as one list -- every add
//! before every remove, first match wins (`ALL_LOOKUP = ADD_LOOKUP +
//! REMOVE_LOOKUP`, `:448`).
//!
//! # A status is a closed vocabulary
//!
//! The thirty names are the game's conditions as Lich catalogued them, and
//! the processor branches on several by name -- the floor positions are
//! mutually exclusive (`POSITION_STATUSES`, `processor.rb:15-19`), a stand-up
//! clears all three. C21 says typed variants for a closed set, and a Lich
//! addition goes red in `tests/combat_defs.rs`, which asserts every TSV name
//! parses, rather than silently classifying to nothing.

use super::defs::{Role, defs};
use super::target::{self, Actor, Pick};
use crate::state::chunks::ChunkLine;

/// A combat status, as Lich names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(missing_docs)]
pub enum StatusName {
    Blind,
    Burning,
    Calm,
    Dazed,
    Demoralized,
    Disarmed,
    Dispelled,
    Hardened,
    Hidden,
    Immobilized,
    Kneeling,
    NaturesDecay,
    OffBalance,
    Paralyzed,
    Poisoned,
    Prone,
    Roundtime,
    Silenced,
    Sitting,
    Sleeping,
    Slowed,
    Sounds,
    Stunned,
    Sunburst,
    Tangleweed,
    Terrified,
    Unconscious,
    Vulnerable,
    Weakened,
    Webbed,
}

impl StatusName {
    /// Every status.
    pub const ALL: [Self; 30] = [
        Self::Blind,
        Self::Burning,
        Self::Calm,
        Self::Dazed,
        Self::Demoralized,
        Self::Disarmed,
        Self::Dispelled,
        Self::Hardened,
        Self::Hidden,
        Self::Immobilized,
        Self::Kneeling,
        Self::NaturesDecay,
        Self::OffBalance,
        Self::Paralyzed,
        Self::Poisoned,
        Self::Prone,
        Self::Roundtime,
        Self::Silenced,
        Self::Sitting,
        Self::Sleeping,
        Self::Slowed,
        Self::Sounds,
        Self::Stunned,
        Self::Sunburst,
        Self::Tangleweed,
        Self::Terrified,
        Self::Unconscious,
        Self::Vulnerable,
        Self::Weakened,
        Self::Webbed,
    ];

    /// The TSV spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Blind => "blind",
            Self::Burning => "burning",
            Self::Calm => "calm",
            Self::Dazed => "dazed",
            Self::Demoralized => "demoralized",
            Self::Disarmed => "disarmed",
            Self::Dispelled => "dispelled",
            Self::Hardened => "hardened",
            Self::Hidden => "hidden",
            Self::Immobilized => "immobilized",
            Self::Kneeling => "kneeling",
            Self::NaturesDecay => "natures_decay",
            Self::OffBalance => "off_balance",
            Self::Paralyzed => "paralyzed",
            Self::Poisoned => "poisoned",
            Self::Prone => "prone",
            Self::Roundtime => "roundtime",
            Self::Silenced => "silenced",
            Self::Sitting => "sitting",
            Self::Sleeping => "sleeping",
            Self::Slowed => "slowed",
            Self::Sounds => "sounds",
            Self::Stunned => "stunned",
            Self::Sunburst => "sunburst",
            Self::Tangleweed => "tangleweed",
            Self::Terrified => "terrified",
            Self::Unconscious => "unconscious",
            Self::Vulnerable => "vulnerable",
            Self::Weakened => "weakened",
            Self::Webbed => "webbed",
        }
    }

    /// Parse the TSV spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.as_str() == text)
    }

    /// **A floor position.** Prone, sitting and kneeling are one mutually
    /// exclusive channel: a creature is in at most one, and the stand-up
    /// messagings are shared, so a removal of any clears all three
    /// (`processor.rb:15-19`, `:2370-2371`).
    #[must_use]
    pub const fn is_position(self) -> bool {
        matches!(self, Self::Prone | Self::Sitting | Self::Kneeling)
    }
}

/// Onset or expiry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StatusAction {
    /// The status began.
    Add,
    /// The status ended.
    Remove,
}

/// One status line, classified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusLine {
    /// Which status.
    pub status: StatusName,
    /// Began or ended.
    pub action: StatusAction,
    /// The subject, as captured. `None` for a line about us
    /// (`You are blinded!`).
    pub target_text: Option<String>,
    /// The subject, resolved to its link.
    pub target: Option<Actor>,
}

impl StatusLine {
    /// Classify one line as a status onset or expiry.
    #[must_use]
    pub fn classify(line: &ChunkLine) -> Option<Self> {
        let text = line.text();
        let (def, caps) = defs().first_match("status", &text)?;
        let action = match def.role {
            Role::Add => StatusAction::Add,
            Role::Remove => StatusAction::Remove,
            _ => return None,
        };
        Some(Self {
            status: StatusName::parse(&def.name)?,
            action,
            target_text: caps.name("target").map(|m| m.as_str().to_owned()),
            target: caps
                .name("target")
                .and_then(|m| target::link_in(line, m.range(), Pick::First)),
        })
    }

    /// A line about us rather than a creature.
    #[must_use]
    pub fn is_self(&self) -> bool {
        self.target_text.as_deref().is_none_or(target::is_self)
    }
}
