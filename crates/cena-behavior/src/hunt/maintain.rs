//! Maintain (`plan/30` §3, eohunter's priority 40): the profile's signs
//! kept up, cast when nothing is here to fight, each asked for again no
//! sooner than [`SIGN_RETRY`] seconds after a cast the effects list has not
//! confirmed; and Assume Aspect, cast as bigshot casts it.
//!
//! **Barkskin (605) has a lockout nothing lists.** After the bark absorbs a
//! blow it cannot be cast again for 301 seconds, and a cast in that time
//! only crumbles. The author's `cab.lic` tracks it from the absorb line
//! (`reference/lich_repo_mirror/lib/cab.lic:81-88`); the effect list
//! declares no cooldown for 605 (`spell_extras.tsv`, its row names none),
//! so the line is the only notice, and Maintain waits it out.

use cena_session::societies::voln;
use cena_session::{GameState, spells};

use super::engine::{Hunt, Phase};
use super::said::Said;

/// The dialog signs are listed under when they are up.
pub(super) const ACTIVE_SPELLS: &str = "Active Spells";
/// Seconds between two casts of the same sign, so a sign the game refused
/// is not asked for every tick.
const SIGN_RETRY: u32 = 60;
/// Barkskin.
const BARKSKIN: &str = "605";
/// Seconds Barkskin cannot be cast after it absorbs (`cab.lic:86`).
const BARK_LOCKOUT: u32 = 301;
/// The start of the absorb line, for the attack or magical energy.
const BARK_ABSORBS: &str = "The layer of bark on you hardens and absorbs the ";

/// The Voln symbols `check_favor` weighs, by spell number, and the name
/// the society table knows each by (`bigshot.lic:9264`).
const FAVOR_SYMBOLS: &[(&str, &str)] = &[
    ("9805", "courage"),
    ("9806", "protection"),
    ("9816", "supremacy"),
];

/// Whether sign `id`'s own cooldown is listed: the game would refuse it
/// (`bigshot.lic:9262` skips a sign so, for the spells it names).
fn cooling(state: &GameState, id: &str, now: u32) -> bool {
    let Some(name) = id
        .parse()
        .ok()
        .and_then(spells::spell)
        .map(|spell| spell.name.as_str())
    else {
        return false;
    };
    state
        .effects
        .in_category("Cooldowns")
        .filter(|(_, effect)| effect.text.eq_ignore_ascii_case(name))
        .any(|(key, _)| state.effects.active(key, now) == Some(true))
}

impl Hunt {
    /// With `check_favor`, a Voln symbol costs more favor than the
    /// character has, or the cost cannot be known (`bigshot.lic:9264-9271`;
    /// the cost from the society table, measured, not bigshot's formula).
    fn short_of_favor(&self, state: &GameState, id: &str) -> bool {
        if !self.profile.check_favor {
            return false;
        }
        let Some((_, name)) = FAVOR_SYMBOLS.iter().find(|(number, _)| *number == id) else {
            return false;
        };
        let level = state
            .character
            .experience
            .level
            .as_deref()
            .and_then(|l| l.parse().ok());
        let cost = voln::symbol(name)
            .zip(level)
            .and_then(|(symbol, level)| voln::favor_cost(&symbol.cost, level));
        match (cost, state.character.currency.voln_favor) {
            (Some(cost), Some(have)) => i64::from(cost) > have,
            _ => true,
        }
    }

    /// A line of the story at game second `now`: Barkskin absorbing starts
    /// its lockout.
    pub(super) fn bark_heard(&mut self, line: &str, now: Option<u32>) {
        if line.contains(BARK_ABSORBS) {
            self.bark_until = now.map(|now| now + BARK_LOCKOUT);
        }
    }

    /// A sign the effects list says is down, cast when nothing is here to fight.
    pub(super) fn maintain(&mut self, state: &GameState, now: Option<u32>) -> Option<Said> {
        if self.phase != Phase::Hunting || self.fightable(state).next().is_some() {
            return None;
        }
        let now = now?;
        let effects = &state.effects;
        let known = effects.saw_category(ACTIVE_SPELLS) || effects.saw_category("Buffs");
        let signs = self.profile.signs.clone();
        for sign in &signs {
            let id = sign.split_whitespace().next()?;
            if id == "650" {
                // Found by replaying real wire (`tests/hunt_replay.rs`):
                // before the lists arrive, every aspect reads as down.
                if !known {
                    continue;
                }
                if let Some(said) = self.assume_aspect(sign, state, now) {
                    return Some(said);
                }
                continue;
            }
            let up = match effects.active(id, now) {
                Some(up) => up,
                None if known => false,
                None => continue,
            };
            let recent = self
                .signs_cast
                .get(id)
                .is_some_and(|at| now.saturating_sub(*at) < SIGN_RETRY);
            let locked = id == BARKSKIN && self.bark_until.is_some_and(|until| now < until);
            if up || recent || locked || cooling(state, id, now) || self.short_of_favor(state, id) {
                continue;
            }
            self.signs_cast.insert(id.to_owned(), now);
            return Some(Said::Send {
                line: format!("incant {sign}"),
                target: None,
            });
        }
        None
    }

    /// Assume Aspect (`650 <aspect> <aspect|evoke>`), as bigshot casts it
    /// (`cmd_assume`, `bigshot.lic:5588-5645`): nothing while an aspect
    /// named is up; the spell first, evoked when the second word is `evoke`
    /// and prepared otherwise; then `assume <aspect>` for each aspect whose
    /// buff is down, once the spell is up. One step a tick, each confirmed
    /// by the effects list before the next, and each with its own retry
    /// window: a spell that fails to land is asked for again in a minute,
    /// not at every prompt.
    pub(super) fn assume_aspect(
        &mut self,
        sign: &str,
        state: &GameState,
        now: u32,
    ) -> Option<Said> {
        let mut words = sign.split_whitespace().skip(1);
        let first = words.next()?.to_ascii_lowercase();
        let second = words.next().map(str::to_ascii_lowercase);
        let evoke = second.as_deref() == Some("evoke");
        let aspects: Vec<String> = std::iter::once(first)
            .chain(second.filter(|word| word != "evoke"))
            .collect();
        let up = |text: &str| {
            state.effects.iter().any(|(id, effect)| {
                effect.text.eq_ignore_ascii_case(text)
                    && state.effects.active(id, now) == Some(true)
            })
        };
        if aspects
            .iter()
            .any(|aspect| up(&format!("Aspect of the {aspect}")))
        {
            return None;
        }
        let spell_up = state.effects.active("650", now) == Some(true) || up("Assume Aspect");
        let (key, line) = if spell_up {
            let aspect = aspects.first()?;
            ("650 assume", format!("assume {aspect}"))
        } else if evoke {
            ("650", "incant 650 evoke".to_owned())
        } else {
            ("650", "prep 650".to_owned())
        };
        let recent = self
            .signs_cast
            .get(key)
            .is_some_and(|at| now.saturating_sub(*at) < SIGN_RETRY);
        if recent {
            return None;
        }
        self.signs_cast.insert(key.to_owned(), now);
        Some(Said::Send { line, target: None })
    }
}
