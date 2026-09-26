//! Boon creatures: bigshot's `boons_ignore` and `boons_flee`
//! (`invalid_target_with_boons`, `should_flee_from_boons?`,
//! `bigshot.lic:8120-8135`, `:8525-8543`).
//!
//! A creature the object table types `boon` (`gameobj-data`, an adjective
//! before its name) is assessed once, `assess #<id>`, before the hunt
//! chooses a target, and the traits the reply names
//! (`cena_session::boons::assessed`) are kept by id. One with a trait on
//! `boons.ignore` is neither fought nor counted; one with a trait on
//! `boons.flee` sends the hunt out of the room. With both lists empty
//! nothing is assessed, as in bigshot.

use cena_session::GameState;
use cena_session::boons;
use cena_session::gameobj;

use super::engine::Hunt;
use super::said::Said;

impl Hunt {
    /// `assess #<id>` for a boon creature here not yet assessed, when the
    /// profile has a boon list.
    pub(super) fn assess_boons(&mut self, state: &GameState) -> Option<Said> {
        let lists = &self.profile.boons;
        if lists.ignore.is_empty() && lists.flee.is_empty() {
            return None;
        }
        let id = state
            .creatures()
            .in_room()
            .filter(|c| c.valid_target() && c.hostile() != Some(false))
            .find(|c| {
                !self.boons.contains_key(&c.id)
                    && gameobj::classify(c.noun.as_deref().unwrap_or_default(), &c.name).is("boon")
            })?
            .id;
        self.assessing = Some(id);
        Some(Said::Send {
            line: format!("assess #{id}"),
            target: None,
        })
    }

    /// Read an assessment's reply. An assessment the game did not answer
    /// with traits is kept as none, so it is not asked again.
    pub(super) fn boons_replied(&mut self, lines: &[&str]) {
        let Some(id) = self.assessing.take() else {
            return;
        };
        let traits = lines
            .iter()
            .find_map(|line| boons::assessed(line))
            .unwrap_or_default();
        self.boons.insert(id, traits);
    }

    /// A creature with a trait on `boons.ignore`.
    pub(super) fn boon_ignored(&self, id: i64) -> bool {
        self.has_boon(id, &self.profile.boons.ignore)
    }

    /// A creature here with a trait on `boons.flee`.
    pub(super) fn boon_to_flee(&self, state: &GameState) -> bool {
        state
            .creatures()
            .in_room()
            .any(|c| self.has_boon(c.id, &self.profile.boons.flee))
    }

    fn has_boon(&self, id: i64, list: &[String]) -> bool {
        self.boons
            .get(&id)
            .is_some_and(|traits| traits.iter().any(|t| list.iter().any(|l| l == t)))
    }
}
