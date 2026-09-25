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
//! # The room is claimed on arrival
//!
//! Whether a room is the hunt's is decided **once, when it is entered**, as
//! Lich's `Claim` decides it from the room the character walked into
//! (`claim.rb:87-107`, run as the room loads): nobody else there, and no
//! stranger's disk unless `wander.ignore_disks` (`bigshot.lic:7091-7099`).
//! Read every tick instead, another player walking in stalled the hunt in
//! place: engage refused the room, and wander would not leave a room with
//! creatures in it (`inventory/12` §2). A room entered claimed stays the
//! hunt's; a room entered contested is neither fought in nor looted
//! (`bigshot.lic:7808`), and is walked on from.
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
use cena_session::GameState;
use cena_session::claim::{Claim, claim_room};

/// Whose the room is, decided on entering it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Held {
    /// Nobody else was here.
    Mine,
    /// Nobody else, but a stranger's disk: looted, not fought in, unless
    /// the profile ignores disks.
    Disk,
    /// Someone else was here first.
    Theirs,
}
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::profile::{Profile, Step, Target};
use super::react::Reacting;
use super::replies::Heard;
pub use super::said::{Ending, Here, Phase, Said, Why};
use crate::heal::HealProfile;
use crate::keep::KeepProfile;
use crate::loot::{Left, LootProfile};
use crate::stance::{self, Want};
use crate::waggle::WaggleProfile;

/// The dialog signs are listed under when they are up.
const ACTIVE_SPELLS: &str = "Active Spells";
/// Seconds between two casts of the same sign, so a sign the game refused
/// is not asked for every tick.
const SIGN_RETRY: u32 = 60;
/// Seconds to rest before asking the state again.
pub(super) const REST_BEAT: u32 = 5;
/// The most steps skipped in one tick before the routine gives up the tick.
const MAX_SKIPS: usize = 32;
/// With `loot.delay`, how often a corpse is looted while targets remain:
/// bigshot's `time_between(:need_to_loot?, 15)` (`bigshot.lic:7824`),
/// whose first call passes, so the first corpse is looted at once.
const LOOT_SPACING: u32 = 15;

/// The machine. See the module docs.
#[derive(Debug)]
pub struct Hunt {
    pub(super) profile: Profile,
    pub(super) phase: Phase,
    /// The room the last tick saw, by the game's number.
    pub(super) room: Option<String>,
    /// The game second the current room was entered.
    pub(super) arrived: Option<u32>,
    /// Corpses already looted, by id.
    pub(super) looted: BTreeSet<i64>,
    /// The creature being fought, its routine, and the next step.
    pub(super) target: Option<i64>,
    pub(super) routine: String,
    pub(super) cursor: usize,
    /// A sequence being played out, one step a tick.
    pub(super) queue: VecDeque<Step>,
    /// Rooms wandered through, oldest first.
    pub(super) visited: Vec<RoomId>,
    /// Kills seen since the mind filled, for `rest.overkill`.
    pub(super) fried_kills: u32,
    pub(super) dead_seen: BTreeSet<i64>,
    /// When a corpse was last looted with targets still here (`loot.delay`).
    pub(super) delayed_loot: Option<u32>,
    /// Commands still to send in the current phase (rest, prepare).
    pub(super) pending: VecDeque<String>,
    /// When each sign was last cast.
    pub(super) signs_cast: BTreeMap<String, u32>,
    /// Things to tell the player, taken by the driver.
    pub(super) notes: Vec<String>,
    pub(super) seed: u64,
    /// The character's loot profile, when one was imported (`plan/31`):
    /// corpses are then looted by the planner, not by a bare `loot #id`.
    pub(super) loot: Option<LootProfile>,
    /// A reason to rest the loot planner handed in, until the rest starts.
    pub(super) must_rest: Option<Why>,
    /// The heal profile, when the character has one (`plan/36`).
    pub(super) heal: Option<HealProfile>,
    /// `;heal`: no hunt, one heal. `Some(false)` until it has been asked for.
    pub(super) heal_only: Option<bool>,
    /// `;heal stock` / `;heal fill`: no hunt, one round; `fill` beside it.
    pub(super) stock_only: Option<(bool, bool)>,
    /// `;keep`: no hunt, the listed spells kept up until stopped, with when
    /// each was last sent.
    pub(super) keep_only: Option<(KeepProfile, BTreeMap<u16, u32>)>,
    /// `;waggle`: no hunt, one run over these names; `true` once asked for.
    pub(super) waggle_only: Option<(WaggleProfile, Vec<String>, bool)>,
    /// `;sc`: no hunt, these lines sent in order, then done.
    pub(super) send_only: Option<VecDeque<String>>,
    /// `--spellcast` and `--ranged` for the heal.
    pub(super) heal_mode: (bool, bool),
    /// What the game's replies taught ([`super::replies`]).
    pub(super) heard: Heard,
    /// Whose this room is, once the game has said who is in it.
    pub(super) held: Option<Held>,
    /// What incidents left to do ([`super::react`]).
    pub(super) react: Reacting,
    /// Rests finished, for `rest.stop_after`.
    pub(super) rests: u32,
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
            delayed_loot: None,
            pending: VecDeque::new(),
            signs_cast: BTreeMap::new(),
            notes: Vec::new(),
            seed,
            loot: None,
            must_rest: None,
            heal: None,
            heal_only: None,
            stock_only: None,
            keep_only: None,
            waggle_only: None,
            send_only: None,
            heal_mode: (false, false),
            heard: Heard::default(),
            held: None,
            react: Reacting::default(),
            rests: 0,
        }
    }

    /// Heal with herbs by this profile during a rest.
    #[must_use]
    pub fn with_heal(mut self, profile: HealProfile) -> Self {
        self.heal = Some(profile);
        self
    }

    /// The heal profile, when there is one.
    #[must_use]
    pub fn heal_profile(&self) -> Option<&HealProfile> {
        self.heal.as_ref()
    }

    /// `--spellcast` and `--ranged` for this heal.
    #[must_use]
    pub const fn heal_mode(&self) -> (bool, bool) {
        self.heal_mode
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

    /// Loot by this profile, with the planner, instead of a bare `loot #id`.
    #[must_use]
    pub fn with_loot(mut self, profile: LootProfile) -> Self {
        self.loot = Some(profile);
        self
    }

    /// The loot profile, when there is one.
    #[must_use]
    pub fn loot_profile(&self) -> Option<&LootProfile> {
        self.loot.as_ref()
    }

    /// The planner finished: how it left things. Bags full or a box in hand
    /// is a reason to rest, taken up by the next tick.
    pub fn loot_ended(&mut self, left: Left) {
        self.must_rest = match left {
            Left::Nothing => None,
            Left::BagsFull => Some(Why::Loaded),
            Left::BoxInHand => Some(Why::BoxInHand),
        };
    }

    /// The reason to rest the loot planner left, in words; empty when none.
    #[must_use]
    pub fn rest_reason_text(&self) -> String {
        self.must_rest
            .map(|why| why.to_string())
            .unwrap_or_default()
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
        if let Some(said) = self.errand(state, here) {
            return said;
        }
        if let Some(ending) = self.heard.ending.take() {
            return Said::Done(ending);
        }
        self.note_room(state, now);
        if self.held.is_none() {
            self.held = Self::hold_room(state, self.profile.wander.ignore_disks);
        }
        if state.status.known().dead() != Some(true)
            && let Some(said) = self.react(state, here, now)
        {
            return said;
        }
        if let Some(said) = Self::survival(state, self.recovering()) {
            return said;
        }
        if let Some(said) = self.rest(state, here) {
            return said;
        }
        if let Some(said) = self.flee(state, here, now) {
            return said;
        }
        if let Some(said) = self.loot(state, now) {
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
        self.held = None;
        self.target = None;
        self.queue.clear();
    }

    // --- survival ----------------------------------------------------------

    /// Dead: the hunt is over. Down, and able to move: stand, unless the
    /// kneel was a recovery's.
    fn survival(state: &GameState, recovering: bool) -> Option<Said> {
        let status = state.status.known();
        if status.dead() == Some(true) {
            return Some(Said::Done(Ending::Dead));
        }
        let down = status.standing() == Some(false)
            || status.prone() == Some(true)
            || status.sitting() == Some(true)
            || status.kneeling() == Some(true);
        let held = status.stunned() == Some(true) || status.webbed() == Some(true);
        (down && !held && !recovering).then(|| Said::Send {
            line: "stand".to_owned(),
            target: None,
        })
    }

    // --- loot -------------------------------------------------------------------

    /// `loot #id` on a corpse not yet looted. With `loot.delay` and targets
    /// still here, no oftener than [`LOOT_SPACING`].
    fn loot(&mut self, state: &GameState, now: Option<u32>) -> Option<Said> {
        if self.phase != Phase::Hunting || self.held == Some(Held::Theirs) {
            return None;
        }
        let corpses: Vec<i64> = state
            .creatures()
            .in_room()
            .filter(|creature| creature.corpse())
            .map(|creature| creature.id)
            .collect();
        for id in &corpses {
            if self.dead_seen.insert(*id) {
                self.fried_kills = self.fried_kills.saturating_add(1);
                self.heard.rested_for_injury = false;
            }
        }
        let corpse = corpses
            .iter()
            .copied()
            .find(|id| !self.looted.contains(id))?;
        if self.profile.loot.delay && self.fightable(state).next().is_some() {
            let spaced = self
                .delayed_loot
                .zip(now)
                .is_none_or(|(last, now)| now.saturating_sub(last) >= LOOT_SPACING);
            if !spaced {
                return None;
            }
            self.delayed_loot = now;
        }
        self.looted.insert(corpse);
        if self.loot.is_some() {
            // The planner takes every corpse here in one visit.
            let mut all = vec![corpse];
            for id in corpses {
                if self.looted.insert(id) {
                    all.push(id);
                }
            }
            return Some(Said::Loot(all));
        }
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

    /// Assume Aspect (`650 <aspect> <aspect|evoke>`), as bigshot casts it
    /// (`cmd_assume`, `bigshot.lic:5588-5645`): nothing while an aspect
    /// named is up; the spell first, evoked when the second word is `evoke`
    /// and prepared otherwise; then `assume <aspect>` for each aspect whose
    /// buff is down, once the spell is up. One step a tick, each confirmed
    /// by the effects list before the next, and each with its own retry
    /// window: a spell that fails to land is asked for again in a minute,
    /// not at every prompt.
    fn assume_aspect(&mut self, sign: &str, state: &GameState, now: u32) -> Option<Said> {
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

    // --- engage -----------------------------------------------------------------

    /// Choose a target, target it, take the hunting stance, and run one step
    /// of its routine.
    fn engage(&mut self, state: &GameState, now: Option<u32>) -> Option<Said> {
        if self.phase != Phase::Hunting || !self.may_fight() {
            return None;
        }
        if self.paused(now) {
            return Some(Said::Wait(1));
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

    /// The creatures here the hunt could fight: alive, not known to be
    /// unhostile (a companion), not an animate or a bare appendage, and not
    /// on the never-attack list. What the flee count counts, whether or not
    /// the target list names them.
    pub(super) fn could_fight<'a>(
        &'a self,
        state: &'a GameState,
    ) -> impl Iterator<Item = &'a cena_session::CreatureInstance> + 'a {
        state.creatures().in_room().filter(move |creature| {
            creature.valid_target()
                && creature.hostile() != Some(false)
                && !listed(&self.profile.never_attack, creature)
        })
    }

    /// Those the target list names: the creatures worth attacking.
    pub(super) fn fightable<'a>(
        &'a self,
        state: &'a GameState,
    ) -> impl Iterator<Item = &'a cena_session::CreatureInstance> + 'a {
        self.could_fight(state).filter(move |creature| {
            self.rank(&creature.name, creature.noun.as_deref())
                .is_some()
        })
    }

    /// Whose the room is, as it stands (`claim::claim_room`), and whether a
    /// stranger's disk is in it. `None` while the game has not said who is
    /// here.
    fn hold_room(state: &GameState, ignore_disks: bool) -> Option<Held> {
        let with_me: Vec<String> = state
            .group
            .members()
            .iter()
            .map(|member| member.noun.clone())
            .chain(state.character.name.clone())
            .collect();
        match claim_room(&state.room, &with_me) {
            Claim::Mine => {
                let stranger = state
                    .room
                    .disks()
                    .any(|disk| !with_me.contains(&disk.owner));
                Some(if stranger && !ignore_disks {
                    Held::Disk
                } else {
                    Held::Mine
                })
            }
            Claim::Contested { .. } => Some(Held::Theirs),
            Claim::Unknown => None,
        }
    }

    /// The room is the hunt's to fight in: claimed on entry, and not a
    /// sanctuary.
    pub(super) fn may_fight(&self) -> bool {
        self.held == Some(Held::Mine) && !self.in_sanctuary()
    }

    /// The stance command to send, if the profile names one for this phase
    /// and the bar does not show it yet.
    pub(super) fn stance_for(want: Option<&str>, state: &GameState) -> Option<String> {
        let want = Want::parse(want?).ok()?;
        if stance::landed(want, state) {
            return None;
        }
        stance::command(want, state)
    }
}

/// Whether `wanted` names a creature: its noun, or its whole name, ignoring
/// case (`bigshot.lic:7170`).
fn named(wanted: &str, name: &str, noun: Option<&str>) -> bool {
    name.eq_ignore_ascii_case(wanted) || noun.is_some_and(|noun| noun.eq_ignore_ascii_case(wanted))
}

/// Whether any of `names` names this creature ([`named`]).
pub(super) fn listed(names: &[String], creature: &cena_session::CreatureInstance) -> bool {
    names
        .iter()
        .any(|wanted| named(wanted, &creature.name, creature.noun.as_deref()))
}
