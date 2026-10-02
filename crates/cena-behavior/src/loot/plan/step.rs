//! The planner's words: one command for the driver to send, or how the
//! visit ended. Moved down from `plan.rs` at its cap.

/// One command for the driver to send, or the end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// A stance command (`loot_defensive`).
    Stance(String),
    /// Ask the game for a fact the planner needs: `stow list`.
    Ask(&'static str),
    /// Cast a spell: the sigil, after a failed search.
    Cast(String),
    /// `loot #id` on a corpse.
    Search(i64),
    /// `open #bag`: a critter's bag, or a bag that is shut.
    Open(String),
    /// `close #bag`: a bag opened this visit, when the profile keeps them
    /// closed.
    Close(String),
    /// `look in #bag`: a critter's bag, to learn what it holds.
    LookIn(String),
    /// `loot room`: everything left on the floor is wanted.
    LootRoom,
    /// `loot #id` on one thing the game stows itself.
    LootItem(String),
    /// Drag one thing into one bag.
    Drag {
        /// The item's id.
        item: String,
        /// The bag's id.
        bag: String,
    },
    /// `get #id`: the skinning weapon into a hand (`skin.rs`).
    Wield(String),
    /// `kneel`, before skinning.
    Kneel,
    /// `stand`, after skinning knelt.
    Stand,
    /// `skin #corpse <hand>`: the hand holding the skinner.
    Skin {
        /// The corpse's id.
        corpse: i64,
        /// `left` or `right`.
        hand: &'static str,
    },
    /// `stow gem #id`: a gem that broke out of a corpse into the left hand.
    StowGem(String),
    /// `describe <noun>`: a creature whose form decides whether it skins.
    Describe(String),
    /// `get coins from #box`: a box's coins, by hand.
    Coins(String),
    /// `point <charm> at #box`: a box's coins, by the profile's charm.
    Charm {
        /// The charm, by name.
        charm: String,
        /// The box's id.
        box_: String,
    },
    /// A line that empties a hand to loot with, or gives back what was
    /// put away: `store left`, `get #id`, `remove #id`, `_drag #id right`.
    Hand(String),
    /// Nothing more to do here.
    Done(Left),
}

/// How the looting ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Left {
    /// Everything wanted is stowed.
    Nothing,
    /// Something wanted could go in no bag: a reason to rest (author:
    /// *"too much loot"*).
    BagsFull,
    /// A box stayed in hand that no bag would take: a reason to rest
    /// (author: *"we don't want to drop it, so we head in to rest"*).
    BoxInHand,
    /// Neither hand is fit to loot with: a wound or a scar of rank 3 on
    /// each arm or hand (`free_hand`, `eloot.lic:3816-3837`). A reason to
    /// rest.
    NoHand,
}
