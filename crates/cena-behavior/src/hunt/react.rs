//! What the hunt does about an incident (`state/incident.rs`): a weapon
//! knocked away is recovered, a weapon reaction offered is taken, a
//! sentinel's snake is clenched, and the rest are said or rested on.
//!
//! | Incident | The hunt | After |
//! |---|---|---|
//! | knocked away | kneel, then `recover item` until it is back, ten tries | ecleanse's `recover`, `ecleanse.lic:1088-1238` |
//! | torn free, floating | `get <noun>` until it is in hand, ten tries | ecleanse's `telekinetic_recover`, `:1548-1580` |
//! | caught in webbing | `pry my <noun>` | ecleanse's `recover_weapon_webbing`, `:1240-1296` |
//! | turned into a snake | `clench <noun>` | `sanctumwatch.lic:37-52` |
//! | a weapon reaction offered | `weapon <reaction>`, when `react.weapon_reaction` | bigshot's `perform_reaction`, `bigshot.lic:8051-8058` |
//! | too much held to pick up | rest: the bags are full | bigshot's `item_limit` |
//! | the hive's ground churning | leave the room | ecleanse's `hive_trap` |
//! | a curse, an infection, a bless gone, an ambusher | said to the player | |
//! | swallowed: The Belly of the Beast | `attack wall` until out | bigshot's `creature_escape`, `bigshot.lic:9638-9700` |
//! | swallowed: Ooze, Innards | `kill organ` until out | " |
//! | a Temporal Rift | an exit, until out | bigshot's `temporal_escape`, `:9706-9713` |
//!
//! bigshot also swaps in a dagger for the worm and a blunt weapon for the
//! ooze from the character's containers. Here the swallowed hunt fights out
//! with what is in hand, and says which kind of weapon would do it faster.
//!
//! ecleanse also casts 213 and 1011 and settles the room before searching,
//! and tells a spirit servant to fetch; those are its own settings, not
//! the hunt's, and are not here. The recovery is on unless `react.recover`
//! is off: ecleanse's is off by default because it is a separate script
//! the player chose to run, and here there is no other.

use cena_session::GameState;
use cena_session::incident::{Disarm, HiveTrap, Incident};

use super::engine::Hunt;
use super::said::{Ending, Said, Why};

/// Tries at a recovery before the hunt gives up (ecleanse's `search_count`).
const TRIES: u32 = 10;

/// A weapon to get back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Recovery {
    how: Disarm,
    /// Its noun, when the line named it.
    noun: Option<String>,
    tries: u32,
}

/// What incidents left the hunt to do.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Reacting {
    pub(super) recover: Option<Recovery>,
    /// A snake to clench, by noun.
    clench: Option<String>,
    /// A weapon reaction offered: the words after `weapon`.
    reaction: Option<String>,
    /// Leave this room: the ground is a trap.
    leave: bool,
    /// Swallowed or in a rift, and already said so.
    escaping: bool,
}

impl Hunt {
    /// Take the incidents the driver's fold has seen since the last turn.
    pub fn incidents(&mut self, incidents: &[Incident]) {
        for incident in incidents {
            match incident {
                Incident::Disarmed { how, weapon } if self.profile.react.recover => {
                    let noun = weapon.as_ref().map(|w| w.noun.clone());
                    self.notes.push(format!(
                        "disarmed: recovering the {}.",
                        noun.as_deref().unwrap_or("weapon")
                    ));
                    self.react.recover = Some(Recovery {
                        how: *how,
                        noun,
                        tries: 0,
                    });
                }
                Incident::Disarmed { weapon, .. } => self.notes.push(format!(
                    "disarmed ({}), and react.recover is off.",
                    weapon.as_ref().map_or("the weapon", |w| w.text.as_str())
                )),
                Incident::SanctumSnake { snake } => {
                    self.react.clench = snake.as_ref().map(|s| s.noun.clone());
                }
                Incident::WeaponReaction(reaction) if self.profile.react.weapon_reaction => {
                    self.react.reaction = Some(reaction.clone());
                }
                Incident::ItemLimit => self.must_rest = Some(Why::Loaded),
                Incident::HiveTrap(HiveTrap::Ground) => self.react.leave = true,
                Incident::ItchyCurse => self.notes.push("an itchy curse: see a healer.".to_owned()),
                Incident::InfectedWound => {
                    self.notes
                        .push("an infected wound: `clean vat` cures it.".to_owned());
                }
                Incident::BlessExpired(weapon) => self.notes.push(format!(
                    "the bless on the {} is gone.",
                    weapon.as_ref().map_or("weapon", |w| w.noun.as_str())
                )),
                Incident::Ambusher(noun) => self.notes.push(format!(
                    "ambushed{}.",
                    noun.as_ref()
                        .map(|n| format!(" by a {n}"))
                        .unwrap_or_default()
                )),
                _ => {}
            }
        }
    }

    /// The game said the weapon is back (`You spy ... and recover it!`).
    pub(super) fn recovered(&mut self) {
        if self.react.recover.take().is_some() {
            self.notes.push("recovered.".to_owned());
        }
    }

    /// The step an incident asks for now, before anything else but death.
    pub(super) fn react(
        &mut self,
        state: &GameState,
        here: super::said::Here<'_>,
        now: Option<u32>,
    ) -> Option<Said> {
        if let Some(said) = self.escape(state) {
            return Some(said);
        }
        if let Some(said) = self.recover_step(state) {
            return Some(said);
        }
        if let Some(noun) = self.react.clench.take() {
            return Some(Said::Send {
                line: format!("clench {noun}"),
                target: None,
            });
        }
        if std::mem::take(&mut self.react.leave) {
            self.notes.push("the ground is a trap: leaving.".to_owned());
            if let Some(to) = self.next_room(here, now) {
                return Some(Said::Walk(to));
            }
        }
        let reaction = self.react.reaction.take()?;
        Some(Said::Send {
            line: format!("weapon {reaction}"),
            target: None,
        })
    }

    /// Swallowed, or in a rift: the one way out, a step a tick.
    fn escape(&mut self, state: &GameState) -> Option<Said> {
        let title = state.room.title.as_deref()?;
        let (line, weapon) = if title.starts_with("The Belly of the Beast") {
            ("attack wall".to_owned(), "a dagger")
        } else if title.starts_with("Ooze, Innards") {
            ("kill organ".to_owned(), "a blunt weapon")
        } else if title.starts_with("Temporal Rift") {
            let exit = state.room.exits.as_ref()?.first()?.clone();
            (exit, "")
        } else {
            self.react.escaping = false;
            return None;
        };
        if !std::mem::replace(&mut self.react.escaping, true) {
            self.notes.push(if weapon.is_empty() {
                "in a Temporal Rift: walking out.".to_owned()
            } else {
                format!("swallowed: fighting out ({weapon} in the right hand is fastest).")
            });
        }
        Some(Said::Send { line, target: None })
    }

    /// One step of getting a weapon back.
    fn recover_step(&mut self, state: &GameState) -> Option<Said> {
        let recovery = self.react.recover.as_mut()?;
        let held = recovery.noun.as_deref().is_some_and(|noun| {
            state.right_hand.noun() == Some(noun) || state.left_hand.noun() == Some(noun)
        });
        if held {
            self.recovered();
            return None;
        }
        if recovery.tries >= TRIES {
            self.react.recover = None;
            return Some(Said::Done(Ending::Disarmed));
        }
        let line = match (recovery.how, recovery.noun.as_deref()) {
            (Disarm::Knocked, _) if state.status.known().kneeling() != Some(true) => {
                "kneel".to_owned()
            }
            (Disarm::Knocked, _) | (Disarm::Telekinetic | Disarm::Webbed, None) => {
                recovery.tries += 1;
                "recover item".to_owned()
            }
            (Disarm::Telekinetic, Some(noun)) => {
                recovery.tries += 1;
                format!("get {noun}")
            }
            (Disarm::Webbed, Some(noun)) => {
                // Prying frees it in hand: nothing to wait for.
                let line = format!("pry my {noun}");
                self.react.recover = None;
                line
            }
        };
        Some(Said::Send { line, target: None })
    }

    /// Whether a recovery is under way: survival does not stand the
    /// character up from the kneel it asked for.
    pub(super) fn recovering(&self) -> bool {
        self.react.recover.is_some()
    }
}
