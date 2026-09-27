//! The hunt in a group: the leader's arm and the follower's (`plan/39` §5,
//! Stage 3). Split from `engine.rs`, as `rest.rs` and `wander.rs` were.
//!
//! The engine stays pure. The driver reads the group's board and hands the
//! engine a [`Party`] each tick ([`Hunt::see`]); the engine answers as
//! always, with one [`Said`], and hands back what the board carries
//! ([`Hunt::report`], [`Hunt::leading`]). Solo, nothing here runs.
//!
//! | Role | What changes (`plan/39` §5) | bigshot |
//! |---|---|---|
//! | lead | muster before rest; the members' reasons merged into rest; the looter picked in loot; at the rest, wait for every follower's prep **for this rest** and gather before walking back | `head`, `rest` (`bigshot.lic:7470-7600`) |
//! | follow | catch up to the leader and `join`; the leader's target first; loot only when named; no rest of its own: the leader's, prepping when the order allows, then its own thresholds | `tail` (`:10090-10242`), `group_all_followers` (`:9335-9347`) |
//!
//! Survival, react, maintain and flee stay each member's own, but a
//! follower does not flee or wander: where it goes is where the leader is.

mod alone;
mod keys;
mod recover;

use std::collections::BTreeSet;

use cena_map::RoomId;
use cena_session::{GameState, State};

use super::engine::{Hunt, REST_BEAT};
use super::said::{Ending, Here, Phase, Said, Why};
use crate::group::{
    self, Hindrance, Leading, Muster, Party, PrepOrder, Report, RestCall, Role, Settings,
};

/// Game seconds a grouped follower waits for the game to carry it after
/// the leader, before it walks there itself.
const CATCH_UP: u32 = 3;
/// Game seconds between two `join`s.
const JOIN_AGAIN: u32 = 5;
/// Game seconds the leader waits for the looter to loot a corpse here
/// before it goes on without.
const LOOT_WAIT: u32 = 20;

/// What the group's arms remember between ticks.
#[derive(Debug, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent facts the group's arms carry between ticks"
)]
pub(super) struct Grouping {
    /// This tick's group, as the driver read it; `None` alone.
    party: Option<Party>,
    /// Lead: the number of the rest under way, or of the last one.
    pub(super) rest: u32,
    /// The rest whose own prep this member has finished.
    prepared: Option<u32>,
    /// Lead: who loots.
    looter: Option<String>,
    /// Lead: who preps first at this rest.
    order: Option<PrepOrder>,
    /// Lead: since when a corpse here has waited for the looter.
    loot_wait: Option<u32>,
    /// Lead: since when the gather before the walk back has run, and
    /// whether `group open` was sent for it.
    gather: Option<(u32, bool)>,
    /// Lead: the lost member being dragged home.
    dragging: Option<String>,
    /// Lead: the reasons last shown for not hunting again.
    shown: Vec<String>,
    /// Lead or follow: whether the group keeps this member from starting a
    /// rest of its own this tick.
    pub(super) hold_rest: bool,
    /// Lead: a member is being waited for; no wander.
    pub(super) holding: bool,
    /// Follow: the leader's rest this follower is taking.
    taking: Option<Taking>,
    /// Follow: since when it has been apart from the leader.
    apart: Option<u32>,
    /// Follow: when it last asked to join.
    joined_at: Option<u32>,
    /// Follow: the rest after which its prepare commands were sent.
    readied: Option<u32>,
    /// Follow: said that the leader has no hunt, once.
    told_alone: bool,
    /// Lead: the members last said to be awaited.
    awaited: Vec<String>,
    /// Lead: `group open` sent while awaiting them.
    opened: bool,
    /// Lead: the leg (rest, walking back) the group was disbanded for.
    disbanded: Option<(u32, bool)>,
    /// Follow: the leg it has set out on alone.
    walking: Option<(u32, bool)>,
    /// The room `final_loot` last looted.
    final_looted: Option<String>,
    /// The dead member this member is carrying out (`party/recover.rs`).
    recovering: Option<String>,
    /// Where the carrying out stands.
    recovery: Option<recover::Recovery>,
    /// Lead: a dead member a follower carries out, and who; and since when.
    recover: Option<(String, String)>,
    recover_since: Option<u32>,
    /// Lead: the lost members added back to the group once.
    added: BTreeSet<String>,
}

/// What the leader's absence asks of a follower this tick.
enum Absent {
    /// The leader is with the group.
    Here,
    /// Held for: the hunting arms act, and nothing walks.
    Hold,
    /// This.
    Act(Said),
}

/// A follower's share of the leader's rest.
#[derive(Clone, Copy, Debug)]
struct Taking {
    rest: u32,
    sold: bool,
    healed: bool,
    commands: bool,
}

impl Hunt {
    /// This tick's group, as the driver read the board; `None` alone.
    pub fn see(&mut self, party: impl Into<Option<Party>>) {
        self.grouping.party = party.into();
    }

    /// This member's part this tick; `None` alone.
    pub(super) fn role(&self) -> Option<Role> {
        self.grouping.party.as_ref().map(|party| party.role)
    }

    /// This member's report, for the board. `name` is the character's,
    /// `room` the map's placing of it, `link` its session's lifecycle.
    pub fn report(
        &mut self,
        state: &GameState,
        name: &str,
        room: Option<RoomId>,
        link: State,
    ) -> Report {
        let hunting = matches!(self.phase, Phase::Hunting);
        let unready = if hunting {
            None
        } else if self.grouping.prepared != Some(self.rest_number()) {
            Some("preparing for the rest")
        } else {
            self.still_resting(state).map(super::report::Unrested::word)
        };
        let grouped = match &self.grouping.party {
            Some(party) if party.role == Role::Follow => group::in_group(state, &party.leader),
            _ => true,
        };
        let looted = state
            .creatures()
            .in_room()
            .filter(|creature| creature.corpse() && self.looted.contains(&creature.id))
            .map(|creature| creature.id)
            .collect();
        Report {
            name: name.to_owned(),
            link,
            room,
            rest: if hunting {
                self.rest_reason(state)
            } else {
                None
            },
            unready,
            hindrance: group::hindrance(state),
            grouped,
            health: state.health().map(|vital| vital.percent),
            headroom: self
                .profile
                .rest
                .encumbered
                .zip(state.character.encumbrance_percent)
                .and_then(|(at, now)| i32::try_from(at).ok().zip(i32::try_from(now).ok()))
                .map(|(at, now)| at - now),
            prepared: self.grouping.prepared,
            looted,
            dropped: None,
        }
    }

    /// What the leader publishes: what it is doing now. `room` is the map's
    /// placing of it.
    #[must_use]
    pub fn leading(&self, room: Option<RoomId>) -> Leading {
        let rooms = |ids: &[u32]| ids.iter().copied().map(RoomId).collect();
        let table = &self.profile.group;
        Leading {
            rest: self.grouping.rest,
            prepared: self.grouping.prepared,
            phase: Some(self.phase),
            room,
            target: self.target,
            looter: self.grouping.looter.clone(),
            order: self.grouping.order,
            rooms: group::Rooms {
                hunting: self.profile.rooms.hunting.map(RoomId),
                resting: self.rest_room().map(RoomId),
                rally: rooms(&self.profile.rooms.rally),
                waypoints: rooms(&self.profile.rest.waypoints),
            },
            independent_travel: table.independent_travel,
            independent_return: table.independent_return,
            successors: table.successors.clone(),
            lost_wait: Some(self.group_settings().lost_wait),
            recover: self.grouping.recover.clone(),
        }
    }

    /// The group's settings: the leader's profile's `[group]`.
    pub(super) fn group_settings(&self) -> Settings {
        self.profile.group.settings()
    }

    /// The rest this member is on or last took: the leader's number.
    fn rest_number(&self) -> u32 {
        match &self.grouping.party {
            Some(Party {
                role: Role::Follow,
                leading: Some(leading),
                ..
            }) => leading.rest,
            _ => self.grouping.rest,
        }
    }

    /// The group's arm, before the rest arm: `None` when the group has
    /// nothing to say this tick.
    pub(super) fn group_arm(
        &mut self,
        state: &GameState,
        here: Here<'_>,
        now: Option<u32>,
    ) -> Option<Said> {
        self.grouping.hold_rest = false;
        self.grouping.holding = false;
        // Carrying out a dead member: nothing else until it is done.
        if let Some(dead) = self.grouping.recovering.clone() {
            self.grouping.hold_rest = true;
            return Some(self.carry_out(state, here, &dead, now));
        }
        match self.role()? {
            Role::Lead => self.lead(state, here, now),
            Role::Follow => self.follow(state, here, now),
            Role::Solo => None,
        }
    }

    // --- lead -------------------------------------------------------------------

    fn lead(&mut self, state: &GameState, here: Here<'_>, now: Option<u32>) -> Option<Said> {
        let party = self.grouping.party.clone()?;
        if party.awaiting != self.grouping.awaited {
            if !party.awaiting.is_empty() {
                self.notes.push(format!(
                    "waiting for {} to start hunting.",
                    party.awaiting.join(", ")
                ));
            }
            self.grouping.awaited.clone_from(&party.awaiting);
        }
        if self.phase == Phase::Hunting && !party.awaiting.is_empty() {
            // `head` opens the group for its followers to join
            // (`bigshot.lic:9908`).
            if state.group.status() != Some(cena_session::group::GroupStatus::Open)
                && !std::mem::replace(&mut self.grouping.opened, true)
            {
                return Some(Said::Send {
                    line: "group open".to_owned(),
                    target: None,
                });
            }
            return Some(Said::Wait(1));
        }
        let settings = self.group_settings();
        let me = self.report(state, &party.leader, here.room, State::Ready);
        // The hunt's own seed, not a fresh roll: a random tie names the same
        // looter every tick.
        self.grouping.looter =
            group::looter(&me, &party.followers, &settings, self.seed).map(str::to_owned);
        if matches!(self.phase, Phase::Resting(_)) {
            self.grouping.order = Some(group::prep_order(&me, &party.followers, &settings));
            if self.pending.is_empty() {
                self.grouping.prepared = Some(self.grouping.rest);
            }
        }
        if self.phase != Phase::Hunting {
            return None;
        }
        self.grouping.gather = None;
        if let Some(said) = self.muster(state, here, now, &party) {
            return Some(said);
        }
        // Muster's own reason to rest, taken up by the rest arm now.
        if matches!(self.must_rest, Some(Why::Linkdead | Why::Straggler)) {
            return None;
        }
        // The members' reasons, merged (`should_rest?`, `bigshot.lic:9046`).
        let me = self.report(state, &party.leader, here.room, State::Ready);
        match group::should_rest(&me, &party.followers, party.dropped, &settings) {
            None => self.grouping.hold_rest = true,
            Some(RestCall::Stunned) => return Some(Said::Wait(1)),
            Some(RestCall::Rest { why, loot_first }) => {
                if loot_first && self.corpse_waiting(state, now) {
                    // One more loot first (`:9071-9073`): the loot arm's.
                    self.grouping.hold_rest = true;
                } else if self.grouping.holding {
                    // A member being waited for: no walk to rest (question 5).
                    self.grouping.hold_rest = true;
                } else if let Some(said) = self.disband(state, self.grouping.rest + 1, false) {
                    // Home apart: disbanded before the walk (`:7478-7486`).
                    return Some(said);
                } else {
                    self.must_rest = Some(why);
                }
            }
        }
        None
    }

    /// The followers apart from the leader, by what muster said of each
    /// (`plan/39` §8, question 7's table and §8a).
    fn muster(
        &mut self,
        state: &GameState,
        here: Here<'_>,
        now: Option<u32>,
        party: &Party,
    ) -> Option<Said> {
        let said = |wanted: fn(&Muster) -> bool| {
            party
                .musters
                .iter()
                .find(|(_, muster)| wanted(muster))
                .map(|(name, muster)| (name.clone(), *muster))
        };
        if let Some((name, _)) = said(|m| *m == Muster::Dead) {
            return Some(self.member_dead(state, here, now, party, &name));
        }
        if let Some((name, _)) = said(|m| *m == Muster::Add)
            && self.grouping.added.insert(name.clone())
            && let Some(id) = state
                .room
                .players
                .iter()
                .find(|player| player.noun == name)
                .map(|player| player.id.clone())
        {
            // The handover's own step (`plan/39` §1): the one left behind is
            // added to the group, then taken home (`Group.add`, `group.rb:298`).
            self.notes.push(format!(
                "{name} is link-dead here: adding them to take them home."
            ));
            return Some(Said::Send {
                line: format!("group #{id}"),
                target: None,
            });
        }
        if let Some((name, _)) = said(|m| *m == Muster::Drag) {
            if self.grouping.dragging.as_deref() != Some(name.as_str()) {
                self.notes
                    .push(format!("{name} is link-dead and hurt: dragging them home."));
                self.grouping.dragging = Some(name.clone());
                return Some(Said::Send {
                    line: format!("drag {name}"),
                    target: None,
                });
            }
            self.must_rest = Some(Why::Linkdead);
            return None;
        }
        if let Some((name, _)) = said(|m| *m == Muster::TakeHome) {
            if self.must_rest != Some(Why::Linkdead) {
                self.notes
                    .push(format!("{name} is link-dead here: taking them home."));
            }
            self.must_rest = Some(Why::Linkdead);
            return None;
        }
        if let Some((_, Muster::Fetch(room))) = said(|m| matches!(m, Muster::Fetch(_))) {
            return Some(Said::Walk(room));
        }
        if let Some((name, _)) = said(|m| *m == Muster::Overdue) {
            // Rests as soon as it can move (`group/muster.rs`).
            let stuck = party
                .followers
                .iter()
                .any(|f| f.name == name && f.hindrance == Some(Hindrance::Stuck));
            if !stuck {
                self.must_rest = Some(Why::Straggler);
                return None;
            }
        }
        self.grouping.holding = party.musters.iter().any(|(_, muster)| {
            matches!(
                muster,
                Muster::Hold { .. } | Muster::Await { .. } | Muster::Lost { .. } | Muster::Overdue
            )
        });
        None
    }

    /// A member dead (question 10): every hunt ends, and the leader, if
    /// able, carries it out first; else it names the follower that will,
    /// and waits for it, within `lost_wait`.
    fn member_dead(
        &mut self,
        state: &GameState,
        here: Here<'_>,
        now: Option<u32>,
        party: &Party,
        dead: &str,
    ) -> Said {
        let me = self.report(state, &party.leader, here.room, State::Ready);
        match group::recoverer(&me, &party.followers) {
            Some(recoverer) if recoverer == party.leader => {
                self.grouping.recovering = Some(dead.to_owned());
                self.carry_out(state, here, dead, now)
            }
            Some(recoverer) => {
                if self.grouping.recover.is_none() {
                    self.notes
                        .push(format!("{dead} is dead: {recoverer} carries them out."));
                }
                self.grouping.recover = Some((dead.to_owned(), recoverer.to_owned()));
                let now = now.unwrap_or(0);
                let since = *self.grouping.recover_since.get_or_insert(now);
                let limit =
                    u32::try_from(self.group_settings().lost_wait.as_secs()).unwrap_or(u32::MAX);
                if now.saturating_sub(since) < limit {
                    Said::Wait(1)
                } else {
                    Said::Done(Ending::MemberDied)
                }
            }
            None => {
                self.alerts.push(format!(
                    "{dead} is dead, and no member of the group can carry them out: they need you."
                ));
                Said::Done(Ending::MemberDied)
            }
        }
    }

    /// Whether a corpse here still waits for the looter, within
    /// [`LOOT_WAIT`] of first waiting.
    pub(super) fn corpse_waiting(&mut self, state: &GameState, now: Option<u32>) -> bool {
        let Some(party) = self
            .grouping
            .party
            .as_ref()
            .filter(|p| p.role == Role::Lead)
        else {
            return false;
        };
        let me = party.leader.as_str();
        let looted: Vec<i64> = match self.grouping.looter.as_deref() {
            Some(looter) if looter != me => party
                .followers
                .iter()
                .find(|f| f.name == looter)
                .map(|f| f.looted.clone())
                .unwrap_or_default(),
            _ => self.looted.iter().copied().collect(),
        };
        let waiting = state
            .creatures()
            .in_room()
            .any(|creature| creature.corpse() && !looted.contains(&creature.id));
        if !waiting {
            self.grouping.loot_wait = None;
            return false;
        }
        let now = now.unwrap_or(0);
        let since = *self.grouping.loot_wait.get_or_insert(now);
        now.saturating_sub(since) < LOOT_WAIT
    }

    /// Whether this member loots the corpses here: alone, the one the
    /// leader named, or the leader when it named itself.
    pub(super) fn loots_here(&self) -> bool {
        let Some(party) = &self.grouping.party else {
            return true;
        };
        match party.role {
            Role::Solo => true,
            Role::Lead => self.grouping.looter.as_deref() == Some(party.leader.as_str()),
            Role::Follow => party
                .leading
                .as_ref()
                .is_some_and(|leading| leading.looter.as_deref() == Some(party.name.as_str())),
        }
    }

    /// A follower's leader's target, while the leader has one.
    pub(super) fn leader_target(&self) -> Option<i64> {
        let party = self.grouping.party.as_ref()?;
        if party.role != Role::Follow {
            return None;
        }
        party.leading.as_ref()?.target
    }

    /// At the rest, before the walk back: every follower ready and
    /// prepared for **this** rest (`should_hunt?`, `bigshot.lic:8979-9002`;
    /// the barrier by number, `plan/39` §0e), then everyone here and
    /// grouped (the gather, `:7556-7562`), within `lost_wait`.
    pub(super) fn hold_for_group(
        &mut self,
        state: &GameState,
        here: Here<'_>,
        now: Option<u32>,
    ) -> Option<Said> {
        let party = self.grouping.party.clone()?;
        if party.role != Role::Lead {
            return None;
        }
        let gone = |name: &str| {
            party
                .musters
                .iter()
                .any(|(n, m)| n == name && matches!(m, Muster::Gone | Muster::Left))
        };
        let counted: Vec<&Report> = party
            .followers
            .iter()
            .filter(|follower| !gone(&follower.name))
            .collect();
        let rest = self.grouping.rest;
        let mut waiting: Vec<String> = counted
            .iter()
            .filter_map(|follower| {
                let why = if follower.link != State::Ready {
                    "its connection is lost"
                } else if follower.prepared != Some(rest) {
                    "preparing for the rest"
                } else {
                    follower.unready?
                };
                Some(format!("{} isn't hunting because: {why}", follower.name))
            })
            .collect();
        waiting.sort();
        if !waiting.is_empty() {
            if waiting != self.grouping.shown {
                self.notes.extend(waiting.iter().cloned());
                self.grouping.shown = waiting;
            }
            return Some(Said::Wait(REST_BEAT));
        }
        self.grouping.shown.clear();
        if let Some(said) = self.gather(state, here, now, &counted) {
            return Some(said);
        }
        // Out apart: disbanded once gathered (`:7246-7252`).
        self.disband(state, self.grouping.rest, true)
    }

    /// Before it hunts, the leader gathers its group: everyone who walked
    /// out on its own, or fell behind (`pre_hunt`, `bigshot.lic:7266-7275`).
    pub(super) fn gather_to_hunt(&mut self, state: &GameState, here: Here<'_>) -> Option<Said> {
        let party = self.grouping.party.clone()?;
        if party.role != Role::Lead {
            return None;
        }
        let counted: Vec<&Report> = party
            .followers
            .iter()
            .filter(|follower| {
                follower.link == State::Ready
                    && !party.musters.iter().any(|(n, m)| {
                        *n == follower.name && matches!(m, Muster::Gone | Muster::Left)
                    })
            })
            .collect();
        self.gather(state, here, state.game_time_now(), &counted)
    }

    /// The gather: every counted follower here and in the group, `group
    /// open` sent once for it, within `lost_wait` (`:7556-7562`).
    fn gather(
        &mut self,
        state: &GameState,
        here: Here<'_>,
        now: Option<u32>,
        counted: &[&Report],
    ) -> Option<Said> {
        let now = now.unwrap_or(0);
        let (since, opened) = *self.grouping.gather.get_or_insert((now, false));
        let limit = u32::try_from(self.group_settings().lost_wait.as_secs()).unwrap_or(u32::MAX);
        let apart = counted
            .iter()
            .any(|f| f.room.is_none() || f.room != here.room || !f.grouped);
        if !apart || now.saturating_sub(since) >= limit {
            self.grouping.gather = None;
            return None;
        }
        if !opened {
            self.grouping.gather = Some((since, true));
            if state.group.status() != Some(cena_session::group::GroupStatus::Open) {
                return Some(Said::Send {
                    line: "group open".to_owned(),
                    target: None,
                });
            }
        }
        Some(Said::Wait(1))
    }

    // --- follow -----------------------------------------------------------------

    fn follow(&mut self, state: &GameState, here: Here<'_>, now: Option<u32>) -> Option<Said> {
        let party = self.grouping.party.clone()?;
        self.grouping.hold_rest = true;
        match self.leader_gone(state, here, now, &party) {
            Absent::Here => {}
            Absent::Hold => return None,
            Absent::Act(said) => return Some(said),
        }
        let Some(leading) = party.leading.clone().filter(|l| l.phase.is_some()) else {
            if !std::mem::replace(&mut self.grouping.told_alone, true) {
                self.notes.push(format!(
                    "following {}, whose hunt has not started: waiting for it.",
                    party.leader
                ));
            }
            return Some(Said::Wait(1));
        };
        self.grouping.told_alone = false;
        let now_s = now.unwrap_or(0);
        let grouped = group::in_group(state, &party.leader);
        // Walking apart (`party/alone.rs`): no catching up on the way.
        if Self::apart(&leading) {
            self.phase = match leading.phase {
                Some(Phase::Returning) => Phase::Returning,
                Some(Phase::ToRest(why) | Phase::Selling(why) | Phase::Healing(why)) => {
                    Phase::ToRest(why)
                }
                _ => self.phase,
            };
            if let Some(said) = self.walk_apart(here, &leading) {
                return Some(said);
            }
            // At the far end: it waits for the leader there, and joins.
            if grouped || here.room.is_none() || here.room != leading.room {
                return Some(Said::Wait(1));
            }
        }
        if let (Some(mine), Some(theirs)) = (here.room, leading.room)
            && mine != theirs
        {
            let since = *self.grouping.apart.get_or_insert(now_s);
            if !grouped || now_s.saturating_sub(since) >= CATCH_UP {
                return Some(Said::Walk(theirs));
            }
            return Some(Said::Wait(1));
        }
        self.grouping.apart = None;
        let phase = leading.phase?;
        if !grouped && here.room.is_some() && here.room == leading.room {
            let due = self
                .grouping
                .joined_at
                .is_none_or(|at| now_s.saturating_sub(at) >= JOIN_AGAIN);
            if due {
                self.grouping.joined_at = Some(now_s);
                return Some(Said::Send {
                    line: format!("join {}", party.leader),
                    target: None,
                });
            }
        }
        match phase {
            Phase::Hunting => {
                if self.phase != Phase::Hunting {
                    self.phase = Phase::Hunting;
                    self.notes.push("hunting.".to_owned());
                }
                self.pending
                    .pop_front()
                    .map(|line| Said::Send { line, target: None })
            }
            Phase::ToRest(why) | Phase::Selling(why) | Phase::Healing(why) => {
                if self.phase == Phase::Hunting {
                    self.notes
                        .push(format!("{}: resting ({why}).", party.leader));
                }
                self.phase = Phase::ToRest(why);
                Some(Said::Wait(1))
            }
            Phase::Resting(why) => Some(self.take_rest(state, why, &leading)),
            Phase::Returning | Phase::Preparing => {
                if self.grouping.readied != Some(leading.rest) {
                    self.grouping.readied = Some(leading.rest);
                    self.fried_kills = 0;
                    self.pending = self.profile.prepare.iter().cloned().collect();
                    self.phase = Phase::Returning;
                }
                Some(
                    self.pending
                        .pop_front()
                        .map_or(Said::Wait(1), |line| Said::Send { line, target: None }),
                )
            }
        }
    }

    /// The leader lost (Stage 6): held for, fighting what comes and nothing
    /// else (question 5); dead, carried out (question 10); handed over,
    /// placed anew by the driver on its next turn. And a dead member the
    /// leader named this one to carry out, or not.
    fn leader_gone(
        &mut self,
        state: &GameState,
        here: Here<'_>,
        now: Option<u32>,
        party: &Party,
    ) -> Absent {
        match party.leader_lost {
            Some(Muster::Lost { .. }) => return Absent::Hold,
            Some(Muster::Dead) => {
                if party.recoverer.as_deref() != Some(party.name.as_str()) {
                    return Absent::Act(Said::Done(Ending::MemberDied));
                }
                self.grouping.recovering = Some(party.leader.clone());
                return Absent::Act(self.carry_out(state, here, &party.leader, now));
            }
            Some(_) => return Absent::Act(Said::Wait(1)),
            None => {}
        }
        let Some((dead, recoverer)) = party.leading.as_ref().and_then(|l| l.recover.clone()) else {
            return Absent::Here;
        };
        if recoverer != party.name {
            return Absent::Act(Said::Done(Ending::MemberDied));
        }
        self.grouping.recovering = Some(dead.clone());
        Absent::Act(self.carry_out(state, here, &dead, now))
    }

    /// The follower's share of the leader's rest: its prep when the order
    /// allows (`quiet_followers`), its selling, herbs and resting
    /// commands, then its own thresholds.
    fn take_rest(&mut self, state: &GameState, why: Why, leading: &Leading) -> Said {
        let rest = leading.rest;
        if self
            .grouping
            .taking
            .is_none_or(|taking| taking.rest != rest)
        {
            self.grouping.taking = Some(Taking {
                rest,
                sold: false,
                healed: false,
                commands: false,
            });
            self.phase = Phase::Resting(why);
            // `RESTING_PREP_COMMANDS` zeroes the overkill count
            // (`bigshot.lic:10205-10208`).
            self.fried_kills = 0;
        }
        let order = leading.order.unwrap_or(PrepOrder::Together);
        if !order.follower_may_prep(leading) {
            return Said::Wait(1);
        }
        let Some(mut taking) = self.grouping.taking else {
            return Said::Wait(1);
        };
        // Its selling, its herbs, its resting commands: each step that has
        // nothing to do gives way to the next in the same tick.
        let said = if !std::mem::replace(&mut taking.sold, true) && self.wants_to_sell(state) {
            Some(Said::Sell)
        } else if !std::mem::replace(&mut taking.healed, true) && self.wants_to_heal(state) {
            Some(Said::Heal)
        } else {
            if !std::mem::replace(&mut taking.commands, true) {
                self.pending = self.profile.rest.commands.iter().cloned().collect();
            }
            self.pending
                .pop_front()
                .map(|line| Said::Send { line, target: None })
        };
        self.grouping.taking = Some(taking);
        if let Some(said) = said {
            return said;
        }
        self.grouping.prepared = Some(rest);
        Said::Wait(1)
    }
}
