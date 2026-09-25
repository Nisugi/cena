//! `censer_between_actions`: Ethereal Censer (320) cast between routine
//! steps whenever it is off cooldown and affordable. The author's reading
//! of bigshot's `censer` word, which bigshot could only hang on one step
//! (`plan/33` §2h).
//!
//! bigshot's `handle_censer` (`bigshot.lic:4530-4542`) casts it before a
//! step when the Ethereal Censer cooldown is not up, 320 is known, and the
//! mana covers 320 **and** the step's own spell when the step is one. 320
//! itself costs nothing: *"Ethereal Censer has no mana cost"*
//! (`reference/wiki_clean/Ethereal Censer _320_.txt:14`), as Lich's table
//! has it (`spell_extras.tsv`, `mana0`), so it is the step's cost that
//! counts; only a cast lost to Spell Hindrance costs 20 (`:42`). Here
//! the censer goes first and the step waits in the queue for the next
//! tick, where its guards are asked again. A cast the game did not take is
//! asked for again no sooner than [`CENSER_RETRY`] seconds later, as a
//! sign is.

use cena_session::GameState;

use super::engine::Hunt;
use crate::cast;

/// Ethereal Censer.
const CENSER: u16 = 320;
/// Seconds before a censer the cooldown has not yet shown is cast again.
const CENSER_RETRY: u32 = 10;

impl Hunt {
    /// The censer to cast before `send`, when the profile asks for it and
    /// now is the time.
    pub(super) fn censer_first(
        &mut self,
        state: &GameState,
        send: &str,
        now: Option<u32>,
    ) -> Option<String> {
        if !self.profile.censer_between_actions
            || state.known_spells.knows(u32::from(CENSER)) != Some(true)
        {
            return None;
        }
        let now = now?;
        if self
            .censer_cast
            .is_some_and(|at| now.saturating_sub(at) < CENSER_RETRY)
        {
            return None;
        }
        let cooling = state
            .effects
            .in_category("Cooldowns")
            .filter(|(_, effect)| effect.text.starts_with("Ethereal Censer"))
            .any(|(id, _)| state.effects.active(id, now) == Some(true));
        if cooling {
            return None;
        }
        cast::ready(state, CENSER, 1, 0).ok()?;
        if let Some(step) = spell_of(send).and_then(|n| state.spell_cost(n, "mana")) {
            let censer = state.spell_cost(CENSER, "mana").unwrap_or(0.0);
            let have = state.mana()?.current?;
            if f64::from(have) < censer + step {
                return None;
            }
        }
        self.censer_cast = Some(now);
        Some(format!("incant {CENSER}"))
    }
}

/// The spell a step casts, as bigshot's pattern reads it (`:4531`):
/// `incant 611`, or a bare `611` with its verb after.
fn spell_of(send: &str) -> Option<u16> {
    let mut words = send.split_whitespace();
    let first = words.next()?;
    let number = if first.eq_ignore_ascii_case("incant") {
        words.next()?
    } else {
        first
    };
    number.parse().ok()
}
