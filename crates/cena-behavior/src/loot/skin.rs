//! Skinning: eloot's `Loot.skin` and `skin_obj_types` (`eloot.lic:5757-5895`),
//! as the planner's first phase when the profile turns it on.
//!
//! eloot skins before it searches, corpse by corpse, with a skinning weapon
//! in hand: the profile names an edged one and a blunt one, and four
//! creatures -- krynch, stone mastiff, krag dweller, cavern urchin -- take
//! the blunt (`:5886`). Before the first cut it may kneel, cast Sigil of
//! Resolve, and cast 604; after the last it stows the skinner in its sheath
//! unless it was already in hand when skinning began. What the game answers
//! decides the corpse's fate: *You cannot skin* teaches a creature that is
//! never skinnable, and the membership and free-account refusals end
//! skinning for the visit.
//!
//! Two of eloot's narrower rules are here too. `skin_bounty_only` skins only
//! the creature a skinning bounty names, and nothing without one
//! (`:5879-5883`). And a `rotting chimera` learned unskinnable is described
//! first, and skinned after all when *a huge scorpion tail rises high from
//! the rear*: only that form yields a skin (`occassional_skinner`, `:5616`).

use std::collections::VecDeque;

use cena_session::containers::ReadySlot;
use cena_session::containers::StowSlot;
use cena_session::{GameState, TaskKind};

use super::outcome::Outcome;
use super::plan::Step;
use super::profile::Skin;

/// Sigil of Resolve, as Lich casts a Sunfist sigil (`sunfist.rs`: 9704).
const SIGIL_OF_RESOLVE: &str = "incant 9704";
/// Bravery (604), refreshed when down or nearly so (`:5816`).
const BRAVERY: &str = "incant 604";
/// The creatures a blunt weapon skins (`:5886`).
const BLUNT: &[&str] = &["krynch", "stone mastiff", "krag dweller", "cavern urchin"];
/// Names never skinned (`:5873`).
const NEVER: &[&str] = &["ethereal", "ghostly", "unwordly", "Grimswarm", "child"];
/// The creature whose one form is skinnable though the name is not.
const CHIMERA: &str = "rotting chimera";

/// Which weapon a corpse takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Edge {
    Edged,
    Blunt,
}

/// Where the phase is.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Phase {
    /// Nothing sent yet for this group.
    Start,
    /// Preparing: the skinner is in hand; knelt, sigil, 604 as the profile asks.
    Ready {
        knelt: bool,
        resolve: bool,
        bravery: bool,
    },
    /// The group's corpses are skinned; the skinner goes back.
    Stowing,
}

/// The skinning phase for one visit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Skinning {
    settings: Skin,
    /// Corpses still to skin, with the weapon each takes; edged first.
    queue: VecDeque<(i64, String, Edge)>,
    /// The group in progress.
    edge: Option<Edge>,
    phase: Phase,
    /// The skinner in hand for the group, and whether it was there before.
    skinner: Option<(String, bool)>,
    knelt: bool,
    /// The game refused skinning for the visit.
    refused: bool,
    /// A gem broke out of a corpse into the left hand.
    gem: Option<String>,
    /// Rotting chimeras learned unskinnable, waiting on a `describe`.
    chimeras: Vec<(i64, String)>,
    described: bool,
}

impl Skinning {
    /// The phase for these corpses, named by the creature registry; corpses
    /// the profile or eloot's rule leaves alone are dropped here.
    pub(super) fn new(
        settings: Skin,
        state: &GameState,
        corpses: &[i64],
        unskinnable: &[String],
    ) -> Self {
        let mut named: Vec<(i64, String)> = corpses
            .iter()
            .filter_map(|id| state.creatures().get(*id).map(|c| (*id, c.name.clone())))
            .collect();
        if settings.bounty_only {
            // Only the bounty's creature; no skinning bounty, no skinning.
            let wanted = state
                .bounty
                .task()
                .filter(|task| task.kind == TaskKind::Skin)
                .and_then(|task| task.creature())
                .map(str::to_ascii_lowercase);
            named.retain(|(_, name)| {
                wanted
                    .as_deref()
                    .is_some_and(|creature| name.to_ascii_lowercase().contains(creature))
            });
        }
        let learned = |name: &str| unskinnable.iter().any(|u| u == name);
        let chimeras = named
            .iter()
            .filter(|(_, name)| learned(name) && name.contains(CHIMERA))
            .cloned()
            .collect();
        let mut queue: Vec<(i64, String, Edge)> = named
            .into_iter()
            .filter(|(_, name)| {
                !learned(name)
                    && !NEVER.iter().any(|word| name.contains(word))
                    && !settings
                        .exclude
                        .iter()
                        .any(|x| !x.is_empty() && name.contains(x))
            })
            .map(|(id, name)| {
                let edge = if BLUNT.iter().any(|b| name.to_ascii_lowercase().contains(b)) {
                    Edge::Blunt
                } else {
                    Edge::Edged
                };
                (id, name, edge)
            })
            .collect();
        // eloot's order: the edged group, then the blunt.
        queue.sort_by_key(|(_, _, edge)| *edge == Edge::Blunt);
        Skinning {
            settings,
            queue: queue.into(),
            edge: None,
            phase: Phase::Start,
            skinner: None,
            knelt: false,
            refused: false,
            gem: None,
            chimeras,
            described: false,
        }
    }

    /// The next step, or `None` when skinning is over for the visit.
    pub(super) fn next(&mut self, state: &GameState) -> Option<Step> {
        if let Some(gem) = self.gem.take() {
            return Some(Step::StowGem(gem));
        }
        if !self.chimeras.is_empty() && !self.described {
            self.described = true;
            return Some(Step::Describe("chimera".to_owned()));
        }
        if self.refused {
            self.queue.clear();
        }
        if let Phase::Stowing = self.phase {
            return self.stow(state);
        }
        let Some((_, _, edge)) = self.queue.front() else {
            // Done with every group: the skinner back, then stand.
            if self.skinner.is_some() {
                self.phase = Phase::Stowing;
                return self.stow(state);
            }
            if self.knelt {
                self.knelt = false;
                return Some(Step::Stand);
            }
            return None;
        };
        let edge = *edge;
        if self.edge != Some(edge) {
            // A new group: put its skinner in hand.
            if self.skinner.is_some() {
                self.phase = Phase::Stowing;
                return self.stow(state);
            }
            self.edge = Some(edge);
            self.phase = Phase::Start;
        }
        match self.phase.clone() {
            Phase::Start => {
                let Some((skinner, in_hand)) = skinner_for(&self.settings, edge, state) else {
                    // eloot: no blunt weapon listed, skip the group.
                    self.queue.retain(|(_, _, e)| *e != edge);
                    self.edge = None;
                    return self.next(state);
                };
                self.skinner = Some((skinner.clone(), in_hand));
                self.phase = Phase::Ready {
                    knelt: false,
                    resolve: false,
                    bravery: false,
                };
                if !in_hand {
                    return Some(Step::Wield(skinner));
                }
                self.next(state)
            }
            Phase::Ready {
                knelt,
                resolve,
                bravery,
            } => {
                if self.settings.kneel && !knelt && !self.knelt {
                    self.phase = Phase::Ready {
                        knelt: true,
                        resolve,
                        bravery,
                    };
                    self.knelt = true;
                    return Some(Step::Kneel);
                }
                if self.settings.resolve && !resolve {
                    self.phase = Phase::Ready {
                        knelt,
                        resolve: true,
                        bravery,
                    };
                    if !effect_up(state, "Sigil of Resolve") {
                        return Some(Step::Cast(SIGIL_OF_RESOLVE.to_owned()));
                    }
                }
                if self.settings.spell_604 && !bravery {
                    self.phase = Phase::Ready {
                        knelt,
                        resolve,
                        bravery: true,
                    };
                    if !effect_up(state, "Bravery") {
                        return Some(Step::Cast(BRAVERY.to_owned()));
                    }
                }
                let (corpse, _, _) = self.queue.front()?;
                let in_left = self
                    .skinner
                    .as_ref()
                    .is_some_and(|(id, _)| state.left_hand.holds(id));
                let hand = if in_left { "left" } else { "right" };
                Some(Step::Skin {
                    corpse: *corpse,
                    hand,
                })
            }
            Phase::Stowing => self.stow(state),
        }
    }

    /// The skinner back where it lives: its sheath by name, else the
    /// default bag. Not stowed when it was in hand to begin with.
    fn stow(&mut self, state: &GameState) -> Option<Step> {
        let Some((skinner, was_in_hand)) = self.skinner.take() else {
            self.phase = Phase::Start;
            self.edge = None;
            return self.next(state);
        };
        self.phase = Phase::Start;
        self.edge = None;
        if was_in_hand {
            return self.next(state);
        }
        let sheath_name = match self.last_edge_for(&skinner, state) {
            Edge::Blunt => &self.settings.sheath_blunt,
            Edge::Edged => &self.settings.sheath,
        };
        let bag = find_named(state, sheath_name)
            .or_else(|| {
                state
                    .containers
                    .ready(ReadySlot::Sheath)
                    .map(|s| s.id.clone())
            })
            .or_else(|| {
                state
                    .containers
                    .stow(StowSlot::Default)
                    .map(|b| b.id.clone())
            });
        bag.map(|bag| Step::Drag { item: skinner, bag })
    }

    /// Which weapon a skinner id is, by the profile's names.
    fn last_edge_for(&self, skinner: &str, state: &GameState) -> Edge {
        if !self.settings.weapon_blunt.is_empty()
            && find_named(state, &self.settings.weapon_blunt).as_deref() == Some(skinner)
        {
            Edge::Blunt
        } else {
            Edge::Edged
        }
    }

    /// What the game said to a skinning step. Returns the corpse's name when
    /// the creature was learned unskinnable, for the memory.
    pub(super) fn outcome(
        &mut self,
        step: &Step,
        outcome: &Outcome,
        state: &GameState,
    ) -> Option<String> {
        match (step, outcome) {
            (
                Step::Skin { corpse, .. },
                Outcome::Skinned | Outcome::Botched | Outcome::AlreadySkinned | Outcome::NotFound,
            ) => {
                self.queue.retain(|(id, _, _)| id != corpse);
                None
            }
            (Step::Skin { corpse, .. }, Outcome::CannotSkin) => {
                let name = self
                    .queue
                    .iter()
                    .find(|(id, _, _)| id == corpse)
                    .map(|(_, name, _)| name.clone());
                self.queue.retain(|(id, _, _)| id != corpse);
                name
            }
            (Step::Skin { .. }, Outcome::SkinNotAllowed) => {
                self.refused = true;
                None
            }
            (Step::Describe(_), Outcome::ScorpionTail) => {
                // The scorpion-tailed form: skinned with the edged group.
                for (id, name) in std::mem::take(&mut self.chimeras) {
                    self.queue.push_front((id, name, Edge::Edged));
                }
                None
            }
            (Step::Skin { .. }, Outcome::BrokeThrough) => {
                // The gem lands in the left hand (`:5849`).
                self.gem = state.left_hand.id().map(str::to_owned);
                None
            }
            _ => None,
        }
    }

    /// Nothing left to do.
    pub(super) fn is_done(&self) -> bool {
        self.queue.is_empty()
            && self.skinner.is_none()
            && !self.knelt
            && self.gem.is_none()
            && (self.chimeras.is_empty() || self.described)
    }
}

/// The skinner for this edge: the profile's named weapon found in the
/// inventory or a hand, and whether it is already in hand. With no edged
/// weapon named, the right hand's item serves (`:5780-5782`); with no blunt
/// one, `None`.
fn skinner_for(settings: &Skin, edge: Edge, state: &GameState) -> Option<(String, bool)> {
    let name = match edge {
        Edge::Edged => &settings.weapon,
        Edge::Blunt => &settings.weapon_blunt,
    };
    if name.is_empty() {
        return match edge {
            Edge::Edged => state.right_hand.id().map(|id| (id.to_owned(), true)),
            Edge::Blunt => None,
        };
    }
    let id = find_named(state, name)?;
    let in_hand = state.right_hand.holds(&id) || state.left_hand.holds(&id);
    Some((id, in_hand))
}

/// The id of the thing named `name`, a whole word of its text, in a hand or
/// in any container the inventory lists.
fn find_named(state: &GameState, name: &str) -> Option<String> {
    if name.is_empty() {
        return None;
    }
    let matches = |text: &str| {
        text.split(|c: char| !c.is_alphanumeric())
            .any(|word| word.eq_ignore_ascii_case(name))
            || text.eq_ignore_ascii_case(name)
    };
    for hand in [&state.right_hand, &state.left_hand] {
        if let (Some(id), Some(text)) = (hand.id(), hand.name())
            && matches(text)
        {
            return Some(id.to_owned());
        }
    }
    for (_, container) in state.inventory.containers() {
        if let Some(item) = container.items.iter().find(|item| matches(&item.text)) {
            return Some(item.id.clone());
        }
    }
    None
}

/// Is a named effect up, by the effects list?
fn effect_up(state: &GameState, name: &str) -> bool {
    let now = state.game_time_now();
    state.effects.iter().any(|(id, effect)| {
        effect.text.eq_ignore_ascii_case(name)
            && now.is_some_and(|now| state.effects.active(id, now) == Some(true))
    })
}
