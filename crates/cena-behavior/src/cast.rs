//! The casting step the spell behaviors share (`plan/37` Stage 3): Lich's
//! `Spell#cast` (`lib/common/spell.rb:627-800`), as a command, a set of
//! answers, and a readiness check.
//!
//! - **The command.** With no target, `incant N` (with `channel` or `evoke`
//!   and a multicast count when asked). At a target, `prepare N` then
//!   `cast|channel|evoke <target>`, since `incant` takes no target. A
//!   different spell already prepared is released first, as Lich does.
//! - **The answers** are Lich's `@@results_regex` and `@@prepare_regex`,
//!   typed: cast, fizzled, hindered, no mana, no target, a place spells of
//!   war are not cast, unable, and the rest.
//! - **Ready** is Lich's `check_energy`: the spell known, the mana, spirit
//!   and stamina its cost needs for this many casts (the spirit rule keeps
//!   one back), and no cast roundtime left.

use cena_session::GameState;

/// How the spell is sent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Verb {
    /// `incant`, or `prepare` and `cast` at a target.
    #[default]
    Cast,
    /// `channel`: more power, longer roundtime.
    Channel,
    /// `evoke`.
    Evoke,
}

impl Verb {
    /// The word, as the game takes it.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Cast => "cast",
            Self::Channel => "channel",
            Self::Evoke => "evoke",
        }
    }

    /// Read a word: `cast`, `incant`, `channel`, `evoke`.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        match word.to_ascii_lowercase().as_str() {
            "cast" | "incant" => Some(Self::Cast),
            "channel" => Some(Self::Channel),
            "evoke" => Some(Self::Evoke),
            _ => None,
        }
    }
}

/// The first spell number that is a society's power (9701, 9801, 9901).
const FIRST_SOCIETY_POWER: u16 = 9700;

/// The line that uses a society's power, at `target` when one is given.
/// `None` for a spell that is prepared and cast. A power sent as `incant
/// 9708` is answered "That is not something you can prepare." (the author's
/// hunt of 2026-09-30).
///
/// The line is Lich's own, read off the power's cast proc
/// ([`ProcLine`](cena_session::spells::ProcLine)), not made from its name:
/// Kai's Smite (9821) is `smite`, and eight symbols take the target the
/// proc appends (`symbol of submission #id`; the crate review of 2026-10-01,
/// MO-F-2). The target goes on only where the proc puts it.
///
/// # Errors
///
/// [`Unusable`] for a power with nothing to send: Lich's proc sends an empty
/// line (9803, 9808), refuses (9920), or there is none (9725, a timer).
#[must_use]
pub fn power(number: u16, target: Option<&str>) -> Option<Result<String, Unusable>> {
    use cena_session::spells::ProcLine;
    if number < FIRST_SOCIETY_POWER {
        return None;
    }
    let spell = cena_session::spells::spell(number)?;
    Some(match spell.extras.proc_line() {
        Some(ProcLine::Sends { line, targeted }) => Ok(match target {
            Some(target) if targeted => format!("{line} {target}"),
            _ => line,
        }),
        Some(ProcLine::Nothing) => Err(Unusable::Passive),
        Some(ProcLine::Refuses) => Err(Unusable::Refused),
        None => Err(Unusable::NotCast),
    })
}

/// Why a society's power has nothing to send ([`power`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unusable {
    /// Lich's proc sends an empty line: a power made passive (Kai's Strike,
    /// Symbol of Thought).
    Passive,
    /// Lich's proc refuses to cast it (Sign of Hopelessness).
    Refused,
    /// No proc at all: not something cast (Illusion - Demon, a timer).
    NotCast,
}

impl std::fmt::Display for Unusable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Passive => "it is passive: nothing is sent to use it",
            Self::Refused => "Lich refuses to cast it, and so does Hydra",
            Self::NotCast => "it is not something cast",
        })
    }
}

/// One casting.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Casting {
    /// The spell's number.
    pub spell: u16,
    /// At whom, as typed; `None` for no target.
    pub target: Option<String>,
    /// A multicast count; `None` for one.
    pub count: Option<u32>,
    /// How it is sent.
    pub verb: Verb,
}

impl Casting {
    /// The lines to send, in order. None for a society's power with nothing
    /// to send ([`power`]); a caller that can say why asks [`power`] first.
    #[must_use]
    pub fn lines(&self, state: &GameState) -> Vec<String> {
        if let Some(power) = power(self.spell, self.target.as_deref()) {
            return power.into_iter().collect();
        }
        let mut out = Vec::new();
        let name = cena_session::spells::spell(self.spell).map(|s| s.name.as_str());
        let other_prepared = state
            .prepared
            .as_deref()
            .is_some_and(|p| p != "None" && !name.is_some_and(|n| n.eq_ignore_ascii_case(p)));
        if other_prepared {
            out.push("release".to_owned());
        }
        let count = self.count.map(|n| format!(" {n}")).unwrap_or_default();
        match &self.target {
            None => {
                let verb = match self.verb {
                    Verb::Cast => String::new(),
                    other => format!(" {}", other.word()),
                };
                out.push(format!("incant {}{verb}{count}", self.spell));
            }
            Some(target) => {
                out.push(format!("prepare {}", self.spell));
                out.push(format!("{} {target}{count}", self.verb.word()));
            }
        }
        out
    }
}

/// What the game said to a casting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// `Cast Roundtime N Seconds.`: it went off.
    Cast,
    /// `Your magic fizzles ineffectually.`
    Fizzled,
    /// `[Spell Hindrance for ...]`: armor got in the way; try again.
    Hindered,
    /// `But you don't have any mana!`
    NoMana,
    /// `Cast at what?` / `You do not currently have a target.`
    NoTarget,
    /// `Be at peace my child...` / `Spells of War cannot be cast`.
    NotHere,
    /// Stunned, webbed, bound, silenced: `You are unable to do that right
    /// now.`, `You don't seem to be able to move`, `You can't think clearly`,
    /// the throat, the blood, a hand too injured.
    Unable,
    /// `You can only evoke certain spells.` / `...channel certain spells`.
    NoSuchVerb,
    /// `You do not know that spell!` / `That is not something you can
    /// prepare.`
    Unknown,
    /// `Your spell is ready.`: prepared, not yet cast.
    Ready,
    /// `You already have a spell readied!`
    AlreadyReady,
}

/// Read one line. `None` when it says nothing about a casting.
#[must_use]
pub fn classify(line: &str) -> Option<Answer> {
    let text = line.trim();
    let has = |needle: &str| text.contains(needle);
    let starts = |prefix: &str| text.starts_with(prefix);
    if (starts("Cast Roundtime ") || starts("Sing Roundtime ")) && text.ends_with('.') {
        return Some(Answer::Cast);
    }
    if starts("Your magic fizzles ineffectually") {
        return Some(Answer::Fizzled);
    }
    if starts("[Spell Hindrance for") {
        return Some(Answer::Hindered);
    }
    if starts("But you don't have any mana!") {
        return Some(Answer::NoMana);
    }
    if starts("Cast at what?") || starts("You do not currently have a target.") {
        return Some(Answer::NoTarget);
    }
    if starts("Be at peace my child") || has("Spells of War cannot be cast") {
        return Some(Answer::NotHere);
    }
    if starts("You are unable to do that right now.")
        || starts("You don't seem to be able to move to do that.")
        || starts("You can't think clearly enough to prepare a spell!")
        || starts("You can't make that dextrous of a move!")
        || starts("You are too injured to make that dextrous of a movement")
        || starts("The searing pain in your throat makes that impossible")
        || starts("All you manage to do is cough up some blood.")
        || has("keeps the spell from working.")
        || has("keep the spell from working.")
    {
        return Some(Answer::Unable);
    }
    if has("You can only evoke certain spells.")
        || has("You can only channel certain spells for extra power.")
    {
        return Some(Answer::NoSuchVerb);
    }
    if starts("You do not know that spell!") || has("That is not something you can prepare.") {
        return Some(Answer::Unknown);
    }
    if starts("Your spell is ready.") || starts("Your spellsong is ready.") {
        return Some(Answer::Ready);
    }
    if starts("You already have a spell readied!") {
        return Some(Answer::AlreadyReady);
    }
    None
}

/// Why a spell cannot be cast now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NotReady {
    /// The `spell` list says it is not known.
    NotKnown,
    /// Short of mana: needed, had.
    Mana(f64, i32),
    /// Short of spirit.
    Spirit,
    /// Short of stamina.
    Stamina,
    /// Cast roundtime left, in seconds.
    CastRoundtime(u32),
}

/// Whether spell `number` can be cast `count` times now, keeping `reserve`
/// mana back (Lich's `check_energy`, `spell.rb:629-652`). A cost the model
/// cannot work out, or a pool it has not been told, does not stop a cast:
/// the game says so if it is wrong.
///
/// # Errors
///
/// The reason it cannot, when the model knows one.
pub fn ready(state: &GameState, number: u16, count: u32, reserve: u32) -> Result<(), NotReady> {
    if state.known_spells.knows(u32::from(number)) == Some(false) {
        return Err(NotReady::NotKnown);
    }
    if let (Some(ends), Some(now)) = (state.cast_time_ends, state.game_time_now())
        && ends > now
    {
        return Err(NotReady::CastRoundtime(ends - now));
    }
    let times = f64::from(count.max(1));
    let current = |vital: Option<cena_session::Vital>| vital.and_then(|v| v.current);
    if let Some(cost) = state.spell_cost(number, "mana")
        && cost > 0.0
        && let Some(have) = current(state.mana())
    {
        let need = cost * times + f64::from(reserve);
        if f64::from(have) < need {
            return Err(NotReady::Mana(need, have));
        }
    }
    if let Some(cost) = state.spell_cost(number, "spirit")
        && cost > 0.0
        && current(state.spirit()).is_some_and(|have| f64::from(have) < cost + 1.0)
    {
        return Err(NotReady::Spirit);
    }
    if let Some(cost) = state.spell_cost(number, "stamina")
        && cost > 0.0
        && current(state.stamina()).is_some_and(|have| f64::from(have) < cost)
    {
        return Err(NotReady::Stamina);
    }
    Ok(())
}
