//! What `loot room` left in the hands (`loot_all`, `eloot.lic:5263`): the
//! game answers *too much* when the hands filled before the floor emptied,
//! and eloot drags a hand's thing away when it is one of those it was
//! looting. Moved down from `plan.rs` at its cap.

use cena_session::GameState;

use super::{DRAG_TRIES, Left, Planner, Step};

impl Planner {
    /// What `loot room` left in a hand, dragged to its bag: the room is
    /// looted again only once the hands are clear of it, or the game answers
    /// *too much* to every `loot room` there is.
    pub(super) fn empty_hands(&mut self, state: &GameState) -> Option<Step> {
        let held = [&state.right_hand, &state.left_hand]
            .into_iter()
            .filter_map(|hand| hand.id())
            .find_map(|id| self.gathered.iter().find(|(item, _)| item == id))?
            .clone();
        if !state.containers.stow_checked() {
            return Some(Step::Ask("stow list"));
        }
        let (id, types) = held;
        if *self.tries.get(&id).unwrap_or(&0) >= DRAG_TRIES {
            // It will not go away: no more of the room by `loot room`.
            self.gathered.clear();
            self.room_looted = true;
            return None;
        }
        *self.tries.entry(id.clone()).or_insert(0) += 1;
        let Some(bag) = self.bag_for(state, &types) else {
            self.bags_full = true;
            return Some(Step::Done(Left::BagsFull));
        };
        if let Some(open) = self.open_first(state, &bag) {
            return Some(open);
        }
        Some(Step::Drag { item: id, bag })
    }
}
