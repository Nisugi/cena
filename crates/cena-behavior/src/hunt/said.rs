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
    /// The map's `meta:` tags for the room, prefix removed (`splashy`,
    /// `nomagic`); empty when the map could not place it.
    pub tags: &'a [String],
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
    /// Stock the herb container at the herbalist and come back (`plan/36`
    /// Stage 4); `true` is eherbs' `fill`, one of each kind lacking.
    Stock(bool),
    /// Cast the waggle profile's spells on these people (`plan/37` Stage 5).
    Waggle(Vec<String>),
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
    /// `;heal stock` or `;heal fill` ran: there was no hunt, only the round.
    Stocked,
    /// `;waggle` ran: no hunt, only the spells.
    Waggled,
    /// `;sc` sent its lines.
    Sent,
    /// An injury refused an action again after a rest for it: nothing the
    /// rest did healed it.
    Injured,
    /// `rest.stop_after` rests were taken.
    Rested(u32),
    /// A dead player is in the room (`react.deader`).
    Deader,
    /// Every wand on the list is gone from the fresh container.
    NoWands,
    /// A weapon needs blessing and nothing known can bless it.
    Unblessed,
    /// Disarmed, and the weapon could not be got back.
    Disarmed,
    /// An attack had no effect: the weapon or ammunition cannot hurt what
    /// is here (`bigshot.lic:6398`).
    NoEffect,
    /// The dead man's switch: dead or badly hurt on Shattered, so the
    /// character quits (`hunt/death.rs`).
    Trouble,
    /// A quick hunt found nothing more to fight here (`hunt/quick.rs`).
    Cleared,
    /// Bounty mode: rested, with the bounty done or a new one ready.
    Bounty,
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
            Self::Stocked => f.write_str("stocked"),
            Self::Waggled => f.write_str("waggled"),
            Self::Sent => f.write_str("sent"),
            Self::Rested(n) => write!(f, "rested {n} times, as rest.stop_after asks"),
            Self::NoWands => f.write_str("no fresh wand is left"),
            Self::Deader => f.write_str("a dead player is here"),
            Self::Unblessed => f.write_str(
                "the weapon needs blessing, and neither Bless (304) nor the Voln symbol is known",
            ),
            Self::Disarmed => f.write_str("disarmed, and the weapon could not be recovered"),
            Self::Injured => f.write_str("an injury still stops the attack after resting for it"),
            Self::NoEffect => f.write_str(
                "an attack had no effect: this weapon or ammunition cannot hurt what is here",
            ),
            Self::Trouble => f.write_str(
                "the dead man's switch: dead or below 40% health on Shattered, so quitting",
            ),
            Self::Cleared => f.write_str("the room is clear"),
            Self::Bounty => f.write_str("the bounty is done or a new one is ready"),
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
    /// Bounty mode: the bounty is done, or a new one is ready.
    Bounty,
    /// Every bag is full: something wanted could go nowhere (`plan/31`;
    /// the author: *"too much loot"*).
    Loaded,
    /// The game refused an action for an injury.
    Injured,
    /// A box stayed in hand that no bag would take (`plan/31`; the author:
    /// *"we don't want to drop it, so we head in to rest"*).
    BoxInHand,
    /// Every member of a group lost its connection at once, and the group
    /// rests first when they are back (`plan/39` §8, question 9; the author:
    /// *"rest first. someone probably died."*). Only a group rests for it
    /// ([`crate::group::should_rest`]).
    Dropped,
}

impl fmt::Display for Why {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Wounded => "wounded",
            Self::Fried => "fried",
            Self::Encumbered => "encumbered",
            Self::Mana => "out of mana",
            Self::Bounty => "the bounty is done or ready",
            Self::Loaded => "too much loot",
            Self::BoxInHand => "a box in hand that no bag will take",
            Self::Injured => "too injured to fight",
            Self::Dropped => "every member's connection dropped",
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
