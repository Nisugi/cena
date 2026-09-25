//! The errand machines: a [`Hunt`] with no hunt, which runs one of the
//! driver's errands and ends -- `;heal`, `;heal stock`/`fill`, `;waggle`,
//! `;sc` -- or `;keep`, which does not end of its own accord. Split from
//! `engine.rs` when they took it past its cap.

use std::collections::BTreeMap;

use cena_session::GameState;

use super::engine::Hunt;
use super::profile::Profile;
use super::said::{Ending, Here, Said};
use crate::heal::HealProfile;
use crate::keep::{self, KeepProfile};
use crate::waggle::WaggleProfile;

impl Hunt {
    /// `;heal`: a machine that heals once by `profile` and ends, with no
    /// hunt around it. `spellcast` and `ranged` are eherbs' flags.
    #[must_use]
    pub fn heal_only(profile: HealProfile, spellcast: bool, ranged: bool) -> Self {
        let mut machine = Self::new(Profile::default(), 0).with_heal(profile);
        machine.heal_only = Some(false);
        machine.heal_mode = (spellcast, ranged);
        machine
    }

    /// `;heal stock` (`fill` false) or `;heal fill`: a machine that stocks
    /// the herb container once and ends.
    #[must_use]
    pub fn stock_only(profile: HealProfile, fill: bool) -> Self {
        let mut machine = Self::new(Profile::default(), 0).with_heal(profile);
        machine.stock_only = Some((false, fill));
        machine
    }

    /// `;keep`: a machine that keeps `profile`'s spells up and never ends of
    /// its own accord (`plan/37` Stage 4).
    #[must_use]
    pub fn keep_only(profile: KeepProfile) -> Self {
        let mut machine = Self::new(Profile::default(), 0);
        machine.keep_only = Some((profile, BTreeMap::new()));
        machine
    }

    /// `;waggle [names]`: a machine that casts the waggle profile's spells
    /// on these people once and ends.
    #[must_use]
    pub fn waggle_only(profile: WaggleProfile, targets: Vec<String>) -> Self {
        let mut machine = Self::new(Profile::default(), 0);
        machine.waggle_only = Some((profile, targets, false));
        machine
    }

    /// `;sc`: a machine that sends these lines, each through the gate, and
    /// ends.
    #[must_use]
    pub fn send_only(lines: Vec<String>) -> Self {
        let mut machine = Self::new(Profile::default(), 0);
        machine.send_only = Some(lines.into());
        machine
    }

    /// The waggle run's profile and names, when this machine is one.
    #[must_use]
    pub fn waggle(&self) -> Option<(&WaggleProfile, &[String])> {
        self.waggle_only
            .as_ref()
            .map(|(profile, targets, _)| (profile, targets.as_slice()))
    }

    /// The errand's next thing to do, when this machine is one.
    pub(super) fn errand(&mut self, state: &GameState, here: Here<'_>) -> Option<Said> {
        if let Some(lines) = self.send_only.as_mut() {
            return Some(match lines.pop_front() {
                Some(line) => Said::Send { line, target: None },
                None => Said::Done(Ending::Sent),
            });
        }
        if let Some((_, targets, asked)) = self.waggle_only.as_mut() {
            if *asked {
                return Some(Said::Done(Ending::Waggled));
            }
            *asked = true;
            return Some(Said::Waggle(targets.clone()));
        }
        if let Some((profile, tried)) = self.keep_only.as_mut() {
            if let Some(line) = self.pending.pop_front() {
                return Some(Said::Send { line, target: None });
            }
            let room = here.room.map(|r| r.0);
            return Some(match keep::next(profile, state, room, tried) {
                Some(lines) => {
                    self.pending = lines.into();
                    match self.pending.pop_front() {
                        Some(line) => Said::Send { line, target: None },
                        None => Said::Wait(1),
                    }
                }
                None => Said::Wait(2),
            });
        }
        if let Some((asked, fill)) = self.stock_only {
            self.stock_only = Some((true, fill));
            return Some(if asked {
                Said::Done(Ending::Stocked)
            } else {
                Said::Stock(fill)
            });
        }
        if let Some(asked) = self.heal_only {
            self.heal_only = Some(true);
            return Some(if asked {
                Said::Done(Ending::Healed)
            } else {
                Said::Heal
            });
        }
        None
    }
}
