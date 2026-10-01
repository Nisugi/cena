//! The driver's side of a group (`plan/39` §5, Stage 3): which board this
//! member is on, what it reads there each turn for the engine, and what it
//! puts up after.
//!
//! The engine's side is `hunt/party.rs`. What needs the process's clock is
//! here: when each follower was first seen apart (the muster's deadlines),
//! and when each member's connection last dropped (question 9's *everyone
//! dropped*). **Nothing crosses sessions but the board's data**: the leader
//! never sends on a follower's session (`plan/39` §3).

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cena_map::RoomId;
use cena_session::group::Leader;
use cena_session::{CommandId, Frame, Gate, Origin, State};

use super::{Driver, HuntEnd};
use crate::error::BehaviorError;
use crate::group::{
    self, Board, Boards, Hindrance, LOST_WAIT, Leading, Muster, Party, Place, Report, Role,
    Settings, muster,
};
use crate::hunt::said::{Ending, Phase};
use crate::travel::TravelNotes;

/// How close together every member's drop must be to count as one network
/// taking them all down (`plan/39` §3, question 9).
const ONE_DROP: Duration = Duration::from_mins(2);

/// This member's place in a group, kept by the driver.
pub(super) struct Membership {
    boards: Arc<Boards>,
    /// The board this member is on, by whose it is.
    on: Option<(String, Arc<Board>)>,
    /// Lead: when each follower was first seen apart.
    since: BTreeMap<String, Instant>,
    /// Lead: when each member not yet reporting was first waited for.
    awaited: BTreeMap<String, Instant>,
    /// Lead: when the last rest began, so older drops do not count.
    rested: Option<Instant>,
    /// When this member's own connection last dropped.
    dropped: Option<Instant>,
    /// Asked the game `group` since the connection came up.
    asked: bool,
    /// Lead: the characters the command named, awaited until they report.
    expected: Vec<String>,
    /// Follow: the leader the command named, until the game's group says.
    following: Option<String>,
    /// Follow: since when the leader has been lost (`plan/39` Stage 6).
    leader_since: Option<Instant>,
    /// Lead: a leader this member took over from, and its board, so its
    /// report is read with the followers' (the handover's *"add the one
    /// left behind"*).
    lost: Option<(String, Arc<Board>)>,
}

impl Membership {
    pub(super) fn new(boards: Arc<Boards>, place: Place) -> Self {
        let (expected, following) = match place {
            Place::Read => (Vec::new(), None),
            Place::Lead(names) => (names, None),
            Place::Follow(leader) => (Vec::new(), Some(leader)),
        };
        Self {
            boards,
            on: None,
            since: BTreeMap::new(),
            awaited: BTreeMap::new(),
            rested: None,
            dropped: None,
            asked: false,
            expected,
            following,
            leader_since: None,
            lost: None,
        }
    }
}

/// What a turn does after reading the group.
pub(super) enum Seen {
    /// Tick the engine.
    Go,
    /// Send this line, then look again: `group`, when nobody has said who
    /// leads; `leave group`, to leave a lost leader's group.
    Say(&'static str),
    /// The leader's hunt is over, and so is this follower's (`plan/39` §8,
    /// question 3).
    Over(HuntEnd),
}

impl<F: FnMut() -> CommandId, W: FnMut(&TravelNotes), L: FnMut(&[String])> Driver<'_, F, W, L> {
    /// This character's game, which keys its group's board with the
    /// leader's name: a group is in one game, and one name can be on two
    /// (BE-B-4). Empty until the login burst names it.
    fn game(&self) -> String {
        self.state.character.instance.clone().unwrap_or_default()
    }

    /// Read the group for this turn and hand the engine its [`Party`].
    pub(super) fn see_party(&mut self, here: Option<RoomId>) -> Seen {
        let Some(name) = self.state.character.name.clone() else {
            self.machine.see(None);
            return Seen::Go;
        };
        if self.membership.is_none() {
            return Seen::Go;
        }
        let game = self.game();
        self.back_after_handover(&game, &name);
        let Some(member) = self.membership.as_mut() else {
            return Seen::Go;
        };
        let (role, leader) = match (group::role(&self.state.group), self.state.group.leader()) {
            // The game's group, once it says, over what the command said.
            (Some(Role::Lead), _) => {
                member.following = None;
                (Role::Lead, name.clone())
            }
            (Some(Role::Follow), Leader::Other(leader)) => {
                member.following = None;
                (Role::Follow, leader.noun.clone())
            }
            (role, _) => match (&member.following, &member.on) {
                // Named a follower by the command: to be caught up to and
                // joined, from outside the group.
                (Some(leader), _) => (Role::Follow, leader.clone()),
                // Walked off, or its roster not yet re-read: still that
                // leader's follower while its hunt goes on (bigshot's
                // `group_all_followers`, `bigshot.lic:9335-9347`).
                (None, Some((leader, _))) if *leader != name => (Role::Follow, leader.clone()),
                // Leading, with members on the board or on their way.
                (None, Some((leader, board))) if *leader == name && board.reports().len() > 1 => {
                    (Role::Lead, name.clone())
                }
                _ if !member.expected.is_empty() => (Role::Lead, name.clone()),
                _ if role.is_none() && !member.asked => {
                    member.asked = true;
                    return Seen::Say("group");
                }
                _ => {
                    member.on = None;
                    member.boards.withdraw_except(&game, &name, None);
                    self.machine.see(None);
                    return Seen::Go;
                }
            },
        };
        let board = match role {
            Role::Lead => Some(member.boards.lead(&game, &name)),
            _ => member.boards.of(&game, &leader),
        };
        let Some(board) = board else {
            // Following a leader whose hunt has ended: so has this one.
            if member.on.as_ref().is_some_and(|(on, _)| *on == leader) {
                member.on = None;
                return Seen::Over(HuntEnd::Finished(Ending::LeaderStopped));
            }
            self.machine.see(Some(Party {
                name,
                role,
                leader,
                leading: None,
                followers: Vec::new(),
                musters: Vec::new(),
                dropped: false,
                awaiting: Vec::new(),
                leader_lost: None,
                recoverer: None,
            }));
            return Seen::Go;
        };
        if member.on.as_ref().is_none_or(|(on, _)| *on != leader) {
            member.boards.withdraw_except(&game, &name, Some(&leader));
            member.on = Some((leader.clone(), Arc::clone(&board)));
        }
        let reports = board.reports();
        let party = if role == Role::Lead {
            self.leaders_view(&name, here, &reports)
        } else {
            let leading = board.leading();
            let (leader_lost, recoverer) = self.leader_view(&leader, &reports, &leading, here);
            if matches!(
                leader_lost,
                Some(Muster::Gone | Muster::TakeHome | Muster::Drag | Muster::Add)
            ) {
                return self.hand_over(&game, &name, &leader, &board, &reports, &leading);
            }
            Party {
                name,
                role,
                leader,
                leading: Some(leading),
                followers: Vec::new(),
                musters: Vec::new(),
                dropped: false,
                awaiting: Vec::new(),
                leader_lost,
                recoverer,
            }
        };
        self.machine.see(Some(party));
        Seen::Go
    }

    /// Back after the lead passed while it was away (question 8): it follows
    /// the new leader.
    fn back_after_handover(&mut self, game: &str, name: &str) {
        let Some(member) = self.membership.as_mut() else {
            return;
        };
        if let Some(new) = member.boards.take_handed(game, name) {
            member.following = Some(new.clone());
            member.on = None;
            member.expected.clear();
            self.handle.say(cena_session::Notice::line(
                cena_session::NoticeKind::Info,
                format!("Hunt: {new} leads now; following."),
            ));
        }
    }

    /// A follower's view of its leader, when the leader is not with the
    /// group: what muster says of it, with the leader's `lost_wait`, and
    /// who carries it out if it is dead (`plan/39` Stage 6, question 10).
    fn leader_view(
        &mut self,
        leader: &str,
        reports: &BTreeMap<String, Report>,
        leading: &Leading,
        here: Option<RoomId>,
    ) -> (Option<Muster>, Option<String>) {
        let Some(member) = self.membership.as_mut() else {
            return (None, None);
        };
        let Some(report) = reports.get(leader) else {
            return (None, None);
        };
        let dead = report.hindrance == Some(Hindrance::Dead);
        if report.link == State::Ready && !dead {
            member.leader_since = None;
            return (None, None);
        }
        let now = tokio::time::Instant::now().into_std();
        let since = *member.leader_since.get_or_insert(now);
        let settings = Settings {
            lost_wait: leading.lost_wait.unwrap_or(LOST_WAIT),
            successors: leading.successors.clone(),
            ..Settings::default()
        };
        let standing = (report.link != State::Ready)
            .then(|| group::standing(&self.state, leader))
            .flatten();
        let said = muster(report, here, standing, since, now, &settings);
        let others: Vec<Report> = reports
            .values()
            .filter(|r| r.name != leader)
            .cloned()
            .collect();
        let recoverer = (said == Some(Muster::Dead))
            .then(|| group::recoverer(report, &others).map(str::to_owned))
            .flatten();
        (said, recoverer)
    }

    /// The leader is lost for good: the successor leads, the others follow
    /// it; anyone still in the lost leader's game group leaves it first
    /// (`plan/39` §1, the author's handover; question 4's successor).
    fn hand_over(
        &mut self,
        game: &str,
        name: &str,
        old: &str,
        board: &Arc<Board>,
        reports: &BTreeMap<String, Report>,
        leading: &Leading,
    ) -> Seen {
        let in_old = matches!(self.state.group.leader(), Leader::Other(l) if l.noun == old);
        let Some(member) = self.membership.as_mut() else {
            return Seen::Go;
        };
        let settings = Settings {
            successors: leading.successors.clone(),
            ..Settings::default()
        };
        let members: Vec<Report> = reports
            .values()
            .filter(|r| r.name != old)
            .cloned()
            .collect();
        // One answer for the group, not one a follower (`Boards::succeed`).
        let successor = member.boards.succeed(
            game,
            old,
            |new| members.iter().any(|r| r.name == new && r.present()),
            || group::successor(&members, &settings, 0).map(str::to_owned),
        );
        member.leader_since = None;
        member.on = None;
        let text = match &successor {
            Some(new) if new == name => {
                member.lost = Some((old.to_owned(), Arc::clone(board)));
                member.following = None;
                member.expected = members
                    .iter()
                    .map(|r| r.name.clone())
                    .filter(|n| n != name)
                    .collect();
                format!("Hunt: {old} is lost; I lead now.")
            }
            Some(new) => {
                member.following = Some(new.clone());
                format!("Hunt: {old} is lost; {new} leads now.")
            }
            None => {
                member.following = None;
                format!("Hunt: {old} is lost, and nobody is left to lead; hunting alone.")
            }
        };
        self.handle.say(cena_session::Notice::line(
            cena_session::NoticeKind::Info,
            text,
        ));
        self.machine.see(None);
        if in_old {
            return Seen::Say("leave group");
        }
        Seen::Go
    }

    /// The leader's view: its followers, what muster says of each apart
    /// from it, and whether every member dropped since the last rest.
    fn leaders_view(
        &mut self,
        name: &str,
        here: Option<RoomId>,
        reports: &BTreeMap<String, Report>,
    ) -> Party {
        let settings = self.machine.profile().group.settings();
        let now = tokio::time::Instant::now().into_std();
        let mut followers: Vec<Report> = reports
            .values()
            .filter(|report| report.name != name)
            .cloned()
            .collect();
        // A leader taken over from, while it is not back with us: read with
        // the followers, so muster takes it home or hands it on (§8a).
        if let Some(member) = self.membership.as_mut()
            && let Some((old, board)) = member.lost.clone()
        {
            match board.reports().get(&old) {
                Some(report) if !followers.iter().any(|f| f.name == old) => {
                    followers.push(report.clone());
                }
                _ => member.lost = None,
            }
        }
        let resting = self.machine.phase() != Phase::Hunting;
        let Some(member) = self.membership.as_mut() else {
            return Party {
                name: name.to_owned(),
                role: Role::Lead,
                leader: name.to_owned(),
                leading: None,
                followers,
                musters: Vec::new(),
                dropped: false,
                awaiting: Vec::new(),
                leader_lost: None,
                recoverer: None,
            };
        };
        if resting {
            member.rested = Some(now);
        }
        let mut musters = Vec::new();
        for follower in &followers {
            let since = member.since.get(&follower.name).copied().unwrap_or(now);
            let standing = (follower.link != State::Ready)
                .then(|| group::standing(&self.state, &follower.name))
                .flatten();
            match muster(follower, here, standing, since, now, &settings) {
                Some(said) => {
                    member.since.insert(follower.name.clone(), since);
                    musters.push((follower.name.clone(), said));
                }
                None => {
                    member.since.remove(&follower.name);
                }
            }
        }
        // The game's group and the command's names, not yet on the board:
        // waited for, then not.
        let named: Vec<String> = self
            .state
            .group
            .members()
            .iter()
            .map(|m| m.noun.clone())
            .chain(member.expected.iter().cloned())
            .collect();
        let mut awaiting = Vec::new();
        for noun in &named {
            if followers.iter().any(|f| f.name == *noun) || awaiting.contains(noun) {
                continue;
            }
            let since = *member.awaited.entry(noun.clone()).or_insert(now);
            if now.duration_since(since) < settings.lost_wait {
                awaiting.push(noun.clone());
            }
        }
        member.awaited.retain(|n, _| named.contains(n));
        member
            .since
            .retain(|n, _| followers.iter().any(|f| f.name == *n));
        // Question 9: every member's drop close together, after the last
        // rest began.
        let drops: Vec<Instant> = std::iter::once(member.dropped)
            .chain(followers.iter().map(|f| f.dropped))
            .collect::<Option<Vec<Instant>>>()
            .unwrap_or_default();
        let dropped = !resting
            && !followers.is_empty()
            && drops.len() == followers.len() + 1
            && drops
                .iter()
                .all(|at| member.rested.is_none_or(|rested| *at > rested))
            && drops
                .iter()
                .max()
                .zip(drops.iter().min())
                .is_some_and(|(latest, earliest)| latest.duration_since(*earliest) <= ONE_DROP);
        Party {
            name: name.to_owned(),
            role: Role::Lead,
            leader: name.to_owned(),
            leading: None,
            followers,
            musters,
            dropped,
            awaiting,
            leader_lost: None,
            recoverer: None,
        }
    }

    /// Put up this member's report, and the leader's state when it leads.
    pub(super) fn publish(&mut self, here: Option<RoomId>, link: State) {
        let Some(name) = self.state.character.name.clone() else {
            return;
        };
        let Some(member) = self.membership.as_ref() else {
            return;
        };
        let Some((leader, board)) = member.on.clone() else {
            return;
        };
        let dropped = member.dropped;
        let mut report = self.machine.report(&self.state, &name, here, link);
        report.dropped = dropped;
        board.publish(report);
        if leader == name {
            board.lead(self.machine.leading(here));
        }
    }

    /// The connection dropped: said on the board at once, since a member
    /// that is down does not tick.
    pub(super) fn party_link_lost(&mut self) {
        if let Some(member) = self.membership.as_mut() {
            member.dropped = Some(tokio::time::Instant::now().into_std());
            member.asked = false;
        }
        let here = self.last_room;
        self.publish(here, State::Reconnecting);
    }

    /// The hunt is over: off the board, or marked given up when Hydra gave
    /// the session up, for the leader's muster (`plan/39` §8a). A leader
    /// whose hunt ended for another reason takes its board down, which ends
    /// its followers' hunts (question 3).
    pub(super) async fn leave_party(&mut self, end: HuntEnd) {
        let Some(name) = self.state.character.name.clone() else {
            return;
        };
        let closed = matches!(end, HuntEnd::Stopped(BehaviorError::Dead));
        if closed {
            let here = self.last_room;
            self.publish(here, State::Closed);
            return;
        }
        // Dead: its report stays up, dead, for the group to carry it out
        // (question 10).
        if end == HuntEnd::Finished(Ending::Dead) {
            let here = self.last_room;
            self.publish(here, State::Ready);
            return;
        }
        let Some(member) = self.membership.as_ref() else {
            return;
        };
        let game = self.game();
        member.boards.withdraw_except(&game, &name, None);
        let led = member.on.as_ref().map(|(leader, _)| *leader == name);
        if led == Some(true) {
            member.boards.close(&game, &name);
        }
        // Question 3: a follower's own stop leaves the game's group, and the
        // rest hunt on without it (bigshot's `LEAVE_GROUP`, `:10108`).
        if led == Some(false) && end == HuntEnd::Stopped(BehaviorError::Cancelled) {
            let id = (self.next_id)();
            let _ = self
                .handle
                .send_gated(
                    id,
                    "leave group",
                    Origin::Behavior(self.token),
                    Duration::from_secs(5),
                    |frame| matches!(frame, Frame::Prompt { .. }),
                    Gate::None,
                )
                .await;
        }
    }
}
