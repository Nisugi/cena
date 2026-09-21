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
