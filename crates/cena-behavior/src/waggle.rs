//! Waggle: ewaggle, spells cast on a list of people (`plan/37` Stage 5).
//! Pure, as the healer is; the driver runs it inside the hunt's.
//!
//! ewaggle (`reference/scripts/scripts/ewaggle.lic`, 1,796 lines) takes a
//! list of names -- yourself when none -- learns what each has up (your own
//! effects, or `spell active <name>` for anyone else), then runs its three
//! passes over the cast list (`Casting`, `:1436-1549`):
//!
//! - **stackable** spells, 625 and 1612 first: while under `stop_at` minutes
//!   and only when under `start_at`, cast as many at once as the ranks allow
//!   and the spell takes (`max_multicast`, `:1709`), counting each cast's
//!   minutes in; one cast only on someone not sharing what they have up;
//! - **refreshable** spells under `refreshable_min` minutes;
//! - **the rest**, when absent.
//!
//! Every pass skips a spell this caster cannot put on this target (known,
//! `all` availability for another, a duration over three minutes), a bard
//! spell already up on yourself, a spell a room refused and a target the game
//! could not find. Too little mana waits, or ends the run when the profile
//! says bail.
//!
//! Not ported: armor specializations, sonic gear, mana bread, the wracking
//! and Symbol of Mana mana sources, 9714/515 support casts, Retribution.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use cena_session::spells::{CastType, Span};
use cena_session::{GameState, SkillKind};

use crate::cast::{self, Answer, Casting, NotReady};

/// ewaggle's short spell: three minutes or less is not worth casting.
const SHORT: f64 = 3.0;

/// How the character waggles (`load_defaults`, `ewaggle.lic:664-737`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WaggleProfile {
    /// The spells cast, by number.
    pub cast_list: Vec<u16>,
    /// A stackable spell is topped up only under this many minutes.
    pub start_at: f64,
    /// ...and up to this many; ewaggle caps it at 250.
    pub stop_at: f64,
    /// A refreshable spell is recast under this many minutes.
    pub refreshable_min: f64,
    /// Cast several at once where the ranks allow.
    pub multicast: bool,
    /// Mana kept back.
    pub reserve_mana: u32,
    /// End the run when mana runs short, rather than wait for it.
    pub bail: bool,
    /// Skip anyone not sharing what they have up.
    pub skip_not_sharing: bool,
}

impl Default for WaggleProfile {
    fn default() -> Self {
        Self {
            cast_list: Vec::new(),
            start_at: 180.0,
            stop_at: 180.0,
            refreshable_min: 15.0,
            multicast: true,
            reserve_mana: 0,
            bail: false,
            skip_not_sharing: false,
        }
    }
}

/// The character's waggle profile: `<data>/hunt/waggle/<instance>_<character>.toml`.
#[must_use]
pub fn path(dir: &std::path::Path, instance: &str, character: &str) -> Option<std::path::PathBuf> {
    let file = crate::hunt::chain::file_name(&format!("{instance}_{character}"))?;
    Some(dir.join("hunt").join("waggle").join(format!("{file}.toml")))
}

impl WaggleProfile {
    /// Read a profile file's text.
    ///
    /// # Errors
    ///
    /// Not TOML, or a key this does not know.
    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }
}

/// One command for the driver, or the end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// `spell active <name>`: what someone has up.
    Ask(String),
    /// Cast.
    Cast(Casting),
    /// Wait this many seconds, for mana or cast roundtime.
    Wait(u32),
    /// The run is over.
    Done(Waggled),
}

/// How a run ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Waggled {
    /// Everyone done; how many casts were sent.
    Done(u32),
    /// Mana ran short and the profile says bail.
    OutOfMana,
}

/// The three passes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Pass {
    Stackable,
    Refreshable,
    Solid,
}

/// The waggler for one run.
#[derive(Clone, Debug)]
pub struct Waggler {
    profile: WaggleProfile,
    targets: VecDeque<String>,
    /// The target in hand, its minutes left by spell, whether it shares.
    target: Option<(String, BTreeMap<u16, f64>, bool)>,
    asked: bool,
    pass: Pass,
    at: usize,
    skip_spells: BTreeSet<u16>,
    skip_targets: BTreeSet<String>,
    casts: u32,
    last: Option<(u16, u32)>,
}

impl Waggler {
    /// A run over `targets` (names; `self` or empty for yourself).
    #[must_use]
    pub fn new(mut profile: WaggleProfile, targets: &[String]) -> Self {
        profile.stop_at = profile.stop_at.min(250.0);
        let mut targets: VecDeque<String> = targets.iter().cloned().collect();
        if targets.is_empty() {
            targets.push_back("self".to_owned());
        }
        Self {
            profile,
            targets,
            target: None,
            asked: false,
            pass: Pass::Stackable,
            at: 0,
            skip_spells: BTreeSet::new(),
            skip_targets: BTreeSet::new(),
            casts: 0,
            last: None,
        }
    }

    /// The next command.
    pub fn next(&mut self, state: &GameState) -> Step {
        loop {
            let Some((name, _, _)) = &self.target else {
                let Some(name) = self.targets.pop_front() else {
                    return Step::Done(Waggled::Done(self.casts));
                };
                if is_self(&name, state) {
                    self.target = Some((name, own_minutes(state), true));
                } else {
                    self.asked = true;
                    self.target = Some((name.clone(), BTreeMap::new(), true));
                    return Step::Ask(name);
                }
                self.pass = Pass::Stackable;
                self.at = 0;
                continue;
            };
            let name = name.clone();
            if self.skip_targets.contains(&name) {
                self.target = None;
                continue;
            }
            if let Some((spell, count)) = self.candidate(state, &name) {
                {
                    if let Err(why) = cast::ready(state, spell, count, self.profile.reserve_mana) {
                        return match why {
                            NotReady::Mana(..) | NotReady::Spirit | NotReady::Stamina
                                if self.profile.bail =>
                            {
                                Step::Done(Waggled::OutOfMana)
                            }
                            NotReady::NotKnown => {
                                self.skip_spells.insert(spell);
                                continue;
                            }
                            NotReady::CastRoundtime(secs) => Step::Wait(secs.max(1)),
                            _ => Step::Wait(5),
                        };
                    }
                    self.last = Some((spell, count));
                    let target = (!is_self(&name, state)).then(|| name.clone());
                    return Step::Cast(Casting {
                        spell,
                        target,
                        count: (count > 1).then_some(count),
                        ..Casting::default()
                    });
                }
            }
            // This pass is done for this target: the next one, or the next
            // target.
            self.at = 0;
            self.pass = match self.pass {
                Pass::Stackable => Pass::Refreshable,
                Pass::Refreshable => Pass::Solid,
                Pass::Solid => {
                    self.target = None;
                    Pass::Stackable
                }
            };
        }
    }

    /// The next spell and count this pass casts on `name`, advancing past
    /// what it does not.
    fn candidate(&mut self, state: &GameState, name: &str) -> Option<(u16, u32)> {
        let list = self.ordered();
        while self.at < list.len() {
            let spell = list[self.at];
            if let Some(count) = self.wants(state, name, spell) {
                return Some((spell, count));
            }
            self.at += 1;
        }
        None
    }

    /// The cast list in pass order: for stacking, 625 and 1612 first
    /// (`:1499`).
    fn ordered(&self) -> Vec<u16> {
        let mut list = self.profile.cast_list.clone();
        if self.pass == Pass::Stackable {
            list.sort_by_key(|n| (![625, 1612].contains(n), *n));
        }
        list
    }

    /// How many to cast of `spell` on `name` in this pass, or `None` for
    /// none.
    fn wants(&self, state: &GameState, name: &str, spell: u16) -> Option<u32> {
        if self.skip_spells.contains(&spell) {
            return None;
        }
        let (_, have, sharing) = self.target.as_ref()?;
        let myself = is_self(name, state);
        let cast = if myself {
            CastType::SelfCast
        } else {
            CastType::Target
        };
        let data = cena_session::spells::spell(spell)?;
        if state.known_spells.knows(u32::from(spell)) == Some(false) {
            return None;
        }
        if !myself && data.availability.as_deref() != Some("all") {
            return None;
        }
        let minutes = state.spell_minutes(spell, cast)?;
        if minutes <= SHORT {
            return None;
        }
        let circle = spell / 100;
        if myself && circle == 10 && have.contains_key(&spell) {
            return None;
        }
        let span = data
            .extras
            .shape(cast)
            .or_else(|| data.extras.shape(CastType::SelfCast));
        let span_kind = span.and_then(|s| s.span);
        let left = have.get(&spell).copied().unwrap_or(0.0);
        match self.pass {
            Pass::Stackable => {
                if span_kind != Some(Span::Stackable)
                    || left > self.profile.start_at
                    || left >= self.profile.stop_at
                {
                    return None;
                }
                let need = ((self.profile.stop_at - left) / minutes).ceil();
                let multicastable = span.and_then(|s| s.multicastable) == Some(true);
                let most = if self.profile.multicast && multicastable && (*sharing || myself) {
                    max_multicast(state, circle)
                } else {
                    1
                };
                // A float count of casts, whole and at least one.
                let need = if need >= f64::from(most) {
                    most
                } else {
                    (1..=most).find(|n| f64::from(*n) >= need).unwrap_or(1)
                };
                // Someone not sharing gets one cast: its time cannot be known.
                (need > 0 && (*sharing || left == 0.0)).then_some(need)
            }
            Pass::Refreshable => (span_kind == Some(Span::Refreshable)
                && left <= self.profile.refreshable_min)
                .then_some(1),
            Pass::Solid => (span_kind.is_none() && left == 0.0).then_some(1),
        }
    }

    /// What the game said: `spell active`'s lines for an ask, the casting's
    /// answers for a cast.
    pub fn outcome(&mut self, lines: &[String], answers: &[Answer], state: &GameState) {
        if self.asked {
            self.asked = false;
            let (sharing, have) = read_spell_active(lines);
            if !sharing && self.profile.skip_not_sharing {
                self.target = None;
                return;
            }
            if let Some(target) = self.target.as_mut() {
                target.1 = have;
                target.2 = sharing;
            }
            self.pass = Pass::Stackable;
            self.at = 0;
            return;
        }
        let Some((spell, count)) = self.last.take() else {
            return;
        };
        if answers.contains(&Answer::NotHere) || answers.contains(&Answer::Unknown) {
            self.skip_spells.insert(spell);
            self.at += 1;
        } else if answers.contains(&Answer::NoTarget) {
            if let Some((name, _, _)) = &self.target {
                self.skip_targets.insert(name.clone());
            }
        } else if answers.contains(&Answer::Hindered) {
            // Tried again, as ewaggle does.
        } else if answers.contains(&Answer::Cast) {
            self.casts += 1;
            if let Some((name, have, sharing)) = self.target.as_mut() {
                let cast = if is_self(name, state) {
                    CastType::SelfCast
                } else {
                    CastType::Target
                };
                let minutes = state.spell_minutes(spell, cast).unwrap_or(0.0);
                *have.entry(spell).or_insert(0.0) += minutes * f64::from(count);
                if self.pass != Pass::Stackable || !*sharing {
                    self.at += 1;
                }
            }
        } else {
            // No answer read: on to the next rather than again forever.
            self.at += 1;
        }
    }
}

/// Is this name the character?
fn is_self(name: &str, state: &GameState) -> bool {
    name.eq_ignore_ascii_case("self")
        || state
            .character
            .name
            .as_deref()
            .is_some_and(|me| me.eq_ignore_ascii_case(name))
}

/// Minutes left on each of your own spells, by the effects list.
fn own_minutes(state: &GameState) -> BTreeMap<u16, f64> {
    let Some(now) = state.game_time_now() else {
        return BTreeMap::new();
    };
    state
        .effects
        .iter()
        .filter_map(|(id, _)| {
            let number: u16 = id.parse().ok()?;
            let secs = state.effects.remaining(id, now)?;
            Some((number, f64::from(secs) / 60.0))
        })
        .collect()
}

/// `spell active <name>`'s answer (`get_target_info`, `:840-885`): whether
/// they share, and each spell's minutes by the name the line gives.
#[must_use]
pub fn read_spell_active(lines: &[String]) -> (bool, BTreeMap<u16, f64>) {
    let sharing = !lines
        .iter()
        .any(|l| l.contains("has spell sharing disabled"));
    let mut have = BTreeMap::new();
    for line in lines {
        let Some((name, time)) = line.trim().rsplit_once(' ') else {
            continue;
        };
        let name = name.trim_end_matches(['.', ' ']).trim();
        let minutes = if time == "Indefinite" {
            599.0
        } else {
            let parts: Vec<f64> = time.split(':').filter_map(|p| p.parse().ok()).collect();
            match parts.as_slice() {
                [h, m, s] => h * 60.0 + m + s / 60.0,
                _ => continue,
            }
        };
        let name = match name {
            n if n.starts_with("Mage Armor - ") => "Mage Armor",
            n if n.starts_with("Cloak of Shadows - ") => "Cloak of Shadows",
            n => n,
        };
        let number = name
            .parse::<u16>()
            .ok()
            .or_else(|| cena_session::spell_named(name).map(|s| s.number));
        if let Some(number) = number {
            have.insert(number, minutes);
        }
    }
    (sharing, have)
}

/// ewaggle's multicast ceiling (`max_multicast`, `:1709-1737`): a
/// profession's mana-control ranks for the spell's circle, over 25, plus one.
fn max_multicast(state: &GameState, circle: u16) -> u32 {
    let ranks = |kind: SkillKind| {
        state
            .character
            .skills
            .get(kind)
            .and_then(|s| s.ranks)
            .map_or(0, u32::from)
    };
    let (emc, smc, mmc) = (
        ranks(SkillKind::ElementalManaControl),
        ranks(SkillKind::SpiritManaControl),
        ranks(SkillKind::MentalManaControl),
    );
    let profession = state
        .character
        .identity
        .profession
        .as_deref()
        .unwrap_or_default();
    let total = match profession {
        "Wizard" => emc,
        "Cleric" | "Ranger" | "Paladin" => smc,
        "Empath" | "Monk" | "Bard" => match circle {
            1 | 2 => smc + mmc / 2,
            11 => mmc.max(smc) + mmc.min(smc) / 2,
            12 => mmc + smc / 2,
            4 => emc + mmc / 2,
            _ => 0,
        },
        "Sorcerer" | "Warrior" | "Rogue" => match circle {
            4 => emc + smc / 2,
            1 => smc + emc / 2,
            7 => emc.max(smc) + emc.min(smc) / 2,
            _ => 0,
        },
        _ => 0,
    };
    total / 25 + 1
}
