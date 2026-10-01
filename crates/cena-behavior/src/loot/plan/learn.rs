//! What a visit learns from the game's answers, for the hunt's memory and
//! for the profile ([`Learned`]). Moved down from `plan.rs` at its cap.

use cena_session::RoomItem;
use cena_session::gameobj::classify;

use super::super::learned::Learned;
use super::super::outcome::Outcome;
use super::{Planner, Step};

impl Planner {
    /// A name learned crumbly or unlootable, by the item the last step
    /// touched: the driver calls this with the item when the outcome was
    /// [`Outcome::Crumbled`] or [`Outcome::Unlootable`]. Kept for the hunt,
    /// and named for the profile as eloot saves it: a thing that crumbled
    /// always (`store_item`, `eloot.lic:4149-4153`), but not a critter's
    /// bandana or robes that crumble as they are opened (`bag_loot`,
    /// `:5045-5050`); a thing that could not be held only when the profile
    /// remembers them, and only one of no known kind (`ELoot.unlootable`,
    /// `:2951-2959`).
    pub fn learn_item(&mut self, outcome: &Outcome, item: &RoomItem) {
        let name = item.text.as_str();
        match outcome {
            Outcome::Crumbled => {
                let opened = matches!(self.last, Some(Step::Open(_)));
                if opened
                    && ["bandana", "flowing robes"]
                        .iter()
                        .any(|w| name.contains(w))
                {
                    return;
                }
                self.memory.crumbly.insert(name.to_owned());
                if !self.profile.crumbly.iter().any(|known| known == name) {
                    Learned::add(&mut self.learned.crumbly, name);
                }
            }
            Outcome::Unlootable => {
                self.memory.unlootable.insert(name.to_owned());
                if self.profile.remember_unlootable
                    && classify(&item.noun, name).types.is_empty()
                    && !self.profile.unlootable.iter().any(|known| known == name)
                {
                    Learned::add(&mut self.learned.unlootable, name);
                }
            }
            _ => {}
        }
    }

    /// [`Self::learn_item`], knowing only the thing's name.
    pub fn learn(&mut self, outcome: &Outcome, name: &str) {
        self.learn_item(
            outcome,
            &RoomItem {
                id: String::new(),
                noun: String::new(),
                text: name.to_owned(),
                before: None,
                after: None,
                status: None,
            },
        );
    }

    /// What this visit learned that the profile does not hold yet.
    #[must_use]
    pub fn learned(&self) -> &Learned {
        &self.learned
    }
}
