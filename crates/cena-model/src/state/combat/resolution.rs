//! Roll lines: the numbers that decide an attack.
//!
//! Ports `Definitions::Resolutions.parse` (`outcomes.rb:375-430`): eight roll
//! grammars, every named capture converted to a number.
//!
//! # One shape for eight grammars
//!
//! Lich returns a hash keyed by whatever the pattern captured (`as`, `ds`,
//! `avd` for one; `cs`, `td`, `cva` for another). Its recorder then folds
//! those into one row shape -- `attacker_stat`, `defender_stat`, `modifier`,
//! `roll`, `bonus`, `penalty`, `result`, `total` (`recorder.rb`, the
//! `resolutions` table) -- and `combat_stats.lic:132-135` prints each grammar
//! back from those columns. That fold is done here, once, so every consumer
//! sees the same eight fields whatever the grammar:
//!
//! | kind | attacker | defender | modifier | extra |
//! |---|---|---|---|---|
//! | `as_ds` | AS | DS | `AvD` | `bonus` = True Strike |
//! | `cs_td` | CS | TD | `CvA` | `bonus` / `penalty` |
//! | `uaf_udf` | UAF | UDF | MM | `total`, fractional |
//! | `fear` | FS | FD | `FvP` | |
//! | `activation` | | | Modifiers | |
//! | `smr` / `ssr` / `maneuver_roll` | | | | `bonus` / `penalty` |
//!
//! **`roll` is kept apart from `result`** so `result - roll` is the
//! deterministic margin -- what a "use a setup first" tactic reads.

use super::defs::defs;
use crate::state::chunks::ChunkLine;

/// Which roll grammar a line is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResolutionKind {
    /// Melee and ranged: `AS: +351 vs DS: +299 with AvD: +22 + d100 roll: +88 = +162`.
    AsDs,
    /// Warding spells: `CS: 384 - TD: 419 + CvA: 13 + d100: 59 == 37`.
    CsTd,
    /// Unarmed combat: `UAF: 681 vs UDF: 575 = 1.18 * MM: 110 + d100: 32 = 130`.
    UafUdf,
    /// Standard maneuver roll: `[SMR result: 191 (Open d100: 13, Bonus: 56)]`.
    Smr,
    /// Standard save roll: `[SSR result: N (Open d100: N)]`.
    Ssr,
    /// Imbed or crystal activation: `1d100: 30 + Modifiers: 375 == 405`.
    Activation,
    /// The legacy maneuver form: `[Roll result: N (open d100: N)]`.
    ManeuverRoll,
    /// Sheer fear: `FS: N - FD: N + FvP: N + d100(L): N = N`.
    Fear,
}

impl ResolutionKind {
    /// Every kind.
    pub const ALL: [Self; 8] = [
        Self::AsDs,
        Self::CsTd,
        Self::UafUdf,
        Self::Smr,
        Self::Ssr,
        Self::Activation,
        Self::ManeuverRoll,
        Self::Fear,
    ];

    /// The TSV and recorder spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AsDs => "as_ds",
            Self::CsTd => "cs_td",
            Self::UafUdf => "uaf_udf",
            Self::Smr => "smr",
            Self::Ssr => "ssr",
            Self::Activation => "activation",
            Self::ManeuverRoll => "maneuver_roll",
            Self::Fear => "fear",
        }
    }

    /// Parse the TSV spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == text)
    }

    /// **Does this roll precede its attack line?**
    ///
    /// `COMBAT_DEFS_ONBOARDING.md` §5, rule 1: *"Maneuver-class rolls
    /// (SMR/SSR/fear) PRECEDE their per-target line; AS/DS-class rolls FOLLOW
    /// their attack line."* The processor's held-roll logic keys on this set
    /// (`processor.rb:1625`: `%i[smr ssr maneuver_roll]`).
    #[must_use]
    pub const fn precedes_its_line(self) -> bool {
        matches!(
            self,
            Self::Smr | Self::Ssr | Self::ManeuverRoll | Self::Fear
        )
    }
}

/// One roll line, folded into the recorder's shape.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolution {
    /// Which grammar.
    pub kind: ResolutionKind,
    /// AS, CS, UAF or FS.
    pub attacker_stat: Option<i64>,
    /// DS, TD, UDF or FD.
    pub defender_stat: Option<i64>,
    /// `AvD`, `CvA`, MM, `FvP` or an activation's Modifiers.
    pub modifier: Option<i64>,
    /// The die, kept separate.
    pub roll: Option<i64>,
    /// A bonus term.
    pub bonus: Option<i64>,
    /// A penalty term.
    pub penalty: Option<i64>,
    /// The endroll.
    pub result: Option<i64>,
    /// UCS's pre-MM total, the one fractional value.
    pub total: Option<f64>,
}

impl Resolution {
    /// Classify one line as a roll.
    #[must_use]
    pub fn classify(line: &ChunkLine) -> Option<Self> {
        let text = line.text();
        let (def, caps) = defs().first_match("resolution", &text)?;
        let kind = ResolutionKind::parse(&def.name)?;
        let int = |name: &str| caps.name(name).and_then(|m| m.as_str().parse::<i64>().ok());
        Some(Self {
            kind,
            attacker_stat: int("as")
                .or_else(|| int("cs"))
                .or_else(|| int("uaf"))
                .or_else(|| int("fs")),
            defender_stat: int("ds")
                .or_else(|| int("td"))
                .or_else(|| int("udf"))
                .or_else(|| int("fd")),
            modifier: int("avd")
                .or_else(|| int("cva"))
                .or_else(|| int("mm"))
                .or_else(|| int("fvp"))
                .or_else(|| int("mods")),
            roll: int("roll"),
            bonus: int("bonus"),
            penalty: int("penalty"),
            result: int("result"),
            total: caps
                .name("total")
                .and_then(|m| m.as_str().parse::<f64>().ok()),
        })
    }

    /// `result - roll`: the margin the dice did not supply.
    #[must_use]
    pub fn margin(&self) -> Option<i64> {
        Some(self.result? - self.roll?)
    }
}

/// Is this the knockdown rider a leg crit rolls after its damage settles?
///
/// `outcomes.rb:437-451`: that roll belongs to the hit that caused it, not to
/// the next attack -- *"without this it was orphaned into a synthetic :unknown
/// attack against whatever creature was current"* (2026-09-07 hunt log).
#[must_use]
pub fn crit_rider_line(line: &ChunkLine) -> bool {
    defs().any_match("crit_rider", &line.text())
}
