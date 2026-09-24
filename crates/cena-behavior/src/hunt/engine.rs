//! The hunt as a pure state machine (`plan/30` §3): the profile and the
//! game state in, **one thing to do** out.
//!
//! [`Hunt::tick`] has no socket, no clock and no random numbers. It is told
//! where the character is and what time it is, and it answers with a
//! [`Said`]: send this line, walk there, wait, or stop. The `async` layer
//! that does those things is [`super::drive`]. The split is travel's
//! (`plan/24`), and it is what lets a whole hunt be driven frame by frame in
//! a test with no game.
//!
//! # eohunter's engine, as an enum
//!
//! eohunter's `Engine` asks its behaviors in priority order whether they
//! want control, and the first that does acts once (`runner.rb`). Here the
//! behaviors are the arms of one `match`, in eohunter's order, under **one**
//! holder of the authority (`plan/12` §4.2):
//!
//! | Priority | eohunter | Here | Does |
//! |---|---|---|---|
//! | 0 | Survival | `Hunt::survival` | dead: stop; down and able: stand |
//! | 10 | Flee | `Hunt::flee` | too many, or a creature the profile always flees: leave the room |
//! | 20 | Rest | `Hunt::rest` | wounded, fried, encumbered or out of mana: walk to the rest room, rest, walk back, prepare |
//! | 30 | Loot | `Hunt::loot` | `loot #id` on each dead creature, once; the stand-in for eloot (M6c) |
//! | 40 | Maintain | `Hunt::maintain` | a sign the effects say is down: cast it, when no target is here |
//! | 50 | Engage | `Hunt::engage` | choose a target, target it, take the hunting stance, run its routine one step a tick |
//! | 60 | Wander | `Hunt::wander` | nothing to fight: wait, then walk to a fresh room inside the boundaries |
//!
//! No arm keeps position: intent is re-derived from the state each tick
//! (`behavior.rb:10-13`). What the machine remembers is only what the state
//! cannot say -- which corpses it has looted, which step of the routine is
//! next, which rooms it has wandered through, and which phase of a rest it
//! is in.
//!
//! # Unknown holds
//!
//! Every reading is three-valued, and `None` means the game has not said
//! (`plan/12` §5.2). A guard whose fact is unknown holds its step
//! ([`Condition::holds`](super::guard::Condition::holds)); a rest threshold
//! whose vital is unknown keeps resting; a room the map cannot place is not
//! wandered from. The machine acts on what the game has said, and waits
//! for the rest.

use cena_map::RoomId;
use cena_session::claim::{Claim, claim_room};
use cena_session::{Able, GameState, Injuries};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::profile::{Profile, Step, Target};
pub use super::said::{Ending, Here, Phase, Said, Why};
use crate::stance::{self, Want};

/// The dialog signs are listed under when they are up.
const ACTIVE_SPELLS: &str = "Active Spells";
/// Seconds between two casts of the same sign, so a sign the game refused
/// is not asked for every tick.
const SIGN_RETRY: u32 = 60;
/// Seconds to rest before asking the state again.
const REST_BEAT: u32 = 5;
/// The most steps skipped in one tick before the routine gives up the tick.
const MAX_SKIPS: usize = 32;

/// The machine. See the module docs.
#[derive(Debug)]
pub struct Hunt {
    profile: Profile,
    phase: Phase,
    /// The room the last tick saw, by the game's number.
    room: Option<String>,
    /// The game second the current room was entered.
    arrived: Option<u32>,
    /// Corpses already looted, by id.
    looted: BTreeSet<i64>,
    /// The creature being fought, its routine, and the next step.
    target: Option<i64>,
    routine: String,
    cursor: usize,
    /// A sequence being played out, one step a tick.
    queue: VecDeque<Step>,
    /// Rooms wandered through, oldest first.
    visited: Vec<RoomId>,
    /// Kills seen since the mind filled, for `rest.overkill`.
    fried_kills: u32,
    dead_seen: BTreeSet<i64>,
    /// Commands still to send in the current phase (rest, prepare).
    pending: VecDeque<String>,
    /// When each sign was last cast.
    signs_cast: BTreeMap<String, u32>,
    /// Things to tell the player, taken by the driver.
    notes: Vec<String>,
    said_aspect: bool,
    seed: u64,
}

impl Hunt {
    /// A hunt on `profile`, seeded for wander's choices.
    #[must_use]
    pub fn new(profile: Profile, seed: u64) -> Self {
        Self {
            profile,
            phase: Phase::Hunting,
            room: None,
            arrived: None,
            looted: BTreeSet::new(),
            target: None,
            routine: "a".to_owned(),
            cursor: 0,
            queue: VecDeque::new(),
            visited: Vec::new(),
            fried_kills: 0,
            dead_seen: BTreeSet::new(),
            pending: VecDeque::new(),
            signs_cast: BTreeMap::new(),
            notes: Vec::new(),
            said_aspect: false,
            seed,
        }
    }

    /// The profile being hunted on.
    #[must_use]
    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    /// Where the hunt is in its cycle.
    #[must_use]
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// The creature being fought, if any.
    #[must_use]
    pub fn target(&self) -> Option<i64> {
        self.target
    }

    /// What the machine wants the player told, since it was last asked.
    pub fn take_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notes)
    }

    /// A walk the driver could not make. A wander goal is given up; a rest
    /// or return goal ends the hunt.
    pub fn walk_failed(&mut self, to: RoomId) -> Option<Ending> {
        match self.phase {
            Phase::Hunting => {
                self.visited.retain(|room| *room != to);
                self.visited.insert(0, to);
                None
            }
            _ => Some(Ending::Unreachable(to)),
        }
    }

    /// The gate refused the target: it is gone.
    pub fn target_gone(&mut self) {
        self.target = None;
        self.queue.clear();
    }

    /// One turn: what to do now, against `state` as it is, standing in
    /// `here`, at game second `now`.
    pub fn tick(&mut self, state: &GameState, here: Here<'_>, now: Option<u32>) -> Said {
        self.note_room(state, now);
        if let Some(said) = Self::survival(state) {
            return said;
        }
        if let Some(said) = self.rest(state, here) {
            return said;
        }
        if let Some(said) = self.flee(state, here, now) {
            return said;
        }
        if let Some(said) = self.loot(state) {
            return said;
        }
        if let Some(said) = self.maintain(state, now) {
            return said;
        }
        if let Some(said) = self.engage(state, now) {
            return said;
        }
        self.wander(state, here, now).unwrap_or(Said::Nothing)
    }

    /// The room changed: what was true of the last room is not of this one.
    fn note_room(&mut self, state: &GameState, now: Option<u32>) {
        if state.room.id == self.room {
            return;
        }
        self.room.clone_from(&state.room.id);
        self.arrived = now;
        self.target = None;
        self.queue.clear();
    }

    // --- survival ----------------------------------------------------------

    /// Dead: the hunt is over. Down, and able to move: stand.
    fn survival(state: &GameState) -> Option<Said> {
        let status = state.status.known();
        if status.dead() == Some(true) {
            return Some(Said::Done(Ending::Dead));
        }
        let down = status.standing() == Some(false)
            || status.prone() == Some(true)
            || status.sitting() == Some(true)
            || status.kneeling() == Some(true);
        let held = status.stunned() == Some(true) || status.webbed() == Some(true);
        (down && !held).then(|| Said::Send {
            line: "stand".to_owned(),
            target: None,
        })
    }

    // --- flee ---------------------------------------------------------------

    /// Too many fightable creatures, or one the profile always flees from.
    fn flee(&mut self, state: &GameState, here: Here<'_>, now: Option<u32>) -> Option<Said> {
        if self.phase != Phase::Hunting {
            return None;
        }
        let flee = &self.profile.flee;
        let fightable = self.fightable(state).count();
        let crowd = flee.count.is_some_and(|limit| fightable > limit as usize);
        let always = state.creatures().in_room().any(|creature| {
            flee.from
                .iter()
                .any(|name| named(name, &creature.name, creature.noun.as_deref()))
        });
        if !(crowd || always) {
            return None;
        }
        let to = self.next_room(here, now)?;
        Some(Said::Walk(to))
    }

    // --- rest -----------------------------------------------------------------

    /// The rest cycle: reasons to go, the walk there, the wait, the walk
    /// back, and the prepare commands.
    fn rest(&mut self, state: &GameState, here: Here<'_>) -> Option<Said> {
        match self.phase {
            Phase::Hunting => {
                let why = self.rest_reason(state)?;
                let Some(resting) = self.profile.rooms.resting else {
                    return Some(Said::Done(Ending::NoRestingRoom));
                };
                self.phase = Phase::ToRest(why);
                self.notes
                    .push(format!("{why}: walking to the resting room."));
                Some(self.step_toward(
                    RoomId(resting),
                    here,
                    Phase::Resting(why),
                    &self.profile.rest.commands.clone(),
                ))
            }
            Phase::ToRest(why) => {
                let resting = RoomId(self.profile.rooms.resting?);
                Some(self.step_toward(
                    resting,
                    here,
                    Phase::Resting(why),
                    &self.profile.rest.commands.clone(),
                ))
            }
            Phase::Resting(_) => {
                if let Some(line) = self.pending.pop_front() {
                    return Some(Said::Send { line, target: None });
                }
                if let Some(still) = self.still_resting(state) {
                    let _ = still;
                    return Some(Said::Wait(REST_BEAT));
                }
                self.fried_kills = 0;
                let Some(hunting) = self.profile.rooms.hunting else {
                    return Some(Said::Done(Ending::NoHuntingRoom));
                };
                self.phase = Phase::Returning;
                self.notes.push("rested: walking back.".to_owned());
                Some(self.step_toward(
                    RoomId(hunting),
                    here,
                    Phase::Preparing,
                    &self.profile.prepare.clone(),
                ))
            }
            Phase::Returning => {
                let hunting = RoomId(self.profile.rooms.hunting?);
                Some(self.step_toward(
                    hunting,
                    here,
                    Phase::Preparing,
                    &self.profile.prepare.clone(),
                ))
            }
            Phase::Preparing => {
                if let Some(line) = self.pending.pop_front() {
                    return Some(Said::Send { line, target: None });
                }
                self.phase = Phase::Hunting;
                self.notes.push("hunting.".to_owned());
                None
            }
        }
    }

    /// Walk toward `goal`; on arrival, move to `then` with `commands` to send.
    fn step_toward(
        &mut self,
        goal: RoomId,
        here: Here<'_>,
        then: Phase,
        commands: &[String],
    ) -> Said {
        if here.room == Some(goal) {
            self.phase = then;
            self.pending = commands.iter().cloned().collect();
            return match self.pending.pop_front() {
                Some(line) => Said::Send { line, target: None },
                None => Said::Wait(1),
            };
        }
        Said::Walk(goal)
    }

    /// Why to rest now, if a reason holds.
    fn rest_reason(&self, state: &GameState) -> Option<Why> {
        let rest = &self.profile.rest;
        if self.wounded(state) {
            return Some(Why::Wounded);
        }
        let mind = state.character.experience.mind_percent;
        let fried = rest
            .fried
            .filter(|at| *at <= 100)
            .zip(mind)
            .is_some_and(|(at, mind)| mind >= at);
        if fried && self.fried_kills >= rest.overkill {
            return Some(Why::Fried);
        }
        let heavy = rest
            .encumbered
            .zip(state.character.encumbrance_percent)
            .is_some_and(|(at, now)| now >= at);
        if heavy {
            return Some(Why::Encumbered);
        }
        let dry = rest
            .mana_below
            .zip(state.mana())
            .is_some_and(|(below, mana)| mana.percent < below);
        dry.then_some(Why::Mana)
    }

    /// `rest.when`: any one holding is enough.
    fn wounded(&self, state: &GameState) -> bool {
        let when = &self.profile.rest.when;
        if when.bleeding && state.status.known().bleeding() == Some(true) {
            return true;
        }
        if when
            .health_at_most
            .zip(state.health())
            .is_some_and(|(at_most, health)| health.percent <= at_most)
        {
            return true;
        }
        let injuries = Injuries::new(&state.character.injuries);
        (when.cannot_cast && injuries.able_to_cast() != Able::Yes)
            || (when.cannot_use_ranged && injuries.able_to_use_ranged() != Able::Yes)
    }

    /// Why the rest is not over, or `None` when it is. A threshold whose
    /// vital the game has not stated keeps resting.
    fn still_resting(&self, state: &GameState) -> Option<&'static str> {
        if self.wounded(state) {
            return Some("wounded");
        }
        let until = &self.profile.rest.until;
        let mind = state.character.experience.mind_percent;
        if until
            .experience
            .is_some_and(|at| mind.is_none_or(|mind| mind > at))
        {
            return Some("mind still above threshold");
        }
        if until
            .mana
            .is_some_and(|at| state.mana().is_none_or(|v| v.percent < at))
        {
            return Some("mana still below threshold");
        }
        if until.spirit.is_some_and(|at| {
            state
                .spirit()
                .and_then(|v| v.current)
                .is_none_or(|spirit| spirit < i32::try_from(at).unwrap_or(i32::MAX))
        }) {
            return Some("spirit still below threshold");
        }
        if until
            .stamina
            .is_some_and(|at| state.stamina().is_none_or(|v| v.percent < at))
        {
            return Some("stamina still below threshold");
        }
        None
    }

    // --- loot -------------------------------------------------------------------

    /// `loot #id` on a corpse not yet looted, unless the profile delays
    /// looting while targets remain.
    fn loot(&mut self, state: &GameState) -> Option<Said> {
        if self.phase != Phase::Hunting {
            return None;
        }
        let corpses: Vec<i64> = state
            .creatures()
            .in_room()
            .filter(|creature| creature.dead())
            .map(|creature| creature.id)
            .collect();
        for id in &corpses {
            if self.dead_seen.insert(*id) {
                self.fried_kills = self.fried_kills.saturating_add(1);
            }
        }
        if self.profile.loot.delay && self.fightable(state).next().is_some() {
            return None;
        }
        let corpse = corpses.into_iter().find(|id| !self.looted.contains(id))?;
        self.looted.insert(corpse);
        Some(Said::Send {
            line: format!("loot #{corpse}"),
            target: None,
        })
    }

    // --- maintain -------------------------------------------------------------

    /// A sign the effects list says is down, cast when nothing is here to fight.
    fn maintain(&mut self, state: &GameState, now: Option<u32>) -> Option<Said> {
        if self.phase != Phase::Hunting || self.fightable(state).next().is_some() {
            return None;
        }
        let now = now?;
        let effects = &state.effects;
        let known = effects.saw_category(ACTIVE_SPELLS) || effects.saw_category("Buffs");
        for sign in &self.profile.signs {
            let id = sign.split_whitespace().next()?;
            if id == "650" {
                if !self.said_aspect {
                    self.said_aspect = true;
                    self.notes.push(format!(
                        "sign `{sign}`: Assume Aspect is not cast by Hydra yet; cast it yourself."
                    ));
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
            if up || recent {
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

    // --- engage -----------------------------------------------------------------

    /// Choose a target, target it, take the hunting stance, and run one step
    /// of its routine.
    fn engage(&mut self, state: &GameState, now: Option<u32>) -> Option<Said> {
        if self.phase != Phase::Hunting || !Self::room_is_mine(state) {
            return None;
        }
        let target = self.choose_target(state)?;
        if state.targeting.current() != Some(target) {
            return Some(Said::Send {
                line: format!("target #{target}"),
                target: Some(target),
            });
        }
        if let Some(line) = Self::stance_for(self.profile.stance.hunting.as_deref(), state) {
            return Some(Said::Send {
                line,
                target: Some(target),
            });
        }
        let _ = now;
        self.next_step(state, target)
    }

    /// The step to send now: the sequence in play first, then the routine
    /// from its cursor, skipping held steps, expanding sequences, and
    /// skipping steps whose guards do not hold.
    fn next_step(&mut self, state: &GameState, target: i64) -> Option<Said> {
        let steps = self.profile.routines.get(&self.routine)?.clone();
        if steps.is_empty() {
            return None;
        }
        for _ in 0..MAX_SKIPS {
            let step = if let Some(step) = self.queue.pop_front() {
                step
            } else {
                let step = steps.get(self.cursor % steps.len())?.clone();
                self.cursor = (self.cursor + 1) % steps.len();
                step
            };
            if step.held.is_some() {
                continue;
            }
            if let Some(sequence) = self.profile.sequences.get(&step.send) {
                if sequence.is_empty() {
                    continue;
                }
                let mut queued: VecDeque<Step> = sequence.iter().cloned().collect();
                queued.append(&mut self.queue);
                self.queue = queued;
                continue;
            }
            let runs = step
                .when
                .iter()
                .all(|condition| condition.holds(state, Some(target)) == Some(true));
            if !runs {
                continue;
            }
            return Some(Said::Send {
                line: step.send.clone(),
                target: Some(target),
            });
        }
        Some(Said::Wait(1))
    }

    /// The target: the current one while it is still here and worth
    /// attacking, else the best by the profile's order.
    fn choose_target(&mut self, state: &GameState) -> Option<i64> {
        if let Some(current) = self.target
            && self.fightable(state).any(|creature| creature.id == current)
        {
            return Some(current);
        }
        let mut best: Option<(usize, i64, String)> = None;
        for creature in self.fightable(state) {
            let Some((rank, target)) = self.rank(&creature.name, creature.noun.as_deref()) else {
                continue;
            };
            if best.as_ref().is_none_or(|(at, _, _)| rank < *at) {
                best = Some((rank, creature.id, target.routine.clone()));
            }
        }
        let (_, id, routine) = best?;
        self.target = Some(id);
        self.routine = routine;
        self.cursor = 0;
        self.queue.clear();
        Some(id)
    }

    /// The first target entry that fits a creature, with its place.
    fn rank(&self, name: &str, noun: Option<&str>) -> Option<(usize, &Target)> {
        self.profile.targets.iter().enumerate().find(|(_, target)| {
            target.any || target.name.as_deref().is_some_and(|n| named(n, name, noun))
        })
    }

    /// The creatures here worth attacking: alive, not an animate or a bare
    /// appendage, not ignored, and named by the target list.
    fn fightable<'a>(
        &'a self,
        state: &'a GameState,
    ) -> impl Iterator<Item = &'a cena_session::CreatureInstance> + 'a {
        state.creatures().in_room().filter(move |creature| {
            creature.valid_target()
                && !self
                    .profile
                    .ignore
                    .iter()
                    .any(|name| named(name, &creature.name, creature.noun.as_deref()))
                && self
                    .rank(&creature.name, creature.noun.as_deref())
                    .is_some()
        })
    }

    /// The room is this character's to fight in (`claim::claim_room`).
    fn room_is_mine(state: &GameState) -> bool {
        let with_me: Vec<String> = state
            .group
            .members()
            .iter()
            .map(|member| member.noun.clone())
            .collect();
        matches!(claim_room(&state.room, &with_me), Claim::Mine)
    }

    /// The stance command to send, if the profile names one for this phase
    /// and the bar does not show it yet.
    fn stance_for(want: Option<&str>, state: &GameState) -> Option<String> {
        let want = Want::parse(want?).ok()?;
        if stance::landed(want, state) {
            return None;
        }
        stance::command(want, state)
    }

    // --- wander -------------------------------------------------------------------

    /// Nothing to fight: wait the profile's moment, take the wander stance,
    /// then walk to a fresh room.
    fn wander(&mut self, state: &GameState, here: Here<'_>, now: Option<u32>) -> Option<Said> {
        if self.phase != Phase::Hunting || self.fightable(state).next().is_some() {
            return None;
        }
        let waited = self.arrived.zip(now).is_none_or(|(arrived, now)| {
            f64::from(now.saturating_sub(arrived)) >= self.profile.wander.wait
        });
        if !waited {
            return Some(Said::Wait(1));
        }
        if let Some(line) = Self::stance_for(self.profile.stance.wander.as_deref(), state) {
            return Some(Said::Send { line, target: None });
        }
        let to = self.next_room(here, now)?;
        Some(Said::Walk(to))
    }

    /// The next room to walk to: a crossable exit not on the boundary, a
    /// fresh one if any, else the one least recently visited (`flee.rb`'s
    /// `Walker`).
    fn next_room(&mut self, here: Here<'_>, now: Option<u32>) -> Option<RoomId> {
        let room = here.room?;
        if !self.visited.contains(&room) {
            self.visited.push(room);
        }
        let boundaries = &self.profile.rooms.boundaries;
        let options: Vec<RoomId> = here
            .exits
            .iter()
            .copied()
            .filter(|exit| !boundaries.contains(&exit.0) && *exit != room)
            .collect();
        if options.is_empty() {
            return None;
        }
        let fresh: Vec<RoomId> = options
            .iter()
            .copied()
            .filter(|exit| !self.visited.contains(exit))
            .collect();
        let chosen = if fresh.is_empty() {
            self.visited
                .iter()
                .copied()
                .find(|seen| options.contains(seen))?
        } else {
            let roll = self.roll(now);
            let at = usize::try_from(roll % fresh.len() as u64).unwrap_or(0);
            fresh[at]
        };
        self.visited.retain(|seen| *seen != chosen);
        self.visited.push(chosen);
        Some(chosen)
    }

    /// The next number from the seed, mixed with the clock so two hunts on
    /// one seed do not walk in step.
    fn roll(&mut self, now: Option<u32>) -> u64 {
        let mut x = self.seed ^ u64::from(now.unwrap_or(0));
        x ^= x >> 33;
        x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
        x ^= x >> 33;
        x = x.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
        x ^= x >> 33;
        self.seed = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
        x
    }
}

/// Whether `wanted` names a creature: its noun, or its whole name, ignoring
/// case (`bigshot.lic:7170`).
fn named(wanted: &str, name: &str, noun: Option<&str>) -> bool {
    name.eq_ignore_ascii_case(wanted) || noun.is_some_and(|noun| noun.eq_ignore_ascii_case(wanted))
}
