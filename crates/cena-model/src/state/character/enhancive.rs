//! Enhancive totals: six sections, a clique, and a static key space.
//!
//! M3 step 7. `INVENTORY ENHANCIVE TOTALS` reports every bonus an equipped
//! enhancive item is currently granting.
//!
//! # The key space must be known statically
//!
//! `enhancive.rb:368-369`, verbatim:
//!
//! > *"Resets all enhancive values to 0/empty // Critical because game output
//! > only shows non-zero values."*
//!
//! A store with no deletion keeps a stale bonus from a removed item forever, so
//! Lich zeroes **191 keys** before every parse (MEASURED: 10 stats + 46 skills
//! x 2 + 5 resources + 80 cman martial + 1 spells + 3 statistics).
//!
//! Typed fields give that for free -- [`Default`] **is** the zeroing -- and
//! every group here maps onto a type this crate already has:
//! [`StatKind`], [`SkillKind`],
//! and the open PSM mnemonic set. That is the port being structurally better
//! than the original rather than merely faster.
//!
//! # The section headers are a CLIQUE, not a chain
//!
//! `parser.rb:178-193` lets any enhancive state follow `Ready` **or** any other
//! enhancive state, because a character with no enhancive skills simply has no
//! `Skills:` section. MEASURED in the capture, and the order is not the one
//! `parser.rb` lists:
//!
//! ```text
//! Stats:  Skills:  Resources:  Self Knowledge Spells:  Martial Knowledge Skills:  Statistics:
//! ```
//!
//! `parser.rb` declares Martial before Spells. The wire prints Spells first. A
//! reader that assumed declaration order would mis-section every martial row.
//!
//! # Two forms, and Lich reads almost none of the longer one
//!
//! MEASURED by running Lich's own regexes against the captured `DETAILS` output
//! (`plan/15` §2b.4):
//!
//! ```text
//! Stats:
//!   Wisdom (WIS): 15/40
//!     +10: a veniom-bound witchwood badge      <- DROPPED by every pattern
//!     +5: a gilded locus                       <- DROPPED
//! ```
//!
//! The whole item-attribution layer matches nothing, so `TOTALS DETAILS` stores
//! exactly what `TOTALS` would -- **minus the martial skills**, because their
//! line shape inverts between the two forms:
//!
//! ```text
//!   Coup de Grace: +2 ranks                                    <- short, matches
//!   +2 Ranks Coup de Grace: a pallid jade green dragonfly tattoo <- details, does NOT
//! ```
//!
//! `EnhanciveMartialSkill` (`parser.rb:123`) requires `name: +N ranks`. The
//! details form puts the rank first. This module reads both.
//!
//! # The headers arrive BOLDED
//!
//! `<pushBold/>Stats:` on the wire. Lich anchors on `/^Stats:$/` and never
//! sees it. Bold is not required here either -- the literal is unambiguous --
//! but it is recorded because it is a third independent signal, and because a
//! section header is exactly the thing a future format change would re-style.

use std::collections::BTreeMap;

use super::skills::SkillKind;
use super::stats::StatKind;

/// One enhancive bonus: what it grants now, and the most it could.
///
/// **`cap` is the item's ceiling, not the character's.** `Wisdom (WIS): 15/40`
/// means 15 granted against a 40 cap, and `enhancive.rb:36`'s `RESOURCE_CAPS`
/// records the same shape for resources.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Bonus {
    /// The bonus currently granted.
    pub value: u16,
    /// The maximum this enhancive line may reach.
    pub cap: u16,
}

/// Which section of the report a line belongs to.
///
/// **A clique, not a chain.** Any may follow any other, and any may be absent.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Section {
    /// `Stats:` -- one line per stat, `Wisdom (WIS): 15/40`.
    Stats,
    /// `Skills:` -- skill bonus and skill rank lines, `value/cap` each.
    Skills,
    /// `Resources:` -- the five `Resource` lines, `value/cap` each.
    Resources,
    /// Spell numbers granted by an item, e.g. `215, 506, 515, 1109`.
    Spells,
    /// Combat-maneuver ranks, reported as `+N ranks`.
    Martial,
    /// Item and property counts.
    Statistics,
}

impl Section {
    /// All six.
    pub const ALL: [Self; 6] = [
        Self::Stats,
        Self::Skills,
        Self::Resources,
        Self::Spells,
        Self::Martial,
        Self::Statistics,
    ];

    /// The header the wire prints, without its colon.
    #[must_use]
    pub const fn heading(self) -> &'static str {
        match self {
            Self::Stats => "Stats",
            Self::Skills => "Skills",
            Self::Resources => "Resources",
            Self::Spells => "Self Knowledge Spells",
            Self::Martial => "Martial Knowledge Skills",
            Self::Statistics => "Statistics",
        }
    }

    /// Classify a section header, or `None` if the line is not one.
    ///
    /// The colon is required: `Stats:` opens a section, and a player saying
    /// "Stats" does not.
    #[must_use]
    pub fn classify(line: &str) -> Option<Self> {
        let trimmed = line.trim();
        let name = trimmed.strip_suffix(':')?;
        Self::ALL.into_iter().find(|s| s.heading() == name)
    }
}

/// The five resources an enhancive may raise.
///
/// Ported from `enhancive.rb:35`'s `RESOURCES`. Closed, so typed.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Resource {
    /// Printed `Max Mana`.
    MaxMana,
    /// Printed `Max Health`.
    MaxHealth,
    /// Printed `Max Stamina`.
    MaxStamina,
    /// Printed `Mana Recovery`.
    ManaRecovery,
    /// Printed `Stamina Recovery`.
    StaminaRecovery,
}

impl Resource {
    /// All five.
    pub const ALL: [Self; 5] = [
        Self::MaxMana,
        Self::MaxHealth,
        Self::MaxStamina,
        Self::ManaRecovery,
        Self::StaminaRecovery,
    ];

    /// The display name the wire prints (`RESOURCE_NAME_MAP`'s keys).
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::MaxMana => "Max Mana",
            Self::MaxHealth => "Max Health",
            Self::MaxStamina => "Max Stamina",
            Self::ManaRecovery => "Mana Recovery",
            Self::StaminaRecovery => "Stamina Recovery",
        }
    }

    /// Look one up by its printed name.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        let name = name.trim();
        Self::ALL.into_iter().find(|r| r.display_name() == name)
    }
}

/// One classified line of an enhancive report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnhanciveLine {
    /// `  Wisdom (WIS): 15/40`
    Stat {
        /// The stat, matched on its full name case-insensitively; the
        /// parenthesised abbreviation is ignored.
        kind: StatKind,
        /// Granted value and item cap, from `value/cap`.
        bonus: Bonus,
    },
    /// `  Two Weapon Combat Bonus: 10/50`
    SkillBonus {
        /// The skill named before the ` Bonus: ` keyword.
        kind: SkillKind,
        /// Granted skill bonus and its cap.
        bonus: Bonus,
    },
    /// `  Ambush Ranks: 3/50`
    SkillRanks {
        /// The skill named before the ` Ranks: ` keyword.
        kind: SkillKind,
        /// Granted ranks and their cap, reusing `Bonus` for the `value/cap`.
        bonus: Bonus,
    },
    /// `  Max Stamina: 6/300`
    Resource {
        /// Which of the five resources the line names.
        kind: Resource,
        /// Granted amount and its cap.
        bonus: Bonus,
    },
    /// `  215, 506, 515, 1109` -- or, in the details form, one per line.
    Spells {
        /// Spell numbers in printed order; never empty.
        numbers: Vec<u16>,
    },
    /// `  Coup de Grace: +2 ranks`, or `  +2 Ranks Coup de Grace: <item>`.
    Martial {
        /// The maneuver's printed name, trimmed, with any `: <item>` removed.
        /// Text, not an enum: the maneuver set is open, and the report prints
        /// the display name (`Coup de Grace`), not the mnemonic.
        name: String,
        /// The `+N` rank count.
        ranks: u16,
    },
    /// `  Enhancive Items: 6`
    Statistic {
        /// One of `Enhancive Items`, `Enhancive Properties` or
        /// `Total Enhancive Amount`.
        name: String,
        /// The count printed after the colon.
        value: u32,
    },
}

/// The counts the `Statistics:` section reports.
///
/// Named here because the three strings are a closed vocabulary
/// (`parser.rb:124`'s alternation) and matching them literally is what keeps a
/// typo from silently producing a fourth statistic nobody reads.
const STATISTIC_NAMES: [&str; 3] = [
    "Enhancive Items",
    "Enhancive Properties",
    "Total Enhancive Amount",
];

/// Split `value/cap`, both unsigned.
fn parse_bonus(text: &str) -> Option<Bonus> {
    let (value, cap) = text.trim().split_once('/')?;
    Some(Bonus {
        value: value.trim().parse().ok()?,
        cap: cap.trim().parse().ok()?,
    })
}

impl EnhanciveLine {
    /// Classify one line, given the section it arrived in.
    ///
    /// **The section is a parameter, not remembered here.** That keeps this a
    /// stateless classifier per `plan/12` §3a: the caller owns "which section
    /// is open", which is the one fact that genuinely spans lines.
    ///
    /// Returns `None` for blank lines, headers, the trailer, and the details
    /// form's item-attribution lines -- which carry no bonus of their own and
    /// are the caller's to attribute if it wants them.
    #[must_use]
    pub fn classify(line: &str, section: Section) -> Option<Self> {
        let trimmed = line.trim();
        if trimmed.is_empty() || Section::classify(line).is_some() {
            return None;
        }
        match section {
            Section::Stats => Self::classify_stat(trimmed),
            Section::Skills => Self::classify_skill(trimmed),
            Section::Resources => Self::classify_resource(trimmed),
            Section::Spells => Self::classify_spells(trimmed),
            Section::Martial => Self::classify_martial(trimmed),
            Section::Statistics => Self::classify_statistic(trimmed),
        }
    }

    /// `Wisdom (WIS): 15/40`
    fn classify_stat(line: &str) -> Option<Self> {
        let (name, rest) = line.split_once(" (")?;
        let (_abbrev, value) = rest.split_once("): ")?;
        let kind = StatKind::ALL
            .into_iter()
            .find(|k| k.as_str().eq_ignore_ascii_case(name.trim()))?;
        Some(Self::Stat {
            kind,
            bonus: parse_bonus(value)?,
        })
    }

    /// `Two Weapon Combat Bonus: 10/50` or `... Ranks: 3/50`.
    ///
    /// The two differ only in the keyword, and both carry a skill whose display
    /// name may contain spaces and hyphens -- so the split is on the keyword,
    /// not on whitespace.
    fn classify_skill(line: &str) -> Option<Self> {
        let (name, value, is_bonus) = if let Some((n, v)) = line.split_once(" Bonus: ") {
            (n, v, true)
        } else {
            let (n, v) = line.split_once(" Ranks: ")?;
            (n, v, false)
        };
        let kind = SkillKind::parse(name)?;
        let bonus = parse_bonus(value)?;
        Some(if is_bonus {
            Self::SkillBonus { kind, bonus }
        } else {
            Self::SkillRanks { kind, bonus }
        })
    }

    /// `Max Stamina: 6/300`
    fn classify_resource(line: &str) -> Option<Self> {
        let (name, value) = line.split_once(':')?;
        Some(Self::Resource {
            kind: Resource::parse(name)?,
            bonus: parse_bonus(value)?,
        })
    }

    /// `215, 506, 515, 1109`, or the details form's `215: <item>`.
    fn classify_spells(line: &str) -> Option<Self> {
        // The details form attributes one spell per line; take the number and
        // leave the item to the caller.
        let numbers_text = line.split_once(':').map_or(line, |(n, _)| n);
        let numbers: Vec<u16> = numbers_text
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::parse)
            .collect::<Result<_, _>>()
            .ok()?;
        if numbers.is_empty() {
            return None;
        }
        Some(Self::Spells { numbers })
    }

    /// `Coup de Grace: +2 ranks`, or `+2 Ranks Coup de Grace: <item>`.
    ///
    /// **Both forms, which is where Lich loses data.**
    /// `EnhanciveMartialSkill` (`parser.rb:123`) requires `name: +N ranks`, so
    /// the details form's inverted shape matches nothing and every martial
    /// enhancive is dropped from a `TOTALS DETAILS` parse.
    fn classify_martial(line: &str) -> Option<Self> {
        // Details form first: it starts with the `+`.
        if let Some(rest) = line.strip_prefix('+') {
            let (ranks, rest) = rest.split_once(' ')?;
            let rest = rest
                .strip_prefix("Ranks ")
                .or_else(|| rest.strip_prefix("ranks "))?;
            let name = rest.split_once(':').map_or(rest, |(n, _)| n);
            return Some(Self::Martial {
                name: name.trim().to_owned(),
                ranks: ranks.parse().ok()?,
            });
        }
        // Short form: `name: +N ranks`.
        let (name, rest) = line.split_once(": +")?;
        let (ranks, unit) = rest.split_once(' ')?;
        if !unit.trim_end_matches('.').eq_ignore_ascii_case("ranks")
            && !unit.trim_end_matches('.').eq_ignore_ascii_case("rank")
        {
            return None;
        }
        Some(Self::Martial {
            name: name.trim().to_owned(),
            ranks: ranks.parse().ok()?,
        })
    }

    /// `Enhancive Items: 6`
    fn classify_statistic(line: &str) -> Option<Self> {
        let (name, value) = line.split_once(": ")?;
        let name = name.trim();
        if !STATISTIC_NAMES.contains(&name) {
            return None;
        }
        Some(Self::Statistic {
            name: name.to_owned(),
            value: value.trim().parse().ok()?,
        })
    }
}

/// Everything the enhancive report said, zeroed by [`Default`].
///
/// **This type IS `reset_all`.** Lich pushes 191 explicit zeros before each
/// parse because its store has no deletion; here, building a fresh
/// `EnhanciveTotals` and filling it is the same operation with nothing to
/// forget. See the module docs.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EnhanciveTotals {
    stats: BTreeMap<StatKind, Bonus>,
    skill_bonus: BTreeMap<SkillKind, Bonus>,
    skill_ranks: BTreeMap<SkillKind, Bonus>,
    resources: BTreeMap<Resource, Bonus>,
    /// Combat-maneuver ranks, keyed by the display name the report printed.
    ///
    /// **Open**, unlike the other four: the martial set is the 80 combat
    /// maneuvers, which grow with the game (`psm.rs` makes the same argument).
    /// Keyed by display name rather than mnemonic because this report prints
    /// only the name -- `Coup de Grace`, not `coupdegrace`.
    martial: BTreeMap<String, u16>,
    spells: Vec<u16>,
    statistics: BTreeMap<String, u32>,
    /// Which sections the report actually carried.
    ///
    /// The clique means absence is normal, so "no enhancive skills" and "the
    /// Skills section was never read" are different facts. This is the same
    /// distinction `PsmSet::has_table` draws, and MO-3's complaint about
    /// `Effects`.
    seen: BTreeMap<Section, bool>,
}

impl EnhanciveTotals {
    /// The bonus to one stat, or `None` if no enhancive raises it.
    #[must_use]
    pub fn stat(&self, kind: StatKind) -> Option<Bonus> {
        self.stats.get(&kind).copied()
    }

    /// The bonus to one skill.
    #[must_use]
    pub fn skill_bonus(&self, kind: SkillKind) -> Option<Bonus> {
        self.skill_bonus.get(&kind).copied()
    }

    /// Enhancive ranks in one skill, which are rarer than bonuses.
    #[must_use]
    pub fn skill_ranks(&self, kind: SkillKind) -> Option<Bonus> {
        self.skill_ranks.get(&kind).copied()
    }

    /// The bonus to one resource.
    #[must_use]
    pub fn resource(&self, kind: Resource) -> Option<Bonus> {
        self.resources.get(&kind).copied()
    }

    /// Extra ranks in one combat maneuver, by its display name.
    #[must_use]
    pub fn martial(&self, name: &str) -> Option<u16> {
        self.martial.get(name).copied()
    }

    /// Every martial entry, ordered.
    pub fn martials(&self) -> impl Iterator<Item = (&str, u16)> {
        self.martial.iter().map(|(n, r)| (n.as_str(), *r))
    }

    /// The spell numbers items grant, in wire order.
    #[must_use]
    pub fn spells(&self) -> &[u16] {
        &self.spells
    }

    /// One of the three `Statistics:` counts.
    #[must_use]
    pub fn statistic(&self, name: &str) -> Option<u32> {
        self.statistics.get(name).copied()
    }

    /// Did the report carry this section at all?
    #[must_use]
    pub fn saw_section(&self, section: Section) -> bool {
        self.seen.get(&section).copied().unwrap_or(false)
    }

    /// Nothing recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }

    /// Note that a section header arrived, even if it holds no rows.
    pub fn mark_section(&mut self, section: Section) {
        self.seen.insert(section, true);
    }

    /// Record one classified line.
    pub fn apply(&mut self, line: &EnhanciveLine) {
        match line {
            EnhanciveLine::Stat { kind, bonus } => {
                self.stats.insert(*kind, *bonus);
            }
            EnhanciveLine::SkillBonus { kind, bonus } => {
                self.skill_bonus.insert(*kind, *bonus);
            }
            EnhanciveLine::SkillRanks { kind, bonus } => {
                self.skill_ranks.insert(*kind, *bonus);
            }
            EnhanciveLine::Resource { kind, bonus } => {
                self.resources.insert(*kind, *bonus);
            }
            EnhanciveLine::Spells { numbers } => {
                // **Extended, not replaced.** The details form prints one
                // spell per line; the short form prints them all on one. Both
                // must end with the same list.
                for n in numbers {
                    if !self.spells.contains(n) {
                        self.spells.push(*n);
                    }
                }
            }
            EnhanciveLine::Martial { name, ranks } => {
                self.martial.insert(name.clone(), *ranks);
            }
            EnhanciveLine::Statistic { name, value } => {
                self.statistics.insert(name.clone(), *value);
            }
        }
    }

    /// Read a whole report out of its lines.
    ///
    /// **The one piece of state is which section is open**, which is why this
    /// is a function over the report rather than a method on a machine: the
    /// section is scoped to this call and cannot survive a disconnect
    /// mid-report. That is the same shape `InfoReport::read` took when the
    /// block state machine was deleted.
    ///
    /// Returns `None` if no section header was ever seen, so a chunk that
    /// merely contains a stat-shaped line is not mistaken for a report.
    #[must_use]
    pub fn read<'a>(lines: impl IntoIterator<Item = &'a str>) -> Option<Self> {
        let mut totals = Self::default();
        let mut section = None;
        for line in lines {
            if let Some(found) = Section::classify(line) {
                section = Some(found);
                totals.mark_section(found);
                continue;
            }
            let Some(open) = section else { continue };
            if let Some(parsed) = EnhanciveLine::classify(line, open) {
                totals.apply(&parsed);
            }
        }
        (!totals.is_empty()).then_some(totals)
    }
}
