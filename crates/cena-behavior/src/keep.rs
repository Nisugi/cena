//! Keep: spellactive, a list of spells held up (`plan/37` Stage 4).
//!
//! spellactive (`reference/scripts/scripts/spellactive.lic`, 244 lines) loops
//! forever: for each listed spell that is down, cast it when it can be cast,
//! skipping the rooms listed as no-cast; and when mana is 25 or more short,
//! Sigil of Power. Its special cases are kept:
//!
//! - Barkskin (605) waits out its cooldown, and eight short spells wait out
//!   theirs (`[140, 211, 215, 219, 240, 919, 1619, 1650]`).
//! - Beacon of Courage (1699) is kept up by casting 1608.
//! - 606 and 640 are kept up by way of 625, which grants them.
//! - 506 and the Sigils of Minor/Major Bane and Protection only with
//!   something hostile in the room.
//! - A spell with a spirit cost only above 75% spirit.
//!
//! Not kept: the sign stagger (at most two spirit signs in three seconds),
//! and combat maneuvers kept by name (circle 96), which need the `CMan` model.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use cena_session::GameState;

use crate::cast::{self, Casting};
use crate::hunt::chain;

/// Seconds before a spell cast and still down is tried again.
const RETRY: u32 = 30;
/// spellactive's cooldown-bound short spells (`:204`).
const COOLDOWN_BOUND: &[u16] = &[140, 211, 215, 219, 240, 919, 1619, 1650];

/// The spells kept up, and where not to cast.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KeepProfile {
    /// Spell numbers, in the order they are checked.
    pub spells: Vec<u16>,
    /// Map room ids where nothing is cast (`nocast`).
    pub nocast: Vec<u32>,
    /// Sigil of Power when mana is 25 short (`power`, on by default).
    pub power: bool,
}

impl Default for KeepProfile {
    fn default() -> Self {
        Self {
            spells: Vec::new(),
            nocast: Vec::new(),
            power: true,
        }
    }
}

impl KeepProfile {
    /// Read a profile file's text.
    ///
    /// # Errors
    ///
    /// The text is not TOML, or names a key this does not know.
    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    /// The profile as a file's text.
    ///
    /// # Errors
    ///
    /// It cannot be written as TOML.
    pub fn to_toml(&self) -> Result<String, String> {
        toml::to_string_pretty(self).map_err(|e| e.to_string())
    }
}

/// The character's keep profile: `<data>/hunt/keep/<instance>_<character>.toml`.
#[must_use]
pub fn path(dir: &Path, instance: &str, character: &str) -> Option<PathBuf> {
    let file = chain::file_name(&format!("{instance}_{character}"))?;
    Some(dir.join("hunt").join("keep").join(format!("{file}.toml")))
}

/// What to send next to keep the list up, or `None` when nothing is down
/// that can be cast now. `room` is the map's room, for `nocast`; `tried`
/// holds when each spell was last sent, so a cast that did not take is not
/// sent again at every prompt.
#[must_use]
pub fn next(
    profile: &KeepProfile,
    state: &GameState,
    room: Option<u32>,
    tried: &mut BTreeMap<u16, u32>,
) -> Option<Vec<String>> {
    if room.is_some_and(|r| profile.nocast.contains(&r)) {
        return None;
    }
    let now = state.game_time_now()?;
    for &listed in &profile.spells {
        if is_up(state, listed, now) {
            continue;
        }
        let Some(spell) = substitute(state, listed, now) else {
            continue;
        };
        if tried
            .get(&spell)
            .is_some_and(|at| now.saturating_sub(*at) < RETRY)
        {
            continue;
        }
        if cast::ready(state, spell, 1, 0).is_err() || !spirit_allows(state, spell) {
            continue;
        }
        tried.insert(spell, now);
        let casting = Casting {
            spell,
            ..Casting::default()
        };
        return Some(casting.lines(state));
    }
    if profile.power
        && let Some(mana) = state.mana()
        && let (Some(now_mana), Some(max)) = (mana.current, mana.max)
        && max - now_mana > 25
        && state.known_spells.knows(9718) == Some(true)
        && tried
            .get(&9718)
            .is_none_or(|at| now.saturating_sub(*at) >= RETRY)
    {
        tried.insert(9718, now);
        return Some(vec!["sigil of power".to_owned()]);
    }
    None
}

/// Whether spell `number` is up, by the effects list.
fn is_up(state: &GameState, number: u16, now: u32) -> bool {
    state.effects.active(&number.to_string(), now) == Some(true)
}

/// The spell actually cast for a listed one that is down, or `None` when it
/// waits (`:200-230`).
fn substitute(state: &GameState, listed: u16, now: u32) -> Option<u16> {
    let name = cena_session::spells::spell(listed).map(|s| s.name.clone())?;
    let cooling = |text: &str| {
        state.effects.iter().any(|(id, e)| {
            e.category == "Cooldowns"
                && e.text.eq_ignore_ascii_case(text)
                && state.effects.active(id, now) == Some(true)
        })
    };
    if listed == 605 && cooling("Barkskin") {
        return None;
    }
    if COOLDOWN_BOUND.contains(&listed) && cooling(&name) {
        return None;
    }
    if listed == 1699 {
        return Some(1608);
    }
    if listed == 606 || listed == 640 {
        // 625 grants them; nothing else is cast for them.
        return (!is_up(state, 625, now)).then_some(625);
    }
    let war = listed == 506
        || [
            "Sigil of Minor Bane",
            "Sigil of Major Bane",
            "Sigil of Minor Protection",
            "Sigil of Major Protection",
        ]
        .iter()
        .any(|n| name.eq_ignore_ascii_case(n));
    // Something the game lets you attack is here: spellactive's non-passive NPC.
    if war && state.targeting.ids().is_empty() {
        return None;
    }
    Some(listed)
}

/// A spirit-cost spell only above 75% spirit (`:216`).
fn spirit_allows(state: &GameState, spell: u16) -> bool {
    let costs = state.spell_cost(spell, "spirit").unwrap_or(0.0);
    costs <= 0.0 || state.spirit().is_some_and(|s| s.percent > 75)
}

/// A change to the profile, from `;keep add 401`, `;keep del 401`, `;keep
/// nocast add 228`, `;keep nocast del 228`, `;keep nocast clear`, `;keep
/// power`, as spellactive takes them. What was done, in words.
///
/// # Errors
///
/// The words are not one of these, or name no spell.
pub fn edit(profile: &mut KeepProfile, words: &[&str]) -> Result<String, String> {
    let spell_of = |word: &str| -> Result<u16, String> {
        word.parse::<u16>()
            .ok()
            .filter(|n| cena_session::spells::spell(*n).is_some())
            .or_else(|| cena_session::spell_named(word).map(|s| s.number))
            .ok_or_else(|| format!("no spell {word}"))
    };
    match words {
        ["add", rest @ ..] if !rest.is_empty() => {
            let n = spell_of(&rest.join(" "))?;
            if profile.spells.contains(&n) {
                return Ok(format!("already keeping {n} up"));
            }
            profile.spells.push(n);
            Ok(format!("keeping {n} up"))
        }
        ["del" | "delete" | "remove" | "rem", rest @ ..] if !rest.is_empty() => {
            let n = spell_of(&rest.join(" "))?;
            profile.spells.retain(|s| *s != n);
            Ok(format!("no longer keeping {n} up"))
        }
        ["nocast", "add", room] => {
            let room: u32 = room.parse().map_err(|_| format!("no room {room}"))?;
            if !profile.nocast.contains(&room) {
                profile.nocast.push(room);
            }
            Ok(format!("nothing cast in room {room}"))
        }
        ["nocast", "del" | "delete" | "remove" | "rem", room] => {
            let room: u32 = room.parse().map_err(|_| format!("no room {room}"))?;
            profile.nocast.retain(|r| *r != room);
            Ok(format!("casting again in room {room}"))
        }
        ["nocast", "clear"] => {
            profile.nocast.clear();
            Ok("no rooms are no-cast".to_owned())
        }
        ["power"] => {
            profile.power = !profile.power;
            Ok(format!(
                "Sigil of Power when 25 short: {}",
                if profile.power { "on" } else { "off" }
            ))
        }
        _ => Err("keep, keep add|del <spell>, keep nocast add|del <room>, keep nocast clear, keep power, keep list".to_owned()),
    }
}
