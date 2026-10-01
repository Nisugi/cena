//! A sack sold whole: at the furrier and the gem shop, a bag of what the shop
//! buys and nothing it is told to keep is taken off, sold at once, worn
//! again, and the note it paid with read and put away (`gemshop`,
//! `eloot.lic:6938`; `furrier`, `:6858`). What the sale left is sold item by
//! item after. Moved down from `plan.rs` at its cap.

use cena_session::GameState;
use cena_session::containers::StowSlot;

use super::super::goods;
use super::super::reply::Reply;
use super::{Seller, Step, free_a_hand, holds};

/// Where a sack sold whole is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum SackPhase {
    Fetching,
    Selling,
    Wearing,
    Reading(String),
    StowingNote(String),
}

impl Seller {
    pub(super) fn continue_sack(&mut self, state: &GameState) -> Option<Step> {
        let (sack, phase) = self.sack.clone()?;
        let in_hand = holds(state, &sack);
        match phase {
            SackPhase::Fetching => {
                if in_hand {
                    self.sack = Some((sack.clone(), SackPhase::Selling));
                    return Some(Step::SellSack(sack));
                }
                if let Some(free) = free_a_hand(state) {
                    return Some(free);
                }
                Some(Step::Fetch(sack))
            }
            SackPhase::Selling => Some(Step::SellSack(sack)),
            SackPhase::Wearing => {
                if in_hand {
                    return Some(Step::Wear(sack));
                }
                // Worn again. A note in a hand is the bulk sale's payment.
                if let Some(note) = goods::note_in_hand(state) {
                    self.sack = Some((sack, SackPhase::Reading(note.clone())));
                    return Some(Step::ReadNote(note));
                }
                self.sack = None;
                // The shop is visited again for what the sale left.
                self.lots_built = false;
                None
            }
            SackPhase::Reading(note) => Some(Step::ReadNote(note)),
            SackPhase::StowingNote(note) => {
                if holds(state, &note) {
                    let bag = state
                        .containers
                        .stow(StowSlot::Default)
                        .map(|b| b.id.clone())?;
                    return Some(Step::Stow { item: note, bag });
                }
                self.sack = None;
                self.lots_built = false;
                None
            }
        }
    }

    /// The sack's, the note's and the stow's answers.
    pub(super) fn sack_outcome(
        &mut self,
        last: &Step,
        sold: bool,
        refused: bool,
        replies: &[Reply],
        state: &GameState,
    ) {
        match last.clone() {
            Step::SellSack(sack) => {
                // Sold, or nothing in it the shop wants whole: item by item
                // then. Either way the sack goes back on.
                if sold || refused || replies.contains(&Reply::SackInspected) {
                    self.sack = Some((sack, SackPhase::Wearing));
                }
            }
            Step::Wear(sack) => {
                if replies.contains(&Reply::CannotWear) {
                    // Back in the default bag instead: stowed as the note
                    // is. (This named no item, so nothing was stowed and
                    // the sack stayed in the hand.)
                    self.sack = Some((sack.clone(), SackPhase::StowingNote(sack)));
                }
            }
            Step::ReadNote(note) => {
                self.bank.earned = true;
                self.sack = self
                    .sack
                    .take()
                    .map(|(sack, _)| (sack, SackPhase::StowingNote(note)));
            }
            Step::Stow { item, .. } if !holds(state, &item) => {
                if self
                    .lot
                    .as_ref()
                    .is_some_and(|(lot, _)| lot.item.id == item)
                {
                    self.lot = None;
                }
                if let Some((sack, SackPhase::StowingNote(_))) = &self.sack
                    && !holds(state, sack)
                {
                    self.sack = None;
                    self.lots_built = false;
                }
            }
            _ => {}
        }
    }
}
