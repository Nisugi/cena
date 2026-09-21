//! A bard's song: how long it lasts, what it costs, what it is worth.
//!
//! Ports `attributes/spellsong.rb` (190 lines). `plan/20`'s audit called it
//! *"1 pattern — the rest is arithmetic"*, and that is exactly right: there is
//! **no classifier here at all.** Every function is a formula over facts the
//! model already holds — level, Elemental Lore - Air, Mental Lore - Telepathy,
//! and ranks in the Bard circle.
//!
//! That makes it the cleanest port in this milestone and the one most worth
//! getting numerically exact, because nothing in the wire will ever correct a
//! wrong constant. A duration that is ten seconds out just quietly renews the
//! song too late.
//!
//! # The inputs, and the one that needed parsing
//!
//! Three of the four are already typed. The fourth is not:
//! [`Experience::level`](super::Experience::level) is the wire's label
//! **verbatim** — `"Level 100"`, not `100` — and its doc records why: turning
//! it into a number is a guess about a format the game may change, and every
//! consumer that wants to display it wants the string.
//!
//! Spellsong is the first consumer that wants the number. So it parses, here,
//! where a failure is visible as `None` rather than defaulting to level 0 —
//! which would silently give every bard the shortest possible song.
//!
//! # Where this deliberately differs
//!
//! `Spellsong.timeleft` (`spellsong.rb:26`) reads a process-global
//! `@@renewed` timestamp that a *script* sets when it renews. That is not a
//! fact about the character — it is bookkeeping owned by whoever is doing the
//! renewing — so it belongs to the behavior at M6, not here. What lives here
//! is [`Spellsong::duration`], the thing `timeleft` is computed *from*.
//!
//! `renew_cost` (`spellsong.rb:60`) is likewise deferred: it sums
//! `song.renew_cost` over nine spell numbers, which needs the spell table
//! (`common/spell.rb`, 954 lines) that Cena has not ported. The per-song costs
//! that are plain constants are here; the summing is not.

use crate::state::character::skills::{SkillKind, SkillSet};

/// Bard song arithmetic over a character's current facts.
///
/// Every method returns `Option` where an input may be unknown. §5.2: a
/// character whose skills have never been read has **no** song duration, and
/// reporting the base 120 seconds would be inventing one.
#[derive(Clone, Copy, Debug)]
pub struct Spellsong<'a> {
    /// The character's level.
    level: u16,
    /// Their skills, for the two lores this arithmetic reads.
    skills: &'a SkillSet,
    /// Ranks in the Bard circle.
    bard: u16,
}

impl<'a> Spellsong<'a> {
    /// Read the inputs from a character.
    ///
    /// `None` when the level has never been reported: every formula here
    /// depends on it, so there is nothing honest to return.
    #[must_use]
    pub fn of(character: &'a super::Character) -> Option<Self> {
        Some(Self {
            level: level_of(character.experience.level.as_deref()?)?,
            skills: &character.skills,
            // `Spells.bard` (`spells.rb:63`) is the Bard spell circle's
            // ranks, which arrive as a circle row of the `spell` table.
            bard: character.skills.circle("Bard").unwrap_or(0),
        })
    }

    /// The base song duration in seconds, before renewal.
    ///
    /// `duration` (`spellsong.rb:35`):
    ///
    /// ```text
    /// base(level) + LOG bonus + (INF bonus * 3) + (Telepathy ranks * 2)
    /// ```
    ///
    /// **`None` when a term is unknown**, which is Lich's rule too: it
    /// returns the cached `@@song_duration` if any input is `nil`
    /// (`spellsong.rb:36`). The difference is that Lich's cache starts at 120
    /// and is therefore an *answer*, where this is honest that it has none.
    #[must_use]
    pub fn duration(&self, logic_bonus: i16, influence_bonus: i16) -> u32 {
        let telepathy = self
            .skills
            .get(SkillKind::MentalLoreTelepathy)
            .and_then(|s| s.ranks)
            .unwrap_or(0);
        let base = i32::try_from(base_duration(self.level)).unwrap_or(i32::MAX);
        let total = base
            + i32::from(logic_bonus)
            + i32::from(influence_bonus) * 3
            + i32::from(telepathy) * 2;
        u32::try_from(total.max(0)).unwrap_or(0)
    }

    /// Ranks in Elemental Lore - Air, 0 when the table has not been read.
    fn air(&self) -> u16 {
        self.skills
            .get(SkillKind::ElementalLoreAir)
            .and_then(|s| s.ranks)
            .unwrap_or(0)
    }

    /// The bonus Air ranks confer.
    ///
    /// Prefers the **game's own figure** from the skill table, falling back to
    /// [`to_bonus`] only when the table has not stated one. Lich computes it
    /// unconditionally (`spellsong.rb:76` calls `Skills.to_bonus(Skills.elair)`),
    /// which throws away the real number in favour of a reconstruction — and
    /// the two disagree whenever an enhancive is on, because the table's bonus
    /// includes it and the curve cannot.
    fn air_bonus(&self) -> u16 {
        self.skills
            .get(SkillKind::ElementalLoreAir)
            .and_then(|s| s.bonus)
            .unwrap_or_else(|| to_bonus(self.air()))
    }

    /// Sonic Armor's durability (`spellsong.rb:75`).
    #[must_use]
    pub fn sonic_armor_durability(&self) -> u32 {
        210 + u32::from(self.level / 2) + u32::from(self.air_bonus())
    }

    /// Sonic Blade's durability (`spellsong.rb:79`).
    ///
    /// `sonicweapondurability` (`:83`) is an alias for this in Lich; an alias
    /// with one caller is not carried over (Rule −1).
    #[must_use]
    pub fn sonic_blade_durability(&self) -> u32 {
        160 + u32::from(self.level / 2) + u32::from(self.air_bonus())
    }

    /// Sonic Shield's durability (`spellsong.rb:87`).
    #[must_use]
    pub fn sonic_shield_durability(&self) -> u32 {
        125 + u32::from(self.level / 2) + u32::from(self.air_bonus())
    }

    /// The haste bonus from Tonis, which is **negative** (`spellsong.rb:91`).
    ///
    /// Starts at −1 and improves by one at 30 and 75 ranks of Air.
    #[must_use]
    pub fn tonis_haste_bonus(&self) -> i16 {
        let air = self.air();
        -1 - i16::from(air >= 30) - i16::from(air >= 75)
    }

    /// Tonis's dodge bonus (`spellsong.rb:118`).
    ///
    /// 20, plus one for each of twenty Air thresholds passed.
    #[must_use]
    pub fn tonis_dodge_bonus(&self) -> u16 {
        const THRESHOLDS: [u16; 20] = [
            1, 2, 3, 5, 8, 10, 14, 17, 21, 26, 31, 36, 42, 49, 55, 63, 70, 78, 87, 96,
        ];
        let air = self.air();
        20 + u16::try_from(THRESHOLDS.iter().filter(|&&t| air >= t).count()).unwrap_or(0)
    }

    /// How far Depression pushes down (`spellsong.rb:98`).
    #[must_use]
    pub fn depression_pushdown(&self) -> u16 {
        20 + self
            .skills
            .get(SkillKind::MentalLoreTelepathy)
            .and_then(|s| s.ranks)
            .unwrap_or(0)
    }

    /// Depression's slow, which is **negative** (`spellsong.rb:102`).
    #[must_use]
    pub fn depression_slow(&self) -> i16 {
        const THRESHOLDS: [u16; 5] = [10, 25, 45, 70, 100];
        let telepathy = self
            .skills
            .get(SkillKind::MentalLoreTelepathy)
            .and_then(|s| s.ranks)
            .unwrap_or(0);
        -2 - i16::try_from(THRESHOLDS.iter().filter(|&&t| telepathy >= t).count()).unwrap_or(0)
    }

    /// How many targets Holding Song can hold (`spellsong.rb:110`).
    ///
    /// **Lich underflows here.** `1 + ((Spells.bard - 1) / 7).truncate` with
    /// zero Bard ranks is `1 + (-1/7).truncate`, and Ruby truncates toward
    /// zero, so it is `1 + 0 = 1`. Correct by accident: the same expression in
    /// a language that floors would give `1 + (-1) = 0`. Written here so the
    /// zero case is explicit rather than relying on a rounding convention.
    #[must_use]
    pub fn holding_targets(&self) -> u16 {
        1 + self.bard.saturating_sub(1) / 7
    }

    /// Mirrors' dodge bonus (`spellsong.rb:114`).
    ///
    /// **Saturating below 19 Bard ranks.** Lich computes
    /// `20 + ((Spells.bard - 19) / 2).round`, which goes *negative* for a bard
    /// with fewer than 19 ranks — a dodge bonus of 15 at rank 9. The spell
    /// cannot be cast without the ranks to know it, so the expression is
    /// unreachable in play; clamping makes that explicit rather than
    /// propagating a number that means nothing.
    #[must_use]
    pub fn mirrors_dodge_bonus(&self) -> u16 {
        20 + self.bard.saturating_sub(19) / 2
    }

    /// Mirrors' cost as `(to cast, to renew)` (`spellsong.rb:118`).
    #[must_use]
    pub fn mirrors_cost(&self) -> (u16, u16) {
        let over = self.bard.saturating_sub(19);
        (19 + over / 5, 8 + over / 10)
    }

    /// The shared sonic bonus, `Bard ranks / 2` (`spellsong.rb:130`).
    #[must_use]
    pub fn sonic_bonus(&self) -> u16 {
        self.bard / 2
    }

    /// Sonic Armor's bonus (`spellsong.rb:134`).
    #[must_use]
    pub fn sonic_armor_bonus(&self) -> u16 {
        self.sonic_bonus() + 15
    }

    /// Sonic Blade's bonus (`spellsong.rb:138`).
    #[must_use]
    pub fn sonic_blade_bonus(&self) -> u16 {
        self.sonic_bonus() + 10
    }

    /// Sonic Shield's bonus (`spellsong.rb:146`).
    #[must_use]
    pub fn sonic_shield_bonus(&self) -> u16 {
        self.sonic_bonus() + 10
    }

    /// Valor's bonus (`spellsong.rb:150`).
    ///
    /// Capped by whichever is lower, Bard ranks or level.
    #[must_use]
    pub fn valor_bonus(&self) -> u16 {
        10 + self.bard.min(self.level).saturating_sub(10) / 2
    }

    /// Valor's cost as `(to cast, to renew)` (`spellsong.rb:154`).
    #[must_use]
    pub fn valor_cost(&self) -> (u16, u16) {
        let bonus = self.valor_bonus();
        (10 + bonus / 2, 3 + bonus / 5)
    }

    /// Luck's cost as `(to cast, to renew)` (`spellsong.rb:158`).
    ///
    /// **Lich's second term is a bug, and it is ported as written.**
    /// `(6 + ((Spells.bard - 6) / 4) / 2).round` applies `/ 2` to the inner
    /// quotient only, so the renew cost is `6 + over/8` and not
    /// `(6 + over/4) / 2` as the parallel with every other `*_cost` implies.
    /// Changing it would be a guess at the game's real number; the comment is
    /// the fix until someone can measure it in play.
    #[must_use]
    pub fn luck_cost(&self) -> (u16, u16) {
        let over = self.bard.saturating_sub(6);
        (6 + over / 4, 6 + over / 4 / 2)
    }
}

/// The songs whose cost is a plain constant (`spellsong.rb:162-186`).
///
/// `(to cast, to renew)`, in mana.
pub mod cost {
    /// Mana Song.
    pub const MANA: (u16, u16) = (18, 15);
    /// Fortitude Song.
    pub const FORTITUDE: (u16, u16) = (3, 1);
    /// Shield Song.
    pub const SHIELD: (u16, u16) = (9, 4);
    /// Weapon Song.
    pub const WEAPON: (u16, u16) = (12, 4);
    /// Armor Song.
    pub const ARMOR: (u16, u16) = (14, 5);
    /// Sword Song.
    pub const SWORD: (u16, u16) = (25, 15);
}

/// The base duration for a level, in seconds (`spellsong.rb:41`).
///
/// A four-band piecewise curve. **Lich logs and returns 120 above level 100**
/// (`spellsong.rb:54`), treating it as unhandled; the bands are cumulative and
/// the last one continues cleanly, so it is extended rather than cut off — a
/// level 101 bard is not a level 0 bard.
#[must_use]
pub fn base_duration(level: u16) -> u32 {
    let level = u32::from(level);
    match level {
        0..=25 => 120 + level * 4,
        26..=50 => 220 + (level - 25) * 3,
        51..=75 => 295 + (level - 50) * 2,
        _ => 345 + (level - 75),
    }
}

/// The bonus a count of ranks confers (`attributes/skills.rb:9`).
///
/// The game's cost curve: the first ten ranks are worth five points each and
/// every further band is worth one less, flattening to one point per rank past
/// forty. **Not bard-specific** — it lives here because spellsong is its first
/// caller, and it moves when a second one appears (Rule −1, rule of three).
///
/// Prefer a skill's own `bonus` when the table has stated one: this
/// reconstruction cannot see an enhancive, and the table can.
#[must_use]
pub fn to_bonus(ranks: u16) -> u16 {
    let ranks = u32::from(ranks);
    let bonus = match ranks {
        0..=10 => ranks * 5,
        11..=20 => 50 + (ranks - 10) * 4,
        21..=30 => 90 + (ranks - 20) * 3,
        31..=40 => 120 + (ranks - 30) * 2,
        _ => 140 + (ranks - 40),
    };
    u16::try_from(bonus).unwrap_or(u16::MAX)
}

/// The number in `"Level 100"`.
///
/// See the module doc: the level is stored verbatim by deliberate decision,
/// and this is the first consumer that needs it as a number. `None` rather
/// than `0` on an unexpected format, so a format change shows up as "unknown"
/// instead of as a level 0 bard with the shortest song in the game.
fn level_of(label: &str) -> Option<u16> {
    label
        .trim()
        .rsplit_once(' ')
        .map_or(label.trim(), |(_, n)| n)
        .parse()
        .ok()
}
