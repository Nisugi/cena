//! Crossings no list of steps can describe (`plan/21` §4.6).
//!
//! Some areas rearrange themselves, and upstream's script for crossing one is
//! a *search*: explore, remember what led where, notice the area shifted,
//! start again. That is an algorithm, and it lives in Rust, in the Travel
//! behavior. The map carries only its **name and its arguments**.
//!
//! The set is open but small. A routine added by a later build is a name this
//! build cannot parse, so the exit loads as `Crossing::Unknown` -- impassable
//! -- and the walker routes around it (`crate::binary`, rule 1). This is the
//! one place a new upstream area can require a new Hydra, and the converter's
//! ratchet is what says so.
//!
//! This crate names them; it does not run them.

use serde::{Deserialize, Serialize};

use crate::room::RoomId;

/// A named search. In JSON: `{"name": "minotaur_maze", "rooms": [...]}`.
///
/// **The exit's own destination is always the goal** and is not repeated here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "name", rename_all = "snake_case")]
pub enum Routine {
    /// The Elemental Confluence: rooms whose exits reshuffle. Explore by
    /// compass, learn what led where, relearn when a room's exits change.
    ///
    /// `leave` is how upstream's two goals are told apart. `false`: reach the
    /// exit's destination, a room of the plane. `true`: find the point of
    /// elemental tranquility and go through it, which lands outside.
    Confluence { leave: bool },
    /// The minotaur maze beneath the Landing: the same search over a fixed
    /// set of rooms. Leaving the set means the walker fell out and replans.
    MinotaurMaze { rooms: Vec<RoomId> },
}
