//! The bank's part in a round: last, when the round earned or paid anything
//! (`finish_sell_run`, `eloot.lic:6109`); first, when the character is over
//! 80% encumbered on the way to a shop (`go_sell`, `:7132`); and between,
//! when the pool will not hand a box over to a character carrying so much or
//! a box's coins will not all fit: the bank, then back to the pool, whose
//! visit goes on where it was (`pool_return`, `:7443-7452`; `box_loot`,
//! `:5109-5115`). Moved down from `plan.rs` at its cap.
//!
//! The silver kept in hand is the profile's (`sell_keep_silver`), but for
//! `loot pool`, which keeps what was carried when it began and banks what the
//! boxes held and the tips cost (`pool`, `:7626-7648`).

use cena_map::RoomId;
use cena_session::GameState;

use super::{HEAVY, Round, Seller, Shop, Step};

/// Where the bank visit is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Banking {
    Depositing,
    Withdrawing,
}

/// The bank's part in one round.
#[derive(Clone, Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "four facts about the round's banking, each read in one place"
)]
pub(super) struct Bank {
    /// The visit, while it lasts.
    pub(super) visit: Option<Banking>,
    /// Silver kept in hand.
    pub(super) keep: u64,
    /// A sale, a note, a box's coins or a tip this round: the bank is
    /// wanted at the end.
    pub(super) earned: bool,
    /// The bank was visited since the last shop: no second trip for weight.
    pub(super) banked: bool,
    /// The round ends at the bank when it earned: all but the pool's
    /// returns alone, which eloot does not bank after (`pool`, `:7645`).
    pub(super) last: bool,
    /// A trip wanted now, the round coming back after it.
    pub(super) between: bool,
}

impl Bank {
    /// The bank for a round: `note` says one waits in the default bag.
    pub(super) fn for_round(round: Round, state: &GameState, keep_silver: u64, note: bool) -> Self {
        let keep = match round {
            // What was carried, by the `wealth` the driver asked first.
            Round::Pool { drop: true, .. } => {
                state.character.currency.silver.unwrap_or(keep_silver)
            }
            _ => keep_silver,
        };
        Self {
            visit: None,
            keep,
            earned: note,
            banked: false,
            last: !matches!(round, Round::Pool { drop: false, .. }),
            between: false,
        }
    }
}

impl Seller {
    /// The next shop: the bank first when the pool wants it or the
    /// character is heavy, the queue, the bank last when the round earned
    /// anything.
    pub(super) fn pick_shop(&mut self, state: &GameState) -> Option<Shop> {
        if std::mem::take(&mut self.bank.between) {
            return Some(Shop::Bank);
        }
        let heavy = state
            .character
            .encumbrance_percent
            .is_some_and(|now| now > HEAVY);
        if heavy && !self.bank.banked && !self.shops.is_empty() {
            return Some(Shop::Bank);
        }
        if let Some(shop) = self.shops.pop_front() {
            return Some(shop);
        }
        (self.bank.earned && !self.bank.banked && self.bank.last).then_some(Shop::Bank)
    }

    /// The pool wants the bank before it goes on: the round goes there, then
    /// back to the pool, whose visit is kept.
    pub(super) fn bank_between(
        &mut self,
        state: &GameState,
        nearest: &dyn Fn(&str) -> Option<RoomId>,
    ) -> Step {
        self.shops.push_front(Shop::Pool);
        self.shop = None;
        self.bank.between = true;
        self.decide(state, nearest)
    }

    pub(super) fn continue_bank(&mut self) -> Option<Step> {
        match self.bank.visit? {
            Banking::Depositing => Some(Step::DepositAll),
            Banking::Withdrawing => Some(Step::Withdraw(self.bank.keep)),
        }
    }

    /// `deposit all` answered: the silver kept is withdrawn, if any.
    pub(super) fn deposited(&mut self) {
        self.bank.visit = (self.bank.keep > 0).then_some(Banking::Withdrawing);
        self.close_bank();
    }

    /// The bank visit is over when no withdrawal is left.
    pub(super) fn close_bank(&mut self) {
        if self.bank.visit.is_none() {
            self.shop = None;
            self.bank.banked = true;
            self.bank.earned = false;
        }
    }
}
