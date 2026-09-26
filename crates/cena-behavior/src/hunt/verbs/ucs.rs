//! Multi-Strike and unarmed combat: bigshot's `cmd_mstrike` and
//! `cmd_unarmed`, with the settings of its UAC and Mstrike tabs
//! ([`crate::hunt::profile::Unarmed`], [`crate::hunt::profile::Mstrike`]).

use std::collections::VecDeque;

use cena_session::{GameState, PositionTier, SkillKind, gameobj};

use super::super::engine::Hunt;
use super::gated::{castable, one};
use super::{Line, up_in};

/// The spells bigshot casts before a Multi-Strike for a Paladin or an Empath
/// (`mstrike_spell_check`, `bigshot.lic:6142-6168`): Rejuvenation (1607)
/// when it would bring stamina to the strike's, and Adrenal Surge (1107),
/// no oftener than every 301 s.
const SLB_STEPS: &[u16] = &[
    1, 3, 6, 10, 15, 21, 28, 36, 45, 55, 66, 78, 91, 105, 120, 136, 153, 171, 190,
];

impl Hunt {
    /// `mstrike [attack]` (`cmd_mstrike`, `bigshot.lic:6175-6211`): the spells
    /// first, then the strike when bigshot would take it -- 30 ranks of
    /// Multi-Opponent Combat, no nest here, not cooling (or cooling with the
    /// stamina the profile allows), `quickstrike 1` first when set and
    /// affordable, unfocused when `mob` creatures are here; with 5 to 29
    /// ranks, unfocused only. A skill never read is 0 ranks, as Lich's is.
    pub(super) fn mstrike(
        &mut self,
        command: &str,
        target: i64,
        state: &GameState,
        now: Option<u32>,
    ) -> (VecDeque<String>, Option<String>) {
        let command = command.trim();
        let spells = self.mstrike_spells(state, now);
        if up_in(state, "Debuffs", "Overexerted") {
            return (spells, None);
        }
        let skills = &state.character.skills;
        let moc = skills.ranks(SkillKind::MultiOpponentCombat).unwrap_or(0);
        // Every hostile creature, named by the targets or not
        // (`bs_hostile_creatures`): a wasp nest is no `any` target.
        let nest = self.could_fight(state).any(|creature| {
            creature
                .noun
                .as_deref()
                .is_some_and(|noun| noun.contains("nest"))
        });
        let here = u32::try_from(self.could_fight(state).count()).unwrap_or(u32::MAX);
        let rules = &self.profile.mstrike;
        let stamina = state.stamina();
        let points = stamina.and_then(|v| v.current).unwrap_or(0);
        let most = stamina.and_then(|v| v.max).unwrap_or(0);
        let needs = |set: Option<u32>| set.map_or(most, |n| i32::try_from(n).unwrap_or(i32::MAX));
        let may = !up_in(state, "Cooldowns", "Multi-Strike")
            || (rules.cooldown && points >= needs(rules.stamina_cooldown));
        let quick = if rules.quickstrike && points >= needs(rules.stamina_quickstrike) {
            "quickstrike 1 "
        } else {
            ""
        };
        let mob = here >= rules.mob;
        let strike = if nest || !may {
            None
        } else if moc >= 30 {
            Some(if mob {
                format!("{quick}{command}")
            } else {
                format!("{quick}{command} #{target}")
            })
        } else if moc >= 5 && mob {
            Some(format!("{quick}{command}"))
        } else {
            None
        };
        (spells, strike)
    }

    fn mstrike_spells(&mut self, state: &GameState, now: Option<u32>) -> VecDeque<String> {
        let mut lines = VecDeque::new();
        let paladin_or_empath = state
            .character
            .identity
            .profession
            .as_deref()
            .is_some_and(|p| p.eq_ignore_ascii_case("paladin") || p.eq_ignore_ascii_case("empath"));
        let Some(now) = now.filter(|_| paladin_or_empath) else {
            return lines;
        };
        let slb = state
            .character
            .skills
            .ranks(SkillKind::SpiritualLoreBlessings)
            .unwrap_or(0);
        let stamina = state.stamina();
        let points = stamina.and_then(|v| v.current).unwrap_or(0);
        let most = stamina.and_then(|v| v.max).unwrap_or(0);
        let needed = self
            .profile
            .mstrike
            .stamina_cooldown
            .map_or(most, |n| i32::try_from(n).unwrap_or(i32::MAX));
        let active = |spell: u16| state.effects.active(&spell.to_string(), now) == Some(true);
        let known = |spell: u16| state.known_spells.knows(u32::from(spell)) == Some(true);
        let ready = |spell: u16| known(spell) && castable(state, spell);
        let bonus = i32::try_from(SLB_STEPS.iter().filter(|n| slb >= **n).count()).unwrap_or(0) * 3;
        if ready(1607) && !active(1607) && points < needed && points + 15 + bonus >= needed {
            lines.push_back("incant 1607".to_owned());
        }
        let surge_ok = self.follow.adrenal_until.is_none_or(|until| now >= until);
        if ready(1107) && !active(9010) && surge_ok {
            let popped = active(9699);
            let lift = if slb >= 35 { 50 } else { 0 };
            if popped || (slb >= 65 && most >= needed) || points + lift >= needed {
                lines.push_back("incant 1107".to_owned());
                self.follow.adrenal_until = Some(now + 301);
            }
        }
        lines
    }

    /// `unarmed <attack> [aim]` (`cmd_unarmed`, `bigshot.lic:5455-5551`):
    /// smite first when the profile says and the creature is noncorporeal at
    /// tier 3; a Multi-Strike with the tier 3 attack unless the profile
    /// forbids it; otherwise the tier 3 attack at tier 3, the follow-up the
    /// game offered, or the step's attack, at the aim named or the profile's
    /// next. The tier and the follow-up are the model's, per creature.
    #[expect(
        clippy::redundant_closure_for_method_calls,
        reason = "the model's UcsAttack is not re-exported for behaviors, so its path cannot be named here"
    )]
    pub(super) fn unarmed(
        &mut self,
        rest: &str,
        target: i64,
        state: &GameState,
        now: Option<u32>,
    ) -> Line {
        let mut words = rest.split_whitespace();
        let Some(attack) = words.next() else {
            return Line::Skip;
        };
        let named_aim = words.next();
        let Some(creature) = state.creatures().get(target) else {
            return Line::Skip;
        };
        let tier3 = creature.ucs_position(now) == Some(PositionTier::Excellent);
        let followup = creature.ucs_tierup(now).map(|a| a.as_str());
        let rules = self.profile.unarmed.clone();
        let noncorporeal = creature
            .noun
            .as_deref()
            .is_some_and(|noun| gameobj::classify(noun, &creature.name).is("noncorporeal"));
        let smite = rules.smite
            && tier3
            && noncorporeal
            && !creature.smote(now)
            && state.known_spells.knows(9821) == Some(true);
        if smite {
            return one(format!("smite #{target}"));
        }
        if !rules.no_mstrike {
            let (mut lines, strike) =
                self.mstrike(&format!("mstrike {}", rules.tier3), target, state, now);
            if let Some(strike) = strike {
                lines.push_back(strike);
                return Line::Send(lines);
            }
        }
        let word = match (tier3, followup) {
            (true, None) => rules.tier3.as_str(),
            (_, Some(next)) => next,
            (false, None) => attack,
        };
        let aim = named_aim
            .map(str::to_owned)
            .or_else(|| rules.aim.get(self.aiming.at()).cloned())
            .unwrap_or_default();
        one(format!("{word} #{target} {aim}").trim_end().to_owned())
    }
}
