//! The planner run by itself, with no hunt around it (`plan/61` step 1):
//! skinning alone, and how a run ended for whoever asked for it.

use super::{Left, Memory, Planner, Step};
use crate::loot::LootProfile;

impl Planner {
    /// A planner that skins the corpses here and stops (`;eloot skin`,
    /// `eloot.lic:8101`), whatever the profile's `skin.enable` says: the
    /// player asked.
    #[must_use]
    pub fn for_skinning(mut profile: LootProfile, memory: Memory, corpses: &[i64]) -> Self {
        profile.skin.enable = true;
        let mut planner = Self::new(profile, memory, corpses);
        planner.only_skin = true;
        planner
    }

    /// What the planner ended on, once it has said it is done.
    #[must_use]
    pub fn ended(&self) -> Option<Left> {
        match self.last {
            Some(Step::Done(left)) => Some(left),
            _ => None,
        }
    }
}
