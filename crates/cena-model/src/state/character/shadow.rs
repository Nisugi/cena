//! Shadow essence: the store of up to five a character forms by shattering
//! souls and spends in sacrifice. Ports Lich's patterns (`infomon/parser.rb:55-62`)
//! and what each does to the count (`parser.rb:474-501`).
//!
//! | Line | The count | Lich |
//! |---|---|---|
//! | `Accumulated Shadow Essence: N`, in the `resource` report | N, at most 5 | `ShadowEssence` |
//! | a soul shattered into shadow essence | one more, at most 5 | `ShadowEssenceGain` |
//! | a sacrifice refused as too much | 5 | `ShadowEssenceCap` |
//! | mana drawn from it: *"... you feel N mana surge into you!"* | less the essences that mana took | `SacrificeMana` |
//! | channel, infest, fate or shift | one less | `SacrificeChannel`, `Infest`, `Fate`, `Shift` |
//!
//! # Where this differs from Lich
//!
//! - **An unknown count stays unknown.** Lich reads a count it was never told
//!   as 0 (`Lich::Resources.shadow_essence.to_i`), so a gain before the first
//!   `resource` says 1. Here a gain or a spend on an unknown count leaves it
//!   unknown (`plan/12` §5.2); the report, or reaching the cap, says it.
//! - **The mana estimate is the arithmetic Lich meant.** `SacrificeMana`
//!   writes `effective_mana_ranks.clamp(0, 60) / 120`, which Ruby's integer
//!   division makes 0 for every character, so its *"mana per essence after"*
//!   never grows with the ranks as the comment says. [`essences_for`] divides
//!   as a real number.

use super::standing::Standing;

/// The most shadow essence a character holds.
pub const MAX: u8 = 5;

/// What one line says of shadow essence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShadowEssence {
    /// The `resource` report states the count.
    Is(u8),
    /// One formed.
    Gained,
    /// At the cap.
    Full,
    /// Drawn as mana: this much mana surged in.
    Mana(u32),
    /// One spent.
    Spent,
}

/// Read a line for shadow essence.
#[must_use]
pub fn line(text: &str) -> Option<ShadowEssence> {
    let text = text.trim();
    if let Some(rest) = text
        .strip_prefix("Accumulated Shadow Essence: ")
        .or_else(|| text.strip_prefix("Accumulated Shadow essence: "))
    {
        let digit = rest.chars().next()?.to_digit(10)?;
        return u8::try_from(digit).ok().map(ShadowEssence::Is);
    }
    if text.starts_with("You violently shatter the bond on the soul of the ")
        && text.contains("forming shadow essence.")
    {
        return Some(ShadowEssence::Gained);
    }
    if text.starts_with(
        "You begin to sacrifice your victim but immediately sense that it would overwhelm you with shadow essence.",
    ) {
        return Some(ShadowEssence::Full);
    }
    if text.starts_with("You summon the shadow essence from the inner depths of your body") {
        let amount = text
            .split_once("There is a flood of power as you feel ")?
            .1
            .split_once(" mana surge into you!")?
            .0;
        return amount.trim().parse().ok().map(ShadowEssence::Mana);
    }
    let spent = [
        // `SacrificeChannel`: the start, and its own ending.
        text.starts_with("Focusing on the bond to your animate, you force shadow essence into ")
            && text.contains("the unnatural revitalization of its animate matter."),
        text.starts_with(
            "Mastering the struggle against the frantic rush of stolen power, you unleash a dark haze of necrosis upon your unfortunate victim.",
        ),
        text.starts_with(
            "You close your eyes momentarily and visualize the strands of fate that tie together the firmament.",
        ),
        text.starts_with(
            "Summoning the shadow essence within yourself, you will it to bleed through the veil",
        ),
    ];
    spent.contains(&true).then_some(ShadowEssence::Spent)
}

/// How many essences `amount` mana took, as `SacrificeMana` estimates it
/// (`parser.rb:486-497`): the first essence gives the level plus 20; each
/// after, half that and a share more by the better of Elemental and Spirit
/// Mana Control (and half the other), at most 60 ranks' worth. Between 1
/// and 5.
#[must_use]
pub fn essences_for(amount: u32, level: u16, emc: u16, smc: u16) -> u8 {
    let effective = f64::from(emc.max(smc)) + f64::from(emc.min(smc) / 2);
    let base = f64::from(level) + 20.0;
    let per_essence = base * (0.5 + effective.clamp(0.0, 60.0) / 120.0);
    let used = ((f64::from(amount) - per_essence) / (base * 0.5)).round();
    // Clamped to 1..=5 before the cast, so the cast cannot truncate.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to 1..=5 first"
    )]
    let used = used.clamp(1.0, f64::from(MAX)) as u8;
    used
}

impl Standing {
    /// Apply what a line said of shadow essence. `essences` estimates the
    /// essences a mana draw took. Returns whether the count changed.
    pub fn apply_shadow(&mut self, said: ShadowEssence, essences: impl FnOnce(u32) -> u8) -> bool {
        let before = self.shadow_essence;
        self.shadow_essence = match said {
            ShadowEssence::Is(count) => Some(count.min(MAX)),
            ShadowEssence::Full => Some(MAX),
            ShadowEssence::Gained => before.map(|count| (count + 1).min(MAX)),
            ShadowEssence::Spent => before.map(|count| count.saturating_sub(1)),
            ShadowEssence::Mana(amount) => {
                before.map(|count| count.saturating_sub(essences(amount)))
            }
        };
        before != self.shadow_essence
    }
}

#[cfg(test)]
mod tests {
    use super::{ShadowEssence, Standing, essences_for, line};

    /// `parser.rb:55`: the report, with either case of "essence".
    #[test]
    fn the_report_states_the_count() {
        assert_eq!(
            line("Accumulated Shadow Essence: 3"),
            Some(ShadowEssence::Is(3))
        );
        assert_eq!(
            line("Accumulated Shadow essence: 0"),
            Some(ShadowEssence::Is(0))
        );
        assert_eq!(line("Accumulated Shadow Essence: none"), None);
    }

    /// `parser.rb:56-62`: each line that moves the count.
    #[test]
    fn the_lines_that_move_it() {
        assert_eq!(
            line(
                "You violently shatter the bond on the soul of the troll.  As you draw it into yourself, you manipulate the chaotic and broken life forces, forming shadow essence."
            ),
            Some(ShadowEssence::Gained)
        );
        assert_eq!(
            line(
                "You begin to sacrifice your victim but immediately sense that it would overwhelm you with shadow essence."
            ),
            Some(ShadowEssence::Full)
        );
        assert_eq!(
            line(
                "You summon the shadow essence from the inner depths of your body, surrounding yourself in a dark halo of power.  You will the shadows into the eddies and currents of the flows of essence around you, spreading through them like blackened veins of corruption.  The surroundings glow with silent anguish.  Everything around you becomes pale and enervated with discoloration, like the life has been drained out of the world.  There is a flood of power as you feel 120 mana surge into you!"
            ),
            Some(ShadowEssence::Mana(120))
        );
        assert_eq!(
            line(
                "You close your eyes momentarily and visualize the strands of fate that tie together the firmament.  Identifying a susceptible star, you compel the shadow essence within you to corrupt it."
            ),
            Some(ShadowEssence::Spent)
        );
        assert_eq!(line("You swing a broadsword at a troll!"), None);
    }

    /// `parser.rb:474-501`, with an unknown count kept unknown.
    #[test]
    fn the_count_moves_and_holds_its_bounds() {
        let mut standing = Standing::default();
        assert!(
            !standing.apply_shadow(ShadowEssence::Gained, |_| 1),
            "unknown + 1 is unknown"
        );
        assert_eq!(standing.shadow_essence, None);
        assert!(standing.apply_shadow(ShadowEssence::Is(4), |_| 1));
        standing.apply_shadow(ShadowEssence::Gained, |_| 1);
        standing.apply_shadow(ShadowEssence::Gained, |_| 1);
        assert_eq!(standing.shadow_essence, Some(5), "at most 5");
        standing.apply_shadow(ShadowEssence::Mana(0), |_| 3);
        assert_eq!(standing.shadow_essence, Some(2));
        standing.apply_shadow(ShadowEssence::Spent, |_| 1);
        standing.apply_shadow(ShadowEssence::Spent, |_| 1);
        standing.apply_shadow(ShadowEssence::Spent, |_| 1);
        assert_eq!(standing.shadow_essence, Some(0), "not below 0");
        standing.apply_shadow(ShadowEssence::Full, |_| 1);
        assert_eq!(standing.shadow_essence, Some(5));
    }

    /// `parser.rb:486-497`: one essence's worth of mana is one; more, more;
    /// never under 1 or over 5.
    #[test]
    fn the_mana_estimate() {
        // Level 30, no mana control: base 50, 25 per essence after.
        assert_eq!(essences_for(50, 30, 0, 0), 1);
        assert_eq!(essences_for(100, 30, 0, 0), 3);
        assert_eq!(essences_for(10_000, 30, 0, 0), 5);
        assert_eq!(essences_for(0, 30, 0, 0), 1);
    }
}
