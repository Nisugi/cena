//! Spellcaster: a spell by number or alias, cast the way the player set it
//! up (`plan/37` Stage 6).
//!
//! spellcaster (`reference/lich_repo_mirror/lib/spellcaster.lic`, 418 lines,
//! the old Lich repository's) catches a typed `401 bob 3` before the game
//! sees it and casts it: an alias for the number, a verb set for the spell
//! (`cast`, `channel`, `evoke`), a stance taken first and `guarded` after,
//! and four switches -- `conserve` refuses a cast short of mana or at a
//! target not in the room, `safety` refuses an attack spell with nothing
//! hostile here, `channel` channels what the spell table says can be, and
//! `stance` takes the offensive stance for what the table marks as wanting
//! it.
//!
//! Here it is `;sc 401 bob 3`, and, with the `typed` switch on (the
//! default), a bare `401 bob 3` or `boom bob` as spellcaster takes it
//! (`SPELL_RX`, `ALIAS_RX`, `:43-44`; author, 2026-09-25: *"I don't want to
//! have to type ;sc to cast a spell with just the number"*). Every switch is
//! the player's, on or off: `;sc set typed off` gives the game its numbers
//! back.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use cena_session::GameState;

use crate::cast::{self, Casting, NotReady, Verb};
use crate::hunt::chain;

/// spellcaster's stances.
const STANCES: &[&str] = &[
    "offensive",
    "advance",
    "forward",
    "neutral",
    "guarded",
    "defensive",
];

/// How the player set spells up (`CharSettings`, `:20-27`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "spellcaster's four switches, carried as they are"
)]
pub struct CasterProfile {
    /// Names for spells: `boom = 910`.
    pub alias: BTreeMap<String, u16>,
    /// The verb for a spell or an alias: `cast`, `channel` or `evoke`.
    pub verbs: BTreeMap<String, String>,
    /// The stance for a spell or an alias, taken before and left after.
    pub stance: BTreeMap<String, String>,
    /// Channel what the spell table says can be.
    pub channel: bool,
    /// Refuse a cast short of mana, or at a target not here.
    pub conserve: bool,
    /// Refuse an attack spell with nothing hostile here.
    pub safety: bool,
    /// Take the offensive stance for what the table marks as wanting it.
    pub stance_all: bool,
    /// Cast a typed `401` or alias with no `;sc` before it ([`typed`]).
    pub typed: bool,
}

impl Default for CasterProfile {
    /// Everything empty and off, but `typed`: spellcaster exists to catch
    /// what is typed.
    fn default() -> Self {
        Self {
            alias: BTreeMap::new(),
            verbs: BTreeMap::new(),
            stance: BTreeMap::new(),
            channel: false,
            conserve: false,
            safety: false,
            stance_all: false,
            typed: true,
        }
    }
}

/// The words of a line typed without the command symbol, when it is a
/// spell to cast: its first word is a spell's number (three or four
/// digits, as `SPELL_RX` has it, and in the spell table) or one of the
/// player's aliases. `None`: the game's line, and always so with `typed`
/// off.
#[must_use]
pub fn typed(profile: &CasterProfile, line: &str) -> Option<Vec<String>> {
    if !profile.typed {
        return None;
    }
    let words: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
    let first = words.first()?;
    let number = (3..=4).contains(&first.len())
        && first.bytes().all(|b| b.is_ascii_digit())
        && first
            .parse::<u16>()
            .is_ok_and(|n| cena_session::spells::spell(n).is_some());
    (number || profile.alias.contains_key(&first.to_ascii_lowercase())).then_some(words)
}

impl CasterProfile {
    /// Read a profile file's text.
    ///
    /// # Errors
    ///
    /// Not TOML, or a key this does not know.
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

/// The character's spellcaster profile: `<data>/hunt/sc/<instance>_<character>.toml`.
#[must_use]
pub fn path(dir: &Path, instance: &str, character: &str) -> Option<PathBuf> {
    let file = chain::file_name(&format!("{instance}_{character}"))?;
    Some(dir.join("hunt").join("sc").join(format!("{file}.toml")))
}

/// The lines for `;sc <spell|alias> [target] [count]` (`cast`, `:318-395`).
///
/// # Errors
///
/// Why it is not cast, in words.
pub fn lines(
    profile: &CasterProfile,
    state: &GameState,
    words: &[&str],
) -> Result<Vec<String>, String> {
    let (first, rest) = words
        .split_first()
        .ok_or("sc <spell|alias> [target] [count]")?;
    let alias = first.to_ascii_lowercase();
    let number = first
        .parse::<u16>()
        .ok()
        .or_else(|| profile.alias.get(&alias).copied())
        .ok_or_else(|| format!("no spell or alias {first}"))?;
    let spell = cena_session::spells::spell(number).ok_or_else(|| format!("no spell {number}"))?;
    let (target, count) = match rest {
        [] => (None, None),
        [n] if n.parse::<u32>().is_ok() => (None, n.parse().ok()),
        [t] => (Some((*t).to_owned()), None),
        [t @ .., n] if n.parse::<u32>().is_ok() => (Some(t.join(" ")), n.parse().ok()),
        t => (Some(t.join(" ")), None),
    };
    if state.known_spells.knows(u32::from(number)) == Some(false) {
        return Err(format!("you do not know {}", spell.name));
    }
    let attack = spell.kind.as_deref().is_some_and(|k| k.contains("attack"));
    if profile.conserve
        && let Err(NotReady::Mana(..)) = cast::ready(state, number, count.unwrap_or(1), 0)
    {
        return Err(format!("not enough mana for {}", spell.name));
    }
    if profile.conserve
        && attack
        && let Some(target) = &target
    {
        let here = state.room.creatures.iter().any(|c| {
            c.text
                .to_ascii_lowercase()
                .contains(&target.to_ascii_lowercase())
        });
        if !here {
            return Err(format!("nothing here matches {target}"));
        }
    }
    if profile.safety && attack && state.targeting.ids().is_empty() {
        return Err(format!("nothing hostile here to cast {} at", spell.name));
    }
    let key = |map: &BTreeMap<String, String>| {
        map.get(&alias)
            .or_else(|| map.get(&number.to_string()))
            .cloned()
    };
    let mut verb = key(&profile.verbs)
        .and_then(|v| Verb::parse(&v))
        .unwrap_or_default();
    if profile.channel && spell.extras.channel && verb == Verb::Cast {
        verb = Verb::Channel;
    }
    let stance = key(&profile.stance)
        .or_else(|| (profile.stance_all && spell.extras.stance).then(|| "offensive".to_owned()));
    let mut out = Vec::new();
    if let Some(stance) = &stance {
        out.push(format!("stance {stance}"));
    }
    out.extend(
        Casting {
            spell: number,
            target,
            count,
            verb,
        }
        .lines(state),
    );
    if stance.is_some() {
        out.push("stance guarded".to_owned());
    }
    Ok(out)
}

/// A change to the profile (`;sc alias`, `verb`, `stance`, `set`, `:120-310`).
/// What was done, in words.
///
/// # Errors
///
/// The words are not one of these.
pub fn edit(profile: &mut CasterProfile, words: &[&str]) -> Result<String, String> {
    match words {
        ["alias", "clear", name] => {
            profile.alias.remove(*name);
            Ok(format!("removed alias {name}"))
        }
        ["alias", spell, name] => {
            let n: u16 = spell.parse().map_err(|_| format!("no spell {spell}"))?;
            profile.alias.insert(name.to_ascii_lowercase(), n);
            Ok(format!("{name} casts {n}"))
        }
        ["verb", spell, "clear"] => {
            profile.verbs.remove(*spell);
            Ok(format!("{spell} casts with the default verb"))
        }
        ["verb", spell, verb] => {
            Verb::parse(verb).ok_or("the verbs are cast, channel and evoke")?;
            profile.verbs.insert((*spell).to_owned(), (*verb).to_owned());
            Ok(format!("{spell} is sent with {verb}"))
        }
        ["stance", spell, "clear"] => {
            profile.stance.remove(*spell);
            Ok(format!("{spell} takes no stance"))
        }
        ["stance", spell, stance] => {
            if !STANCES.contains(stance) {
                return Err(format!("the stances are {}", STANCES.join(", ")));
            }
            profile.stance.insert((*spell).to_owned(), (*stance).to_owned());
            Ok(format!("{spell} is cast from {stance}"))
        }
        ["set", option, value @ ("on" | "off")] => {
            let on = *value == "on";
            match *option {
                "channel" => profile.channel = on,
                "conserve" => profile.conserve = on,
                "safety" => profile.safety = on,
                "stance" => profile.stance_all = on,
                "typed" => profile.typed = on,
                other => return Err(format!("no option {other}: channel, conserve, safety, stance, typed")),
            }
            Ok(format!("{option} {value}"))
        }
        _ => Err("sc <spell|alias> [target] [count], sc alias <spell> <name>, sc verb <spell> <verb>, sc stance <spell> <stance>, sc set <option> on|off".to_owned()),
    }
}
