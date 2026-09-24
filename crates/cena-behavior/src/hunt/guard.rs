//! The guard vocabulary: the preconditions a routine step may carry
//! (`plan/30` §5; `plan/33` for every word bigshot has and what became of it).
//!
//! A guard names **when the step runs**. `(hidden)` runs a step only while I
//! am hidden; `(!hidden)` only while I am not. That is one rule for every
//! word, where bigshot has two (`plan/33` §1: `frozen`, `prone`, `rooted`
//! and `voidweaver` read the other way round), and the importer turns
//! bigshot's readings into this one.
//!
//! # Three answers, and unknown skips
//!
//! [`Condition::holds`] answers `Some(true)`, `Some(false)` or `None`: the
//! game has not said (`plan/12` §5.2). **A step whose guard is unknown does
//! not run.** That is bigshot's rule for `thp`, where unknown health skips
//! (`bigshot.lic:4336`), made the rule for every word: a guard is there to
//! hold a step back, and acting on a belief nothing supports is the failure
//! `StatusInfo::known` exists to prevent.
//!
//! # The words
//!
//! | Word | Runs when | Reads |
//! |---|---|---|
//! | `hidden` | I am hidden | `status.known().hidden()` |
//! | `thp N` | the target's health is **known** and at or below N percent | `hp_percent`, when the game stated the HP or the bestiary knows the creature |
//! | `empowered_below N` | no Empowered buff of +N or more is up | the `Buffs` dialog, once the game has stated it |
//! | `immobilized` | the target is immobilized | `has_status(Immobilized)` |
//! | `expiring "<name>" N` | the named effect is up with N seconds or less left | the effects, by display name, whichever dialog |
//!
//! Each takes `!` in front. These are the words Nisugi's profile needs
//! (`plan/30` §5); the rest of bigshot's are `plan/33`'s rows, built as
//! profiles need them.
//!
//! # `expiring` reads the other way from bigshot's `buff`
//!
//! bigshot's `(buff5)` on `kweed` **skips** while Tangleweed Vigor is up
//! with five seconds or less left: "don't fire into the expiry window"
//! (`bigshot.lic:4240`). Under the one rule, the word names the window and
//! the step that must not run inside it carries the negation:
//! `kweed (!expiring "Tangleweed Vigor" 5)`. `plan/33` §5 spells that step
//! `expiring "Tangleweed Vigor" 5`, with the polarity inside the word's
//! meaning; this file keeps the polarity in the `!`, so that the one rule
//! holds for every word. The author's review of `plan/33` decides the name.

use std::fmt;

use cena_session::{GameState, StatusName};

/// The dialog Empowered buffs are listed under.
const BUFFS: &str = "Buffs";

/// The words, each with what follows it. What an error message lists.
pub const WORDS: &[&str] = &[
    "hidden",
    "thp N",
    "empowered_below N",
    "immobilized",
    "expiring \"<name>\" N",
];

/// One precondition on a step, from the closed vocabulary above.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Guard {
    /// `hidden`: I am hidden.
    Hidden,
    /// `thp N`: the target's health is known and at or below `N` percent.
    TargetHealthAtMost(u32),
    /// `empowered_below N`: no Empowered buff of `+N` or more is up.
    EmpoweredBelow(u32),
    /// `immobilized`: the target is immobilized.
    TargetImmobilized,
    /// `expiring "<name>" N`: the named effect is up with `N` seconds or
    /// less left.
    Expiring {
        /// The effect's display name, as the game lists it.
        name: String,
        /// Seconds left, at most.
        within: u32,
    },
}

/// A guard and its polarity: `!hidden` runs the step when I am not hidden.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Condition {
    /// The fact read.
    pub guard: Guard,
    /// `!` was written: the step runs when the guard does **not** hold.
    pub negated: bool,
}

impl Condition {
    /// Read one group of guards, the parentheses already gone:
    /// `thp 20 empowered_below 30`, or `!expiring "Tangleweed Vigor" 5`.
    ///
    /// # Errors
    ///
    /// A word Hydra does not know, a number or name missing or malformed, or
    /// a quote left open. The message says which, and lists the words.
    pub fn parse_group(text: &str) -> Result<Vec<Self>, String> {
        let mut out = Vec::new();
        let mut it = tokens(text)?.into_iter();
        while let Some(token) = it.next() {
            let (negated, word) = token
                .strip_prefix('!')
                .map_or((false, token.as_str()), |word| (true, word));
            let guard = match word {
                "" => return Err("`!` needs a guard after it".to_owned()),
                "hidden" => Guard::Hidden,
                "immobilized" => Guard::TargetImmobilized,
                "thp" => Guard::TargetHealthAtMost(number(word, it.next())?),
                "empowered_below" => Guard::EmpoweredBelow(number(word, it.next())?),
                "expiring" => {
                    let name = quoted(word, it.next())?;
                    let within = number(word, it.next())?;
                    Guard::Expiring { name, within }
                }
                other => {
                    return Err(format!(
                        "`{other}` is not a guard Hydra knows. The guards are: {}",
                        WORDS.join(", ")
                    ));
                }
            };
            out.push(Self { guard, negated });
        }
        Ok(out)
    }

    /// Whether the step may run, against `state` as it is now, with `target`
    /// the creature the step is aimed at (`None`: no target chosen).
    ///
    /// `None` when the game has not said, and the step then does not run.
    #[must_use]
    pub fn holds(&self, state: &GameState, target: Option<i64>) -> Option<bool> {
        self.guard
            .read(state, target)
            .map(|holds| holds != self.negated)
    }
}

impl Guard {
    /// The fact, before polarity.
    fn read(&self, state: &GameState, target: Option<i64>) -> Option<bool> {
        let now = state.game_time_now();
        match self {
            Self::Hidden => state.status.known().hidden(),
            Self::TargetHealthAtMost(at_most) => {
                let creature = state.creatures().get(target?)?;
                (creature.hp_is_stated() || creature.has_template())
                    .then(|| creature.hp_percent() <= f64::from(*at_most))
            }
            Self::TargetImmobilized => {
                let creature = state.creatures().get(target?)?;
                Some(creature.has_status(StatusName::Immobilized, now))
            }
            Self::EmpoweredBelow(below) => {
                let now = now?;
                state
                    .effects
                    .saw_category(BUFFS)
                    .then(|| strongest_empowered(state, now).is_none_or(|bonus| bonus < *below))
            }
            Self::Expiring { name, within } => {
                let now = now?;
                if state.effects.is_empty() {
                    return None;
                }
                Some(state.effects.iter().any(|(id, effect)| {
                    effect.text.eq_ignore_ascii_case(name)
                        && state.effects.active(id, now) == Some(true)
                        && state
                            .effects
                            .remaining(id, now)
                            .is_some_and(|left| left <= *within)
                }))
            }
        }
    }
}

impl fmt::Display for Condition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.negated {
            f.write_str("!")?;
        }
        match &self.guard {
            Guard::Hidden => f.write_str("hidden"),
            Guard::TargetImmobilized => f.write_str("immobilized"),
            Guard::TargetHealthAtMost(n) => write!(f, "thp {n}"),
            Guard::EmpoweredBelow(n) => write!(f, "empowered_below {n}"),
            Guard::Expiring { name, within } => write!(f, "expiring \"{name}\" {within}"),
        }
    }
}

/// The largest `+N` among the Empowered buffs up now, if any is.
fn strongest_empowered(state: &GameState, now: u32) -> Option<u32> {
    state
        .effects
        .in_category(BUFFS)
        .filter(|(id, _)| state.effects.active(id, now) == Some(true))
        .filter_map(|(_, effect)| empowered_bonus(&effect.text))
        .max()
}

/// `Empowered (+30)` is 30.
fn empowered_bonus(text: &str) -> Option<u32> {
    text.strip_prefix("Empowered (+")?
        .strip_suffix(')')?
        .parse()
        .ok()
}

/// Split on whitespace, keeping `"a quoted name"` as one token.
///
/// # Errors
///
/// A quote opened and not closed.
pub(crate) fn tokens(text: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in text.chars() {
        if c == '"' {
            quoted = !quoted;
            current.push(c);
        } else if c.is_whitespace() && !quoted {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
        } else {
            current.push(c);
        }
    }
    if quoted {
        return Err(format!("a quote was opened and not closed in `{text}`"));
    }
    if !current.is_empty() {
        out.push(current);
    }
    Ok(out)
}

/// The number a word takes.
fn number(word: &str, token: Option<String>) -> Result<u32, String> {
    let token = token.ok_or_else(|| format!("`{word}` needs a number after it"))?;
    token
        .parse()
        .map_err(|_| format!("`{word}` needs a number after it, not `{token}`"))
}

/// The quoted name a word takes, quotes removed.
fn quoted(word: &str, token: Option<String>) -> Result<String, String> {
    let token = token.ok_or_else(|| format!("`{word}` needs a name in double quotes after it"))?;
    token
        .strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .filter(|inner| !inner.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("`{word}` needs a name in double quotes after it, not `{token}`"))
}
