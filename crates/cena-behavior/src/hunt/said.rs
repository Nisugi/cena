//! What the hunt machine says and where it stands: the vocabulary of
//! [`super::engine`], kept beside it so that file holds the decisions.

use std::fmt;

use cena_map::RoomId;

/// Where the character is, as the map knows it, and where it could go.
#[derive(Clone, Copy, Debug)]
pub struct Here<'a> {
    /// The room, when the map could place it.
    pub room: Option<RoomId>,
    /// The rooms one crossable exit away.
    pub exits: &'a [RoomId],
}

/// One thing for the driver to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Said {
    /// Send this line, gated at write time on `target` still being here.
    Send {
        /// The line, as the game takes it.
        line: String,
        /// The creature it is aimed at, for the gate.
        target: Option<i64>,
    },
    /// Walk there: travel, under the hunt's authority.
    Walk(RoomId),
    /// Nothing to do for this many seconds; fold events meanwhile.
    Wait(u32),
    /// The hunt is over.
    Done(Ending),
    /// Loot these corpses and the floor by the character's loot profile
    /// (`plan/31`): the driver runs the planner until it is done.
    Loot(Vec<i64>),
    /// Sell what the bags hold, at the shops and back (`plan/31` Stage 4):
    /// the driver runs the town planner until it is home again.
    Sell,
    /// Heal with herbs by the character's heal profile (`plan/36`): the
    /// driver runs the healer until it is done.
    Heal,
    /// Nothing this tick.
    Nothing,
}

/// Why a hunt ended of its own accord.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ending {
    /// The character died.
    Dead,
    /// A rest was needed and the profile names no resting room.
    NoRestingRoom,
    /// The walk back needs a hunting room and the profile names none.
    NoHuntingRoom,
    /// A walk the hunt depends on could not be made.
    Unreachable(RoomId),
    /// `;heal` ran: there was no hunt, only the healing.
    Healed,
}

impl fmt::Display for Ending {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dead => f.write_str("the character died"),
            Self::NoRestingRoom => {
                f.write_str("a rest is needed and the profile names no resting room")
            }
            Self::NoHuntingRoom => f.write_str("the profile names no hunting room to return to"),
            Self::Unreachable(room) => write!(f, "there is no way to room {}", room.0),
            Self::Healed => f.write_str("healed"),
        }
    }
}

/// Why the hunt is resting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// A `rest.when` condition holds.
    Wounded,
    /// The mind is at or above `rest.fried`, with the overkill spent.
    Fried,
    /// Encumbrance is at or above `rest.encumbered`.
    Encumbered,
    /// Mana is below `rest.mana_below`.
    Mana,
    /// Every bag is full: something wanted could go nowhere (`plan/31`;
    /// the author: *"too much loot"*).
    Loaded,
    /// A box stayed in hand that no bag would take (`plan/31`; the author:
    /// *"we don't want to drop it, so we head in to rest"*).
    BoxInHand,
}

impl fmt::Display for Why {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Wounded => "wounded",
            Self::Fried => "fried",
            Self::Encumbered => "encumbered",
            Self::Mana => "out of mana",
            Self::Loaded => "too much loot",
            Self::BoxInHand => "a box in hand that no bag will take",
        })
    }
}

/// Where the hunt is in its cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// In the hunting ground.
    Hunting,
    /// Walking to the resting room.
    ToRest(Why),
    /// Arrived to rest with loot to sell: the selling round, then the rest.
    Selling(Why),
    /// Arrived to rest hurt: the herbs, then the rest.
    Healing(Why),
    /// At the resting room, until the thresholds are met.
    Resting(Why),
    /// Walking back to the hunting room.
    Returning,
    /// At the hunting room, sending the prepare commands.
    Preparing,
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hunting => f.write_str("hunting"),
            Self::ToRest(why) => write!(f, "{why}: walking to the resting room"),

            Self::Selling(why) => write!(f, "selling before resting ({why})"),
            Self::Healing(why) => write!(f, "healing before resting ({why})"),
            Self::Resting(why) => write!(f, "resting ({why})"),
            Self::Returning => f.write_str("rested: walking back"),
            Self::Preparing => f.write_str("preparing"),
        }
    }
}
