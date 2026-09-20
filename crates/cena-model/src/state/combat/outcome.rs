//! Outcome lines: why a swing produced nothing, or that it landed.
//!
//! Ports `Definitions::Outcomes.parse` and `inbound_line?`
//! (`outcomes.rb:29-373`). One outcome per line, first match wins, with the
//! specific categories before the broad `evade`/`block` pools.
//!
//! # An outcome can be the only record of an inbound swing
//!
//! *"A fully-intercepted inbound swing prints no initiation line, so this is
//! the only record of it -- the processor opens it inbound, not as an attack
//! ON X"* (`outcomes.rb:358-362`, hunt log 2026-09-07: barrier blocks filed as
//! unknown vs warg). [`Outcome::names_attacker`] is that signal: the def
//! carried an `(?<attacker>)` capture.

use super::defs::defs;
use super::target::{self, Actor, Pick};
use crate::state::chunks::ChunkLine;

/// What an outcome line says happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OutcomeKind {
    /// A clean miss, a swing gone wide, an arrow off into the distance.
    Miss,
    /// `A hit!` / `Good hit!`, and the rider lines that confirm a strike.
    Hit,
    /// Dodged.
    Evade,
    /// Blocked by a shield.
    Block,
    /// Parried.
    Parry,
    /// A warding spell warded off.
    Warded,
    /// The target's warding failed -- the spell landed.
    WardFailed,
    /// Intercepted by a barrier or a guardian.
    Intercept,
    /// Resisted.
    Resisted,
    /// Unaffected.
    Unaffected,
    /// `d100 == 1 FUMBLE!`
    Fumble,
    /// The spell-hindrance roll.
    Hindrance,
    /// Confused.
    Confused,
    /// The target was already dead.
    AlreadyDead,
}

impl OutcomeKind {
    /// Every kind.
    pub const ALL: [Self; 14] = [
        Self::Miss,
        Self::Hit,
        Self::Evade,
        Self::Block,
        Self::Parry,
        Self::Warded,
        Self::WardFailed,
        Self::Intercept,
        Self::Resisted,
        Self::Unaffected,
        Self::Fumble,
        Self::Hindrance,
        Self::Confused,
        Self::AlreadyDead,
    ];

    /// The TSV spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Miss => "miss",
            Self::Hit => "hit",
            Self::Evade => "evade",
            Self::Block => "block",
            Self::Parry => "parry",
            Self::Warded => "warded",
            Self::WardFailed => "ward_failed",
            Self::Intercept => "intercept",
            Self::Resisted => "resisted",
            Self::Unaffected => "unaffected",
            Self::Fumble => "fumble",
            Self::Hindrance => "hindrance",
            Self::Confused => "confused",
            Self::AlreadyDead => "already_dead",
        }
    }

    /// Parse the TSV spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == text)
    }

    /// Did nothing land?
    #[must_use]
    pub const fn is_negation(self) -> bool {
        !matches!(self, Self::Hit | Self::WardFailed)
    }
}

/// One outcome line, classified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// What happened.
    pub kind: OutcomeKind,
    /// The def captured an attacker: this line records who attacked US.
    pub names_attacker: bool,
    /// That attacker, resolved.
    pub attacker: Option<Actor>,
    /// The target the line names, resolved, where the def captures one.
    pub target: Option<Actor>,
}

impl Outcome {
    /// Classify one line as an outcome.
    #[must_use]
    pub fn classify(line: &ChunkLine) -> Option<Self> {
        let text = line.text();
        let (def, caps) = defs().first_match("outcome", &text)?;
        let names_attacker = def
            .regex
            .as_ref()
            .is_some_and(|re| re.capture_names().any(|n| n == Some("attacker")));
        Some(Self {
            kind: OutcomeKind::parse(&def.name)?,
            names_attacker,
            attacker: caps.name("attacker").and_then(|m| {
                target::link_in(line, m.range(), Pick::Last)
                    .or_else(|| Some(Actor::unlinked(m.as_str())))
            }),
            target: caps
                .name("target")
                .and_then(|m| target::link_in(line, m.range(), Pick::First)),
        })
    }
}
