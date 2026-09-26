//! Muster: what the group does about a member who is not with the leader,
//! and until when (`plan/39` §8, question 7's table, and question 5's
//! `lost_wait`).
//!
//! *Muster* is gathering the group, `plan/39` §0e's word for it, and
//! eohunter's name for the leader's holds between fights (`Muster`,
//! `scripts/eohunter/group.rb:1143-1228`). bigshot has no such rule: a
//! follower whose remote call raises is dropped, and the leader hunts on
//! (`member_online`, `bigshot.lic:954-966`); eohunter drops one silent for
//! ten seconds (`REPORT_STALE`, `group.rb:214`). **The author's answer is
//! not to** (question 7): the group learns why the member is apart, from
//! its own report, and acts on the reason.
//!
//! # The order the reasons are read in
//!
//! 1. **Its connection.** A member whose session is not `Ready` has a
//!    report its own state no longer backs (`plan/12` §5.2), so nothing
//!    else in it is read.
//! 2. **Dead**, before it left the group: death takes a character out of
//!    the game's group, and the author: *"Don't just continue hunting
//!    unless they leave the group. But if they leave the group because
//!    they died well ...."*
//! 3. **Left the game's group**: the group hunts on without it.
//! 4. **Where it is, and whether it can move.**
//!
//! # Every wait has a deadline
//!
//! A wait ends [`Settings::lost_wait`] after `since`, when the member was
//! first seen apart from the leader: the caller keeps `since` while
//! [`muster`] answers `Some`, and forgets it on `None`. What the deadline
//! changes is INFERRED, from the author's words where they reach:
//!
//! | The wait | At the deadline | Why |
//! |---|---|---|
//! | its connection ([`Muster::Lost`]) | [`Muster::Gone`]: the handover | the author's design, *"after x amount of time"* (`plan/39` §1); what the handover does is Stage 6's, and waits on question 6 |
//! | walking over ([`Muster::Await`]) | [`Muster::Fetch`]: the group goes to it | *"go to them if they can't come to you immediately"* (question 7) |
//! | hindered here ([`Muster::Hold`]), or apart where the map cannot place it | [`Muster::Overdue`]: rest, when it can move | eohunter's stated intent for its barrier: *"the leader should go rest and wait rather than hunt on"*, and *"nobody gets left behind"* (`plan/39` §2) |

use std::iter;
use std::time::Instant;

use cena_map::RoomId;
use cena_session::State;

use super::report::{Hindrance, Report};
use super::settings::Settings;

/// What the group does about one member who is not with the leader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Muster {
    /// In the leader's room, but in roundtime, down or stuck: the group
    /// holds and keeps the room clear until `until`; a member down is
    /// pulled up, as now (`hunt/react.rs`). Question 7's first row.
    Hold {
        /// What keeps it.
        hindrance: Hindrance,
        /// The deadline.
        until: Instant,
    },
    /// In another room and able to move: it walks to the leader and rejoins
    /// (Follow, Stage 3). The group waits until `until`. Question 7's
    /// second row.
    Await {
        /// The deadline.
        until: Instant,
    },
    /// In another room and unable to move, or not arrived by its deadline:
    /// the leader walks the group to it, by its room on the board.
    /// Question 7's third row.
    Fetch(RoomId),
    /// Its connection dropped and Hydra is bringing it back: the members
    /// here fight what comes and nothing else, no wander and no walk to
    /// rest, until `until` (question 5; Stage 6 as `plan/39` §6 writes it).
    Lost {
        /// The deadline: `lost_wait` after it was first seen apart.
        until: Instant,
    },
    /// Its connection is gone for good (`Closed`: Hydra gave up, `plan/39`
    /// §8a), or the lost wait ran out: the handover, Stage 6's. `Closed`
    /// skips the wait, as question 5 proposed (*"Hydra knows when a session
    /// gives up ... so out of game could start the takeover at once"*).
    Gone,
    /// Dead: every member's hunt ends, and one member recovers it
    /// ([`recoverer`]; the recovery itself is Stage 7). Question 10.
    Dead,
    /// It left the game's group, by its own stop or a player's `leave`: the
    /// group hunts on without it. Question 7's last row.
    Left,
    /// A wait ran out and the member still cannot come: the group rests as
    /// soon as everyone can move, and the player is told.
    Overdue,
}

/// What the group does about `member`, with the leader in `leader_room`;
/// `None` when it is with the leader and nothing keeps it.
///
/// `since` is when `member` was first seen apart; `now` is the caller's
/// clock. See the module docs for the order and the deadlines.
#[must_use]
pub fn muster(
    member: &Report,
    leader_room: Option<RoomId>,
    since: Instant,
    now: Instant,
    settings: &Settings,
) -> Option<Muster> {
    let until = since.checked_add(settings.lost_wait).unwrap_or(since);
    let overdue = now >= until;
    match member.link {
        State::Ready => {}
        State::Closed => return Some(Muster::Gone),
        _ if overdue => return Some(Muster::Gone),
        _ => return Some(Muster::Lost { until }),
    }
    if member.hindrance == Some(Hindrance::Dead) {
        return Some(Muster::Dead);
    }
    if !member.grouped {
        return Some(Muster::Left);
    }
    if member.room.is_some() && member.room == leader_room {
        let hindrance = member.hindrance?;
        return Some(if overdue {
            Muster::Overdue
        } else {
            Muster::Hold { hindrance, until }
        });
    }
    let stuck = member.hindrance == Some(Hindrance::Stuck);
    Some(match member.room {
        Some(room) if stuck || overdue => Muster::Fetch(room),
        _ if overdue => Muster::Overdue,
        _ => Muster::Await { until },
    })
}

/// Who recovers a dead member: the leader if it is able, else the first
/// follower who is (`plan/39` §8, question 10: *"the leader if able, else
/// any member who can"*). Able is connected, alive and free to move
/// (INFERRED: the recovery takes the dead member's hand and carries it).
/// `None` when nobody can, and the player is told.
#[must_use]
pub fn recoverer<'a>(leader: &'a Report, followers: &'a [Report]) -> Option<&'a str> {
    iter::once(leader)
        .chain(followers)
        .find(|member| member.present() && member.hindrance != Some(Hindrance::Stuck))
        .map(|member| member.name.as_str())
}
