//! The selling round's commands: what the planner asks the driver to send.

use cena_map::RoomId;

#[cfg(doc)]
use super::reply::Reply;

/// One command for the driver to send, or the end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Walk there.
    Walk(RoomId),
    /// `get #id`: into a hand.
    Fetch(String),
    /// `sell #id`.
    Sell(String),
    /// `appraise #id`.
    Appraise(String),
    /// `analyze #id`, for a possible transmog.
    Analyze(String),
    /// `sell #sack`: the whole sack at once.
    SellSack(String),
    /// `wear #sack`: the sack back on after its bulk sale.
    Wear(String),
    /// `read #note`: the note a bulk sale paid with.
    ReadNote(String),
    /// `deposit #id`: a collectible handed in.
    Deposit(String),
    /// `give #item to #to`: a gold ring to the Chronomage's clerk.
    Give {
        /// The item's id.
        item: String,
        /// The clerk's id.
        to: String,
    },
    /// `read #id`: a scroll, for the spells the profile keeps
    /// (`sell_keep_scrolls`).
    ReadScroll(String),
    /// `bundle remove`: one skin out of the bundle in hand.
    Unbundle,
    /// `deposit all`: the silver and notes carried, into the account.
    DepositAll,
    /// `withdraw N silver`: what the profile keeps in hand.
    Withdraw(u64),
    /// `swap`: the box into the right hand, where the worker takes it.
    Swap,
    /// `give #to <amount>[ PERCENT][ confirm]`: a box and its tip to the
    /// pool's worker.
    Tip {
        /// The worker's id.
        to: String,
        /// The tip in silver, or a percent of the box's value.
        amount: u64,
        /// The tip is a percent.
        percent: bool,
        /// The second give, accepting the worker's quote.
        confirm: bool,
    },
    /// `ask #worker for return`: a box the pool has finished.
    AskReturn(String),
    /// The box in hand, emptied by the loot planner (`box_loot`); the driver
    /// runs it and says [`Reply::BoxLocked`] when the box would not open.
    EmptyBox(String),
    /// `trash #id`: an emptied box into the room's receptacle.
    Trash(String),
    /// `drop #id`: an emptied box, where there is no receptacle.
    Drop(String),
    /// Put one thing in one bag.
    Stow {
        /// The item's id.
        item: String,
        /// The bag's id.
        bag: String,
    },
    /// The round is over; the driver is home.
    Done,
}
