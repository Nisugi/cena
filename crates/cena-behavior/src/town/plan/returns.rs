//! The round at the pool: the visit begun on arrival, gone on with, sent to
//! the bank and back when it asks (`plan/bank.rs`), and over when it says
//! so; and what its returns bring to the round (`plan/61` step 5b).
//!
//! - **The shops follow what the returns brought.** eloot reads the bags for
//!   its shops after the boxes are done (`check_items` after `process_boxes`,
//!   `Sell.sell`, `eloot.lic:7820-7835`); a round deciding its shops before
//!   the pool would leave what the boxes held unsold. After each visit the
//!   shops for anything not seen before are queued, a shop already visited
//!   again too.
//! - **A box that still holds what no bag would take** (`pool_full_recovery?`,
//!   `:5384-5388`): held in hand while the round sells, its contents among the
//!   goods ([`super::super::goods::goods`]), and the pool visited again after,
//!   where it is emptied with the room the sales made and the returns go on
//!   (`pool_direct_sell_recovery`, `:5455-5491`). Once a box: set aside a
//!   second time, it stays in hand, and the round says so, as eloot pauses
//!   (`pool_sell_recovery`, `:5516-5526`). `loot pool` sells nothing, so its
//!   box stays in hand at once, as eloot's `pool_command` does not sell
//!   (`stow_box_item`, `:5363`).
//! - **What the hands held when the round began is never for sale**: a
//!   weapon stowed in a selling bag to free a hand is fetched back at the end,
//!   not sold at the next shop (the crate review of 2026-10-01, BE-E-6).
//!
//! Moved down from `plan.rs` at its cap.

use std::collections::BTreeSet;

use cena_map::RoomId;
use cena_session::GameState;

use super::super::goods::{self, Good, Shop};
use super::super::pool::Pool;
use super::{Round, Seller, Step};

impl Seller {
    /// At the pool: the visit begun on arrival, gone on with, sent to the
    /// bank and back when it asks, and over when it says so.
    pub(super) fn at_pool(
        &mut self,
        state: &GameState,
        nearest: &dyn Fn(&str) -> Option<RoomId>,
    ) -> Step {
        if self.pool.is_none() {
            let (drop, collect) = match self.round {
                Round::Pool { drop, collect } => (drop, collect),
                // A round gives the pool boxes when the profile uses it, and
                // asks for its returns always (`always_check_pool`).
                Round::All | Round::Bank => (self.town.pool, true),
            };
            // No worker in the room: the pool is passed by, and a box set
            // aside for it stays in hand.
            let aside = self.aside.take();
            self.pool = Pool::new(&self.town, state, self.worker.as_deref()).map(|pool| {
                let pool = pool.only(drop, collect);
                match aside.clone() {
                    Some(id) => pool.resume(id),
                    None => pool,
                }
            });
            if self.pool.is_none() {
                self.stuck = self.stuck.take().or(aside);
            }
        }
        if self.pool.as_mut().is_some_and(Pool::wants_bank) {
            return self.bank_between(state, nearest);
        }
        if let Some(step) = self.pool.as_mut().and_then(|pool| pool.next(state)) {
            return step;
        }
        let aside = self
            .pool
            .take()
            .and_then(|pool| pool.aside().map(str::to_owned));
        self.shop = None;
        self.after_pool(aside, state);
        self.decide(state, nearest)
    }

    /// What the round may part with now: the bags' goods and those of the
    /// box set aside, never what the hands held when it began.
    pub(super) fn goods(&self, state: &GameState) -> Vec<Good> {
        goods::goods(&self.town, state, self.aside.as_deref())
            .into_iter()
            .filter(|(item, _, _)| !self.restore.contains(&item.id))
            .collect()
    }

    /// The pool's visit is over, stopped at `aside` when it was: that box is
    /// held for the shops and the pool visited again after them, once; and
    /// the shops for what the returns brought are queued.
    pub(super) fn after_pool(&mut self, aside: Option<String>, state: &GameState) {
        let again = match aside {
            Some(id) if self.round == Round::All && self.set_aside.insert(id.clone()) => {
                self.aside = Some(id);
                true
            }
            Some(id) => {
                self.stuck = Some(id);
                false
            }
            None => false,
        };
        if self.round == Round::All {
            self.requeue(state);
        }
        if again {
            self.shops.push_back(Shop::Pool);
        }
    }

    /// Queue the shops for what the round has not seen before, each in its
    /// place: a shop already visited is visited again for it.
    pub(super) fn requeue(&mut self, state: &GameState) {
        let mut more = BTreeSet::new();
        for (item, types, _) in self.goods(state) {
            if !self.seen.insert(item.id.clone()) {
                continue;
            }
            if let Some(shop) = goods::shop_for(&self.town, &item, &types) {
                more.insert(shop);
            }
            // Clothing both shops buy is offered at the pawnshop too.
            if types.is("clothing") {
                more.insert(Shop::Pawnshop);
            }
        }
        for shop in more {
            if self.shop == Some(shop) || self.shops.contains(&shop) {
                continue;
            }
            let at = self
                .shops
                .iter()
                .position(|queued| *queued > shop)
                .unwrap_or(self.shops.len());
            self.shops.insert(at, shop);
        }
    }

    /// A box from the pool still in hand at the round's end, holding what no
    /// bag would take: the player's to see to.
    #[must_use]
    pub fn stuck(&self) -> Option<&str> {
        self.stuck.as_deref()
    }

    /// The box the hands must not let go of: the one set aside, or the one
    /// stuck in hand.
    pub(super) fn held_box(&self) -> Option<&str> {
        self.aside.as_deref().or(self.stuck.as_deref())
    }
}
