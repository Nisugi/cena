//! What a scripted crossing is made of (`plan/21` §3a, §4.8).
//!
//! A crossing is a **flat list of steps, each optionally guarded** (DECIDED,
//! author, 2026-09-20). Steps never contain steps: a loop or a search is one
//! step, or a named routine. This file grows one variant at a time, in the
//! order `research/mapdb-inventory/chokepoints.py` says opens the most rooms.
//!
//! A map may carry a step this build has never heard of. Such a crossing does
//! not fail the load: it arrives as `Crossing::Unknown` and is impassable
//! (`crate::binary`, rule 1).

use serde::{Deserialize, Serialize};

use crate::cond::Cond;

/// One thing the walker does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Send a movement command and wait to arrive, with everything a plain
    /// exit's crossing does: retries, standing, doors, roundtime.
    Move(String),
    /// Send a movement command **until the room changes**, however many times
    /// that takes: rowing a boat, or the Red Forest's fog, which turns the
    /// walker round and sets it back where it started. Being sent back is not
    /// a failure here, which is the difference from [`Action::Move`] -- that
    /// gives an exit up after a few tries. The walker bounds it.
    KeepMoving(String),
    /// Send a movement command **until the walker is at the exit's
    /// destination**. A forest whose one way in lands somewhere different each
    /// time: every send moves the walker, and only one landing is the goal.
    /// Where [`Action::KeepMoving`] stops at the first change of room, this
    /// stops at the right one. The walker bounds it; upstream tries fifty.
    MoveUntilThere(String),
    /// Try a movement command that may well not work, and carry on either
    /// way: a curtain that opens only once the locker is shut, a door that
    /// opens on its own schedule. Not moving is an answer, not a failure;
    /// the steps after it ask `Cond::StillHere` and do what is left to do.
    TryMove(String),
    /// Send a movement command again for as long as the question holds in
    /// the room the walker is then in: `northeast` while there is a `nw`.
    MoveWhile(String, Cond),
    /// Take the first obvious exit that is not this one: a room with two
    /// ways out, entered by one of them.
    MoveByAnyExitBut(String),
    /// Wait for the room to change with nothing sent: the walker is being
    /// carried.
    AwaitArrival,
    /// [`Action::Await`], for any one of several lines.
    AwaitAny(Vec<String>),
    /// Find out where the walker is and plan again from there. Always last.
    /// Upstream's `$go2_restart = true`, on crossings that may land somewhere
    /// other than the exit's destination (`plan/21` §4.3). Skipped when the
    /// walker is where it meant to be.
    Replan,
    /// Send a command that does not change rooms, and wait for the game to
    /// answer it: `open door`, `pull lever`, the first `event transport` that
    /// only asks for confirmation.
    Put(String),
    /// Wait for the game to say this, however long it takes: a ship's
    /// `A crew member escorts you off the ship.` The exit's cost says how
    /// long that usually is, and the walker's patience is scaled from it.
    Await(String),
    /// Cast a spell or use a society power, by name, and wait for it to land.
    /// How -- `incant`, `sigil of …`, `symbol of …` -- is the walker's
    /// business; the map says what, not how.
    Cast(String),
    /// Put away whatever is in the walker's hands, remembering what went
    /// where (`plan/21` §4.5). The walk never ends with anything still stowed,
    /// whether or not a matching [`Action::FillHands`] follows.
    EmptyHands,
    /// Take back what [`Action::EmptyHands`] put away, last first.
    FillHands,
    /// Forget a memory ([`Action::Remember`]): the way back out of an event
    /// ground clears where the walker came in from.
    Forget(String),
    /// Write down `.0 = .1` for a later crossing to ask about
    /// (`Cond::Remembered`): a room id, a realm, a location name. Done once the
    /// steps before it have succeeded, so a transport that failed leaves no
    /// false memory behind.
    Remember(String, String),
    /// Wait this many milliseconds. Whole milliseconds, not float seconds, so
    /// a crossing can be compared for equality.
    Pause(u32),
}

/// An [`Action`], and the question that decides whether it happens.
///
/// # Two kinds of check, in two places (author, 2026-09-21)
///
/// - **Can this exit be used at all?** That is the exit's *cost*
///   (`Cost::Gated`), asked **while planning**. If the answer is no, the
///   pathfinder never routes through it.
/// - **How is it crossed?** That is a step's `when`, asked **in the room**.
///   It may change the way across; it must never take the way across away.
///
/// So a crossing's steps always include something that moves the walker,
/// **whatever its guards answer and even if none can be answered** -- see
/// [`moves_whatever_is_known`], which the converter's ratchet holds every
/// ported crossing to. A guard that could strand the walker belongs in the
/// cost instead.
///
/// **The question is asked when the step is reached, not when the walk is
/// planned.** `cast Water Walking` followed by `go north` *when Water Walking is
/// up* only means anything if the second question sees the first step's
/// result.
///
/// In JSON the action's tag sits beside `when`: `{"pause": 4200, "when": {…}}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    #[serde(flatten)]
    pub action: Action,
    /// Absent: always. Present and unanswerable: skipped (`Cond::holds`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Cond>,
}

/// Whether these steps move the walker when **nothing** is known about it:
/// the weakest walker there is, so a list that passes moves everyone.
///
/// A step counts when it changes rooms -- [`Action::Move`], or an
/// [`Action::Await`], which is how a voyage ends -- and has no guard, or a
/// guard that holds for a walker with no facts at all.
#[must_use]
pub fn moves_whatever_is_known(steps: &[Step]) -> bool {
    let nobody = crate::cond::Walker::default();
    steps.iter().any(|step| {
        matches!(
            step.action,
            Action::Move(_)
                | Action::KeepMoving(_)
                | Action::MoveUntilThere(_)
                | Action::TryMove(_)
                | Action::MoveWhile(..)
                | Action::MoveByAnyExitBut(_)
                | Action::AwaitArrival
                | Action::AwaitAny(_)
                | Action::Await(_)
        ) && step.when.as_ref().is_none_or(|when| when.holds(&nobody))
    })
}
