//! Mana from a society before a rest for it: bigshot's `use_wracking`
//! (`wrack`, `bigshot.lic:6852-6862`).
//!
//! When the hunt would rest for mana, with `rest.wracking` on, it first
//! uses whichever the character has, in bigshot's order:
//!
//! | Society | Ability | When |
//! |---|---|---|
//! | Council of Light | Sign of Wracking (9918) | 9012 not on, spirit at least `rest.wracking_spirit` and at least 6 plus the signs up (9912, 9913, 9914, and 9916 counted three times, as bigshot counts it) |
//! | Guardians of Sunfist | Sigil of Power (9718) | stamina at least 50; bigshot sends it `stamina / 50` times, here once a tick |
//! | Order of Voln | Symbol of Mana (9813) | its cooldown not on |
//!
//! A wrack is sent at most every [`AGAIN`] seconds, so one the mana bar has
//! not yet shown is not sent twice.

use cena_session::GameState;
use cena_session::Society;

use super::engine::Hunt;

/// Seconds between two wracks.
const AGAIN: u32 = 10;

impl Hunt {
    /// The wrack to send now instead of resting for mana, if any.
    pub(super) fn wrack(&mut self, state: &GameState, now: Option<u32>) -> Option<String> {
        if !self.profile.rest.wracking {
            return None;
        }
        let now = now?;
        if self
            .wracked
            .is_some_and(|at| now.saturating_sub(at) < AGAIN)
        {
            return None;
        }
        let up = |id: &str| state.effects.active(id, now) == Some(true);
        let spirit = state.spirit().and_then(|v| v.current).unwrap_or(0);
        let stamina = state.stamina().and_then(|v| v.current).unwrap_or(0);
        let signs = ["9912", "9913", "9914", "9916", "9916", "9916"]
            .iter()
            .filter(|id| up(id))
            .count();
        let floor = i32::try_from(self.profile.rest.wracking_spirit).unwrap_or(i32::MAX);
        let line = if knows(state, 9918)
            && !up("9012")
            && spirit >= floor
            && spirit >= 6 + i32::try_from(signs).unwrap_or(0)
        {
            "sign of wracking"
        } else if knows(state, 9718) && stamina >= 50 {
            "sigil of power"
        } else if knows(state, 9813)
            && !state.effects.iter().any(|(id, effect)| {
                effect.category == "Cooldowns"
                    && effect.text == "Symbol of Mana"
                    && state.effects.active(id, now) == Some(true)
            })
        {
            "symbol of mana"
        } else {
            return None;
        };
        self.wracked = Some(now);
        Some(line.to_owned())
    }
}

/// Whether the character's society rank gives this ability.
fn knows(state: &GameState, number: u16) -> bool {
    let standing = &state.character.standing;
    let (Some(Some(society)), Some(rank)) = (standing.society, standing.society_rank) else {
        return false;
    };
    let society: Society = society;
    society
        .abilities()
        .iter()
        .any(|ability| ability.spell_number == number && ability.known_at(rank))
}
