//! The healer: the next command of a heal, given the state. Pure, as the
//! loot planner is (`plan/36` Stage 2).
//!
//! eherbs' `use_herbs` (`eherbs.lic:1916-1987`): while something is left to
//! treat ([`next_kind`]), a herb for it is found -- already in a hand, else
//! in the herb container, with a hand freed for it -- and eaten or drunk; a
//! kind the container has no herb for is skipped for the rest of the run,
//! and named. When nothing is left, the herbs still in hand go back to the
//! container.
//!
//! A kind treated more than [`KIND_USES`] times without healing is skipped
//! too: eherbs trusts each bite to work and would eat forever if one did not.

use std::collections::{BTreeMap, BTreeSet};

use cena_session::body::{Body, Track};
use cena_session::containers::StowSlot;
use cena_session::herbs::{self, HerbKind};
use cena_session::{GameState, RoomItem};

use super::choose::{Mode, Skipped, next_kind, skip_word};
use super::profile::HealProfile;
use super::reply::Reply;

/// Uses of one kind before it is given up on for the run.
pub const KIND_USES: u32 = 20;
/// Fetches of one herb before it is given up on.
const FETCHES: u8 = 2;

/// One command for the driver to send, or the end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// `look in <target>`: the herb container's contents, by `#id` or by
    /// `my <name>` while its id is not known.
    Look(String),
    /// `get #id`: a herb into a hand.
    Fetch(String),
    /// `eat my <noun>`.
    Eat(String),
    /// `drink my <noun>`.
    Drink(String),
    /// Put one thing in one bag.
    Stow {
        /// The item's id.
        item: String,
        /// The bag's id.
        bag: String,
    },
    /// The heal is over.
    Done(Healed),
}

/// How a heal ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Healed {
    /// Nothing left to treat that there was a herb for; the kinds with none
    /// are named.
    Done {
        /// Kinds the container had no herb for.
        missing: Vec<HerbKind>,
    },
    /// The herb container is not in the inventory.
    NoContainer,
    /// *Why don't you leave some for others?*
    Refused,
}

/// The healer for one run.
#[derive(Clone, Debug)]
pub struct Healer {
    profile: HealProfile,
    mode: Mode,
    skipped: Skipped,
    missing: Vec<HerbKind>,
    uses: BTreeMap<HerbKind, u32>,
    fetches: BTreeMap<String, u8>,
    /// Herbs that could not be fetched, by id.
    bad: BTreeSet<String>,
    /// The herb container's id, once found.
    sack: Option<String>,
    looked: bool,
    refused: bool,
    last: Option<Step>,
}

impl Healer {
    /// A healer for this profile. `spellcast` and `ranged` are eherbs'
    /// `--spellcast` and `--ranged` for this run.
    #[must_use]
    pub fn new(profile: HealProfile, spellcast: bool, ranged: bool) -> Self {
        let mode = Mode {
            skip_scars: profile.skip_scars,
            spellcast,
            ranged,
            blood_only: profile.blood_only,
        };
        Self {
            profile,
            mode,
            skipped: Skipped::new(),
            missing: Vec::new(),
            uses: BTreeMap::new(),
            fetches: BTreeMap::new(),
            bad: BTreeSet::new(),
            sack: None,
            looked: false,
            refused: false,
            last: None,
        }
    }

    /// Whether anything is left to treat, by the profile, before any herb
    /// is looked for: the rest's reason to heal.
    #[must_use]
    pub fn wanted(profile: &HealProfile, state: &GameState) -> bool {
        let mode = Mode {
            skip_scars: profile.skip_scars,
            blood_only: profile.blood_only,
            ..Mode::default()
        };
        next_kind(state, mode, &Skipped::new()).is_some()
    }

    /// The herb container's id, once found.
    #[must_use]
    pub fn container(&self) -> Option<&str> {
        self.sack.as_deref()
    }

    /// The next command.
    pub fn next(&mut self, state: &GameState) -> Step {
        let step = self.decide(state);
        self.last = Some(step.clone());
        step
    }

    fn decide(&mut self, state: &GameState) -> Step {
        if self.refused {
            return Step::Done(Healed::Refused);
        }
        let Some(kind) = next_kind(state, self.mode, &self.skipped) else {
            return self.finish(state);
        };
        if self.uses.get(&kind).copied().unwrap_or(0) >= KIND_USES {
            self.give_up(kind);
            return self.decide(state);
        }
        if let Some((noun, name)) = in_hand(state, kind) {
            *self.uses.entry(kind).or_insert(0) += 1;
            return if herbs::is_drinkable(&name) {
                Step::Drink(noun)
            } else {
                Step::Eat(noun)
            };
        }
        // eherbs fetches only with an arm to reach with (`:1929`).
        let body = Body::new(&state.character.injuries);
        let reach = body
            .rank("leftArm", Track::Wound)
            .min(body.rank("rightArm", Track::Wound))
            < 3;
        if !reach {
            self.give_up(kind);
            return self.decide(state);
        }
        let Some(sack) = self.find_sack(state) else {
            if self.looked {
                return Step::Done(Healed::NoContainer);
            }
            self.looked = true;
            return Step::Look(format!("my {}", self.profile.container));
        };
        let Some(contents) = state.inventory.container(&sack).map(|c| c.items.clone()) else {
            if self.looked {
                return Step::Done(Healed::NoContainer);
            }
            self.looked = true;
            return Step::Look(format!("#{sack}"));
        };
        let Some(herb) = self.find_herb(&contents, kind) else {
            self.give_up(kind);
            return self.decide(state);
        };
        if state.right_hand.is_holding() && state.left_hand.is_holding() {
            let Some(item) = state.right_hand.id().map(str::to_owned) else {
                return self.finish(state);
            };
            let bag = if hand_herb(state.right_hand.name()) {
                sack
            } else {
                state
                    .containers
                    .stow(StowSlot::Default)
                    .map_or(sack, |b| b.id.clone())
            };
            return Step::Stow { item, bag };
        }
        let tries = self.fetches.entry(herb.id.clone()).or_insert(0);
        *tries += 1;
        if *tries > FETCHES {
            self.bad.insert(herb.id);
            return self.decide(state);
        }
        Step::Fetch(herb.id)
    }

    /// No herb for this kind: skipped for the run, and named.
    fn give_up(&mut self, kind: HerbKind) {
        self.skipped.extend(skip_word(kind));
        if !self.missing.contains(&kind) {
            self.missing.push(kind);
        }
    }

    /// The herbs still in hand back into the container, then done.
    fn finish(&self, state: &GameState) -> Step {
        if let Some(sack) = &self.sack {
            for hand in [&state.right_hand, &state.left_hand] {
                if hand_herb(hand.name())
                    && let Some(id) = hand.id()
                {
                    return Step::Stow {
                        item: id.to_owned(),
                        bag: sack.clone(),
                    };
                }
            }
        }
        Step::Done(Healed::Done {
            missing: self.missing.clone(),
        })
    }

    /// The herb container: a container whose title holds every word of the
    /// profile's name (`find_herbsack`, `:3078`).
    fn find_sack(&mut self, state: &GameState) -> Option<String> {
        if let Some(sack) = &self.sack {
            return Some(sack.clone());
        }
        let words: Vec<String> = self
            .profile
            .container
            .split_whitespace()
            .map(str::to_ascii_lowercase)
            .collect();
        if words.is_empty() {
            return None;
        }
        let found = state
            .inventory
            .containers()
            .find(|(_, container)| {
                container.title.as_deref().is_some_and(|title| {
                    let title = title.to_ascii_lowercase();
                    words.iter().all(|w| {
                        title
                            .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '\'')
                            .any(|t| t == w)
                    })
                })
            })
            .map(|(id, _)| id.to_owned());
        self.sack.clone_from(&found);
        found
    }

    /// eherbs' `find_herb` (`:1872-1895`): yabathilium first for blood when
    /// the profile asks, then an edible herb of the kind unless potions are
    /// preferred, a drinkable one if they are, then any.
    fn find_herb(&self, contents: &[RoomItem], kind: HerbKind) -> Option<RoomItem> {
        let usable = |item: &&RoomItem| {
            !self.bad.contains(&item.id)
                && herbs::herb_for(&item.text).is_some_and(|herb| herb.kind == kind)
        };
        if kind == HerbKind::Blood && self.profile.yabathilium {
            let yaba = contents
                .iter()
                .find(|i| !self.bad.contains(&i.id) && i.text.contains("yabathilium"));
            if let Some(yaba) = yaba {
                return Some(yaba.clone());
            }
        }
        let drinkable = |item: &&RoomItem| herbs::is_drinkable(&item.text);
        let preferred = if self.profile.potions {
            contents.iter().filter(usable).find(drinkable)
        } else {
            contents.iter().filter(usable).find(|i| !drinkable(i))
        };
        preferred.or_else(|| contents.iter().find(usable)).cloned()
    }

    /// What the game said to the last step.
    pub fn outcome(&mut self, replies: &[Reply]) {
        if replies.contains(&Reply::LeaveSome) || replies.contains(&Reply::MustPick) {
            self.refused = true;
        }
        if let Some(Step::Fetch(id)) = &self.last
            && replies.contains(&Reply::Gone)
        {
            self.bad.insert(id.clone());
        }
    }
}

/// A herb of this kind in a hand: its noun and name.
fn in_hand(state: &GameState, kind: HerbKind) -> Option<(String, String)> {
    [&state.right_hand, &state.left_hand]
        .into_iter()
        .find_map(|hand| {
            let name = hand.name()?;
            let noun = hand.noun()?;
            herbs::herb_for(name)
                .filter(|herb| herb.kind == kind)
                .map(|_| (noun.to_owned(), name.to_owned()))
        })
}

/// Is what a hand holds a herb eherbs knows?
fn hand_herb(name: Option<&str>) -> bool {
    name.is_some_and(|name| herbs::herb_for(name).is_some())
}
