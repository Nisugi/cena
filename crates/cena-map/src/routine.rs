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
    /// Walk a fixed circuit until something appears, then go through it. The
    /// Rift: its ways out -- a thread, a maw, a door, a mirror, a fissure --
    /// drift from room to room, so the walker goes round until it sees one.
    ///
    /// The walk starts at the walker's place on the circuit: its room's
    /// position in `starts` is the position in `dirs` to begin from, and
    /// `dirs` then repeats. The two lists are not the same length upstream
    /// and need not be. A walker whose room is not in `starts` is lost, and
    /// replans. **Where the way out lands is not known in advance**, so this
    /// routine always ends by finding out where it is and planning again.
    /// Follow signposts: each room on the way says which way to go from it,
    /// until the walker is at the exit's destination. The underwater route off
    /// River's Rest, where a current can carry the walker somewhere else on the
    /// route and the right direction depends on where it ends up.
    ///
    /// A room that is not in `dirs` means the walker is lost, and it replans;
    /// upstream swims in a random direction instead, which is not copied. The
    /// hands are emptied first when the walk starts in one of `hands_free_in`,
    /// and refilled at the end either way.
    Signposts {
        /// Put before each direction: `swim`.
        verb: String,
        /// `(room, direction)`, in upstream's order.
        dirs: Vec<(RoomId, String)>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        hands_free_in: Vec<RoomId>,
    },
    /// The Order of Voln's symbol of seeking: it offers a destination, a
    /// different one each time, and the walker asks again until the one offered
    /// is this exit's -- told by the room title the vision shows -- and then
    /// confirms. Upstream gives up when the offers come round to the first
    /// again, or after twenty.
    Seeking {
        /// Written down once the walker has arrived: which side of the Red
        /// Forest it sought its way into (`plan/21` §4.4).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        remember: Option<(String, String)>,
    },
    /// The Mist Harbor trinket, named by the profile's `fwi_trinket`: get it
    /// out if it is not worn, turn it, put it back. From a town it remembers
    /// the room left as `fwi_return_room`; from Mist Harbor it returns there.
    /// **Arrival in Mist Harbor is a random room, or one a GM has set**, so the
    /// routine ends by finding out where it is and planning again.
    Trinket,
    /// A rogue guild's door: `lean door`, then each verb of the profile's
    /// `rogue_password` on the door, then `go door`. An exit crossed this way
    /// is priced impassable when the profile has no password, so the walker
    /// is never sent to a door it cannot open.
    GuildPassword,
    /// A round tower room with four flights of steps, listed in an order
    /// that changes: `look`, read which flight is on this `wall`
    /// (`northern`), and climb that one -- `climb steps`, `climb second
    /// steps`, and so on. Upstream gives up when no flight is on the wall.
    FlightOfSteps { wall: String },
    /// Go to each of `rooms` in turn, by the map, until one shows a thing
    /// whose name holds `sees`; then `enter` it. A portal or a doorframe
    /// that wanders. `by_uid`: the rooms are the game's numbers, not the
    /// map's. Where it leads is not promised, so it ends by planning again.
    SearchRooms {
        rooms: Vec<u32>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        by_uid: bool,
        sees: String,
        enter: String,
    },
    /// Room 16165: turn the mirror right to its stop, then tilt it until the
    /// light falls on the centre of the statue -- starting the turn again
    /// from the left if it flips over -- and `go shadow`.
    Mirror,
    /// Room 8373: look at each of the stone ring's four wedges and turn the
    /// ring until it points at its sigil, pushing each wedge as it is set.
    RingWedges,
    /// The Graveyard's bronze gate, 4140 and 4141: `go gate` until through.
    /// Between tries, cast the first known of Unlock, Consecrate, Bless Item
    /// and Force Projection at it, waiting for the mana and casting again
    /// when armour hinders; failing that a Warrior of 15 batters it, where
    /// `batter` allows; failing that, push it with empty hands.
    BronzeGate { batter: bool },
    /// Room 30850: the barrier's colour says which walk leads to its
    /// grotto. Walk it, `touch crystal`, walk back, and `go barrier`.
    ColourBarrier,
    Patrol {
        /// `None` keeps a gap upstream left: positions matter.
        starts: Vec<Option<RoomId>>,
        dirs: Vec<String>,
        /// What to look for among the room's objects. The first listed that is
        /// present wins.
        landmarks: Vec<Landmark>,
        /// Commands sent once through: `stand`, after climbing a thread.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        after: Vec<String>,
    },
}

/// One way out a [`Routine::Patrol`] looks for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Landmark {
    /// The object's noun: `thread`.
    pub noun: String,
    /// The command that goes through it: `climb thread`.
    pub enter: String,
    /// Some must be worked open first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open: Option<Opening>,
}

/// Work a landmark open: send `command` until the game says `until`, at most
/// `tries` times, standing up again between tries if knocked down.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Opening {
    pub command: String,
    pub until: String,
    pub tries: u32,
}
