//! Which creatures the hunt fights, and which it fights first: the
//! profile's target list read against the room (`bigshot.lic:8652-8668`,
//! `sort_npcs`; `:7170`, a name or a noun).

use cena_session::{CreatureInstance, GameState};

use super::engine::Hunt;
use super::profile::Target;

impl Hunt {
    /// The target: the current one while it is still here and worth
    /// attacking, else the best by the profile's order.
    pub(super) fn choose_target(&mut self, state: &GameState) -> Option<i64> {
        if let Some(current) = self.target
            && let Some(here) = self
                .fightable(state)
                .find(|creature| creature.id == current)
        {
            let rank = |creature| self.rank(creature).map(|(at, _)| at);
            let held = rank(here);
            let outranked = self.profile.priority
                && self.fightable(state).any(|creature| rank(creature) < held);
            if !outranked {
                return Some(current);
            }
        }
        let mut best: Option<(usize, i64, String)> = None;
        for creature in self.fightable(state) {
            let Some((rank, target)) = self.rank(creature) else {
                continue;
            };
            if best.as_ref().is_none_or(|(at, _, _)| rank < *at) {
                best = Some((rank, creature.id, target.routine.clone()));
            }
        }
        let (_, id, routine) = best?;
        self.target = Some(id);
        self.aiming.reset();
        self.routine = routine;
        self.cursor = 0;
        self.queue.clear();
        Some(id)
    }

    /// The first target entry that fits a creature, with its place. An
    /// `any` entry does not take a hazard creature (a wasp nest, a
    /// shimmering fungus: `inventory/12` §2); one that names it does.
    fn rank(&self, creature: &CreatureInstance) -> Option<(usize, &Target)> {
        let noun = creature.noun.as_deref();
        self.profile.targets.iter().enumerate().find(|(_, target)| {
            (target.any && !creature.hazard())
                || target
                    .name
                    .as_deref()
                    .is_some_and(|n| named(n, &creature.name, noun))
        })
    }

    /// The creatures here the hunt could fight: alive, not known to be
    /// unhostile, nobody's familiar, companion or summons
    /// ([`CreatureInstance::ally`]), and not on the never-attack list. What
    /// the flee count counts, whether or not the target list names them.
    pub(super) fn could_fight<'a>(
        &'a self,
        state: &'a GameState,
    ) -> impl Iterator<Item = &'a CreatureInstance> + 'a {
        state.creatures().in_room().filter(move |creature| {
            creature.valid_target()
                && creature.hostile() != Some(false)
                && creature.ally().is_none()
                && !self.boon_ignored(creature.id)
                && !listed(&self.profile.never_attack, creature)
        })
    }

    /// Those the target list names: the creatures worth attacking.
    pub(super) fn fightable<'a>(
        &'a self,
        state: &'a GameState,
    ) -> impl Iterator<Item = &'a CreatureInstance> + 'a {
        self.could_fight(state)
            .filter(move |creature| self.rank(creature).is_some())
    }
}

/// Whether `wanted` names a creature: its noun, or its whole name, ignoring
/// case (`bigshot.lic:7170`).
fn named(wanted: &str, name: &str, noun: Option<&str>) -> bool {
    name.eq_ignore_ascii_case(wanted) || noun.is_some_and(|noun| noun.eq_ignore_ascii_case(wanted))
}

/// Whether any of `names` names this creature ([`named`]).
pub(super) fn listed(names: &[String], creature: &CreatureInstance) -> bool {
    names
        .iter()
        .any(|wanted| named(wanted, &creature.name, creature.noun.as_deref()))
}
