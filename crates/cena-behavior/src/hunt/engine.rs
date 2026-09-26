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

use super::aim::{Aimed, Aiming};
use super::ammo::Ammo;
use super::bounty::BountyMode;
use super::death::Mourning;
use super::follow::{Follow, Resend};
use super::guard::{Facts, Used};
use super::monitor::Watch;
use super::profile::{Profile, Step};
use super::react::Reacting;
use super::repeat::Repeats;
use super::replies::Heard;
pub use super::said::{Ending, Here, Phase, Said, Why};
use super::verbs::Go;
use super::wand::Wanding;
use crate::heal::HealProfile;
use crate::keep::KeepProfile;
use crate::loot::{Left, LootProfile};
use crate::stance::{self, Want};
use crate::waggle::WaggleProfile;

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
    /// The room was entered since flee last looked (`flee.lone_only`).
    pub(super) entered: bool,
    /// Where the aim lists stand for this target ([`super::aim`]).
    pub(super) aiming: Aiming,
    /// Where the wand list stands ([`super::wand`]).
    pub(super) wanding: Wanding,
    /// Return waypoints still to walk through on the way to rest.
    pub(super) waypoints: VecDeque<RoomId>,
    /// This rest has fogged once from the rift already.
    pub(super) fogged: bool,
    /// When a society mana ability was last used ([`super::wrack`]).
    pub(super) wracked: Option<u32>,
    /// Boon creatures assessed, by id, with their traits ([`super::boons`]).
    pub(super) boons: BTreeMap<i64, Vec<&'static str>>,
    /// The creature an `assess` was sent for.
    pub(super) assessing: Option<i64>,
    /// Long-Term Experience Boosts spent this hunt, and whether one is
    /// waiting on its reply.
    pub(super) boosts: (u32, bool),
    /// The steps sent in this room, for `once` and `every` ([`Used`]).
    pub(super) used: Used,
    /// When the censer was last cast ([`super::censer`]).
    pub(super) censer_cast: Option<u32>,
    /// Barkskin cannot be cast before this game second ([`super::maintain`]).
    pub(super) bark_until: Option<u32>,
    /// Lines a step's first line must be followed by: a spell's `cast`
    /// after its `prepare` ([`super::verbs`]).
    pub(super) followups: VecDeque<String>,
    /// bigshot verbs the player was told Hydra does not send yet.
    pub(super) told_unported: BTreeSet<&'static str>,
    /// An arrow the game would not fire, being put away ([`super::ammo`]).
    pub(super) ammo: Ammo,
    /// Recovering from a death ([`super::death`]).
    pub(super) mourning: Mourning,
    /// The character's waggle profile, for a waggle after departing.
    pub(super) waggle_profile: Option<WaggleProfile>,
    /// The interaction monitor's patterns ([`super::monitor`]).
    pub(super) watch: Watch,
    /// Lines the monitor wants put in front of the player.
    pub(super) alerts: Vec<String>,
    /// A quick hunt: this room, until it is clear ([`super::quick`]).
    pub(super) quick: bool,
    /// `eachtarget` and `force`, and `resonance`'s last spell.
    pub(super) repeats: Repeats,
    /// A step's hold or awaited answer, and the character rooted.
    pub(super) follow: Follow,
    /// `;hunt <name> bounty`.
    pub(super) bounty_mode: BountyMode,
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
            entered: false,
            aiming: Aiming::default(),
            wanding: Wanding::default(),
            waypoints: VecDeque::new(),
            fogged: false,
            wracked: None,
            boosts: (0, false),
            boons: BTreeMap::new(),
            assessing: None,
            used: Used::new(),
            censer_cast: None,
            bark_until: None,
            followups: VecDeque::new(),
            told_unported: BTreeSet::new(),
            ammo: Ammo::default(),
            mourning: Mourning::default(),
            waggle_profile: None,
            watch: Watch::default(),
            alerts: Vec::new(),
            quick: false,
            repeats: Repeats::default(),
            follow: Follow::default(),
            bounty_mode: BountyMode::default(),
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

    /// The connection dropped: the target, the room's claim and the room
    /// itself are asked again once the session is back.
    pub fn link_lost(&mut self) {
        self.target_gone();
        self.repeats_gone();
        self.follow_gone();
        self.held = None;
        self.room = None;
    }

    /// The gate refused the target: it is gone.
    pub fn target_gone(&mut self) {
        self.target = None;
        self.queue.clear();
        self.followups.clear();
        self.aiming.reset();
    }

    /// One turn: what to do now, against `state` as it is, standing in
    /// `here`, at game second `now`.
    pub fn tick(&mut self, state: &GameState, here: Here<'_>, now: Option<u32>) -> Said {
        if let Some(said) = self.nudging() {
            return said;
        }
        if let Some(said) = self.errand(state, here) {
            return said;
        }
        if let Some(ending) = self.heard.ending.take() {
            return Said::Done(ending);
        }
        if let Some(said) = self.death(state, now) {
            return said;
        }
        self.note_room(state, now);
        if self.held.is_none() {
            self.held = if self.quick {
                Some(Held::Mine)
            } else {
                Self::hold_room(state, self.profile.wander.ignore_disks)
            };
        }
        if state.status.known().dead() != Some(true)
            && let Some(said) = self.react(state, here, now)
        {
            return said;
        }
        if let Some(said) = self.survival(state) {
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
        if let Some(said) = self.engage(state, here, now) {
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
        self.entered = true;
        self.target = None;
        self.queue.clear();
        self.followups.clear();
        self.used.clear();
    }

    // --- survival ----------------------------------------------------------

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
        // A kill that left no corpse (vaporized, faded) counts as one.
        let gone = state
            .creatures()
            .in_room()
            .filter(|creature| creature.ending().is_some_and(cena_session::Ending::killed))
            .map(|creature| creature.id);
        for id in corpses.iter().copied().chain(gone) {
            if self.dead_seen.insert(id) {
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

    // --- engage -----------------------------------------------------------------

    /// Choose a target, target it, take the hunting stance, and run one step
    /// of its routine.
    fn engage(&mut self, state: &GameState, here: Here<'_>, now: Option<u32>) -> Option<Said> {
        if self.phase != Phase::Hunting || !self.may_fight() {
            return None;
        }
        if self.paused(now) {
            return Some(Said::Wait(1));
        }
        if let Some(said) = self.assess_boons(state) {
            return Some(said);
        }
        let target = self.choose_target(state)?;
        if let Some(said) = self.repeating(state, here, now) {
            return Some(said);
        }
        if state.targeting.current() != Some(target) {
            return Some(Said::Send {
                line: format!("target #{target}"),
                target: Some(target),
            });
        }
        self.next_step(state, here, target, now)
    }

    /// The step to send now: the sequence in play first, then the routine
    /// from its cursor, skipping held steps, expanding sequences, and
    /// skipping steps whose guards do not hold.
    fn next_step(
        &mut self,
        state: &GameState,
        here: Here<'_>,
        target: i64,
        now: Option<u32>,
    ) -> Option<Said> {
        if let Some(line) = self.ammo_line(state) {
            return Some(Said::Send { line, target: None });
        }
        if let Some(line) = self.followups.pop_front() {
            self.follow.resend = Some(Resend::Line(line.clone()));
            return Some(Said::Send {
                line,
                target: Some(target),
            });
        }
        if let Some(said) = self.holding(state, now) {
            return Some(said);
        }
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
            let key = step.to_string();
            let facts = Facts {
                state,
                target: Some(target),
                tags: here.room.map(|_| here.tags),
                used: Some(&self.used),
                step: &key,
            };
            if !step.when.iter().all(|c| c.holds(&facts) == Some(true)) {
                continue;
            }
            if let Some(said) = self.waits_behind(&step.send, state, target, now) {
                self.queue.push_front(step);
                return Some(said);
            }
            if self.repeat(&step, state, target, now) {
                match self.repeating(state, here, now) {
                    Some(said) => return Some(said),
                    None => continue,
                }
            }
            match self.wand_step(&step, &key, state, target, now) {
                Some(Go::Said(said)) => return Some(said),
                Some(_) => continue,
                None => {}
            }
            let hidden = state.status.known().hidden() == Some(true);
            let line = match self.aim(&step.send, target, hidden) {
                None => match self.verb_step(&step.send, &key, target, state, now) {
                    Go::Send(line) => line,
                    Go::Said(said) => return Some(said),
                    Go::Skip => continue,
                },
                Some(Aimed::Instead(line)) => line,
                Some(Aimed::First(line)) => {
                    self.queue.push_front(step);
                    return Some(Said::Send {
                        line,
                        target: Some(target),
                    });
                }
            };
            self.used.record(&key, Some(target), now);
            self.follow.resend = Some(Resend::Step(step));
            return Some(Said::Send {
                line,
                target: Some(target),
            });
        }
        Some(Said::Wait(1))
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
