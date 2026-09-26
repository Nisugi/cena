//! What each guard word reads, against [`Facts`]: the fact before polarity,
//! or `None` when the game has not said (the module docs of
//! [`super`] give each word's source).

use cena_session::claim::{Claim, claim_room};
use cena_session::{CreatureInstance, GameState, PositionTier, PsmCategory, StatusName, gameobj};

use super::{Fact, Facts, Guard, Measure};

/// The dialog Empowered buffs are listed under.
const BUFFS: &str = "Buffs";

/// bigshot's `PRONE_STATUSES` (`bigshot.lic:2736`), which `down` reads.
const DOWN: &[StatusName] = &[
    StatusName::Sleeping,
    StatusName::Webbed,
    StatusName::Stunned,
    StatusName::Kneeling,
    StatusName::Sitting,
    StatusName::Prone,
    StatusName::Immobilized,
];

/// The only `ancient` creature bigshot counts as not ancient (`:4404`).
const NOT_ANCIENT: &str = "ancient ghoul master";

impl Guard {
    /// The fact, before polarity.
    pub(super) fn read(&self, facts: &Facts<'_>) -> Option<bool> {
        let state = facts.state;
        let now = state.game_time_now();
        match self {
            Self::Is(fact) => fact.read(facts, now),
            Self::Amount(measure, n) => measure.read(facts, now, *n),
            Self::Effect(dialog, name) => {
                let now = now?;
                let title = dialog.title();
                state
                    .effects
                    .saw_category(title)
                    .then(|| up(state, title, name, now))
            }
            Self::Expiring { name, within } => {
                let now = now?;
                if state.effects.is_empty() {
                    return None;
                }
                // Lapsing unless some effect of that name is up with more
                // than `within` left; one with no end time never lapses.
                let wanted = name.to_ascii_lowercase();
                let holding = state
                    .effects
                    .iter()
                    .filter(|(_, effect)| effect.text.to_ascii_lowercase().starts_with(&wanted))
                    .any(|(id, _)| {
                        state.effects.active(id, now) == Some(true)
                            && state
                                .effects
                                .remaining(id, now)
                                .is_none_or(|left| left > *within)
                    });
                Some(!holding)
            }
            Self::Injured { part, rank } => Some(u32::from(target(facts)?.injury(*part)) >= *rank),
        }
    }
}

impl Fact {
    fn read(self, facts: &Facts<'_>, now: Option<u32>) -> Option<bool> {
        let state = facts.state;
        let known = state.status.known();
        match self {
            Self::Hidden => known.hidden(),
            Self::SelfKneeling => known.kneeling(),
            Self::Diseased => known.diseased(),
            Self::Poisoned => known.poisoned(),
            Self::Outside => state
                .room
                .component("room exits")
                .map(|runs| runs.plain().trim_start().starts_with("Obvious paths")),
            Self::Splashy => tagged(facts, "splashy"),
            Self::NoMagic => tagged(facts, "nomagic"),
            Self::Alone => alone(state),
            Self::Status(status) => Some(target(facts)?.has_status(status, now)),
            Self::Down => {
                let creature = target(facts)?;
                Some(DOWN.iter().any(|status| creature.has_status(*status, now)))
            }
            Self::Flag(class) => {
                let creature = target(facts)?;
                creature.flags_known().then(|| creature.flag(class))
            }
            Self::Undead => of_type(target(facts)?, "undead"),
            Self::Noncorporeal => of_type(target(facts)?, "noncorporeal"),
            Self::Ancient => {
                let name = target(facts)?.name.to_ascii_lowercase();
                Some(
                    (name.starts_with("grizzled ") || name.starts_with("ancient "))
                        && name != NOT_ANCIENT,
                )
            }
            Self::FatalCrit => Some(target(facts)?.fatal_crit()),
            Self::Smote => Some(target(facts)?.smote(now)),
            Self::TierUp => Some(target(facts)?.ucs_tierup(now).is_some()),
            Self::Helpless => Some(target(facts)?.muckled(now)),
            Self::CoupReady => {
                let creature = target(facts)?;
                let psms = &state.character.psms;
                if !psms.has_table(PsmCategory::CombatManeuver) || !hp_known(creature) {
                    return None;
                }
                let rank = psms
                    .get(PsmCategory::CombatManeuver, "coupdegrace")
                    .map_or(0, |ranks| u32::from(ranks.ranks));
                Some(creature.coup_eligible(rank, now))
            }
            Self::Once => {
                let target = facts.target?;
                Some(!facts.used.is_some_and(|used| used.at(facts.step, target)))
            }
            Self::OnceHere => Some(!facts.used.is_some_and(|used| used.here(facts.step))),
        }
    }
}

impl Measure {
    fn read(self, facts: &Facts<'_>, now: Option<u32>, n: u32) -> Option<bool> {
        let state = facts.state;
        match self {
            Self::HealthAtLeast => state.health().map(|vital| vital.percent >= n),
            Self::ManaAtLeast => points(state.mana()?.current).map(|have| have >= n),
            Self::StaminaAtLeast => points(state.stamina()?.current).map(|have| have >= n),
            Self::SpiritAtLeast => points(state.spirit()?.current).map(|have| have >= n),
            Self::EncumbranceAtLeast => state.character.encumbrance_percent.map(|at| at >= n),
            Self::TargetHealthAtMost => {
                let creature = target(facts)?;
                hp_known(creature).then(|| creature.hp_percent() <= f64::from(n))
            }
            Self::EmpoweredBelow => {
                let now = now?;
                state
                    .effects
                    .saw_category(BUFFS)
                    .then(|| strongest_empowered(state, now).is_none_or(|bonus| bonus < n))
            }
            Self::Position => Some(tier(target(facts)?, now) == n),
            Self::PositionAtLeast => Some(tier(target(facts)?, now) >= n),
            Self::StunnedFor => Some(target(facts)?.stunned_for(now) >= n),
            Self::TargetsAtLeast => Some(valid_targets(state) >= n),
            Self::TargetsAtMost => Some(valid_targets(state) <= n),
            Self::Every => facts
                .used
                .map_or(Some(true), |used| used.due(facts.step, n, now)),
        }
    }
}

/// The creature the step is aimed at, when there is one and it is known.
fn target<'a>(facts: &Facts<'a>) -> Option<&'a CreatureInstance> {
    facts.state.creatures().get(facts.target?)
}

/// Health that is known: stated by the game, or the bestiary's. Anything
/// else is `FALLBACK_MAX_HP` guessing, which `thp` must not act on
/// (`plan/33` §2f).
fn hp_known(creature: &CreatureInstance) -> bool {
    creature.hp_is_stated() || creature.has_template()
}

/// A bar's current amount as points; `None` for a bar that states none.
fn points(current: Option<i32>) -> Option<u32> {
    current.map(|have| u32::try_from(have).unwrap_or(0))
}

/// My unarmed position on this creature as a tier; none is tier 0.
fn tier(creature: &CreatureInstance, now: Option<u32>) -> u32 {
    match creature.ucs_position(now) {
        None => 0,
        Some(PositionTier::Decent) => 1,
        Some(PositionTier::Good) => 2,
        Some(PositionTier::Excellent) => 3,
    }
}

/// Whether the map tags this room so; unknown when the map could not place
/// the character.
fn tagged(facts: &Facts<'_>, tag: &str) -> Option<bool> {
    facts.tags.map(|tags| tags.iter().any(|t| t == tag))
}

/// No player here but my group: the room's claim, as the engine claims a
/// room on entry.
fn alone(state: &GameState) -> Option<bool> {
    let with_me: Vec<String> = state
        .group
        .members()
        .iter()
        .map(|member| member.noun.clone())
        .chain(state.character.name.clone())
        .collect();
    match claim_room(&state.room, &with_me) {
        Claim::Mine => Some(true),
        Claim::Contested { .. } => Some(false),
        Claim::Unknown => None,
    }
}

/// bigshot's `npc.type` check: the creature's `gameobj-data` types.
fn of_type(creature: &CreatureInstance, kind: &str) -> Option<bool> {
    let noun = creature.noun.as_deref()?;
    Some(gameobj::classify(noun, &creature.name).is(kind))
}

/// Valid targets in the room, as bigshot's `mob` and `valid` count them.
fn valid_targets(state: &GameState) -> u32 {
    let count = state
        .creatures()
        .in_room()
        .filter(|creature| creature.valid_target())
        .count();
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// Whether an effect whose name starts `name` (ignoring case) is up in the
/// dialog titled `title`.
fn up(state: &GameState, title: &str, name: &str, now: u32) -> bool {
    let wanted = name.to_ascii_lowercase();
    state
        .effects
        .in_category(title)
        .filter(|(_, effect)| effect.text.to_ascii_lowercase().starts_with(&wanted))
        .any(|(id, _)| state.effects.active(id, now) == Some(true))
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
