//! The spell steps: bigshot's `cmd_spell` and the handlers that cast
//! (`bigshot.lic:5839-5910`), with Lich's stance for a spell that wants one.

use std::collections::VecDeque;

use cena_session::{GameState, SpellMark, Stance};

use super::super::engine::Hunt;
use super::super::maintain::ACTIVE_SPELLS;
use super::tables::{PLANTS, SELF_CAST, SHORT_BUFFS, UNAIMED};
use super::{Line, line, up_in};
use crate::cast::{self, Casting, NotReady, Verb};

impl Hunt {
    /// `resonance N N ...`: one of the spells, never the last one twice
    /// running, chosen at random as bigshot chooses (`cmd_resonance_bolt`,
    /// `bigshot.lic:5917-5929`), and incanted at the game's target.
    pub(super) fn resonance(
        &mut self,
        spells: &[u16],
        target: i64,
        state: &GameState,
        now: Option<u32>,
    ) -> Line {
        let options: Vec<u16> = spells
            .iter()
            .enumerate()
            .filter(|(at, n)| !spells[..*at].contains(n) && Some(**n) != self.repeats.resonance)
            .map(|(_, n)| *n)
            .collect();
        let Ok(len) = u64::try_from(options.len()) else {
            return Line::Skip;
        };
        if len == 0 {
            return Line::Skip;
        }
        let at = usize::try_from(self.roll(now) % len).unwrap_or(0);
        let Some(pick) = options.get(at).copied() else {
            return Line::Skip;
        };
        self.repeats.resonance = Some(pick);
        let mut spell = Spell::bare(pick);
        spell.incanted = true;
        spell.cast_step(target, state)
    }
}

/// `resonance N N ...`: the spells (`bigshot.lic:4063`).
pub(super) fn resonance(send: &str) -> Option<Vec<u16>> {
    let mut words = send.split_whitespace();
    if !words.next()?.eq_ignore_ascii_case("resonance") {
        return None;
    }
    let spells: Vec<u16> = words.map_while(|w| w.parse().ok()).collect();
    (!spells.is_empty()).then_some(spells)
}

/// The buff a step names before it (`celerity fire`), by its word
/// (`bigshot.lic:4015-4046`).
fn buff_before(word: &str) -> Option<u16> {
    match word {
        "celerity" | "haste" | "506" => Some(506),
        "slayer" | "240" => Some(240),
        "tonis" | "1035" => Some(1035),
        _ => None,
    }
}

/// `celerity <step>` and its kin: the buff's lines, then the step's.
pub(super) fn buff_first(first: &str, rest: &str, target: i64, state: &GameState) -> Option<Line> {
    let number = buff_before(first)?;
    if rest.is_empty() {
        return None;
    }
    let mut lines = buffed(number, target, state);
    Some(match line(rest, target, state) {
        Line::Send(mut step) => {
            lines.append(&mut step);
            Line::Send(lines)
        }
        step if lines.is_empty() => step,
        _ => Line::Send(lines),
    })
}

/// The lines that put the buff up before the step, when it is down or has
/// three seconds or less (Lich's `timeleft <= 0.05`, in minutes); none
/// before any spell list has been seen, as maintain waits for one.
/// Celerity is not cast while it is up at all (`cmd_spell`, `:5857`), and
/// Spirit Slayer not while it is cooling (`:4028`).
fn buffed(number: u16, target: i64, state: &GameState) -> VecDeque<String> {
    let Some(now) = state.game_time_now() else {
        return VecDeque::new();
    };
    let effects = &state.effects;
    let id = number.to_string();
    let up = match effects.active(&id, now) {
        Some(up) => up,
        None if effects.saw_category(ACTIVE_SPELLS) || effects.saw_category("Buffs") => false,
        None => return VecDeque::new(),
    };
    let lapsing = !up || effects.remaining(&id, now).is_some_and(|left| left <= 3);
    let slayer_cooling = number == 240
        && cena_session::spells::spell(240)
            .is_some_and(|spell| up_in(state, "Cooldowns", &spell.name));
    if !lapsing || (number == 506 && up) || slayer_cooling {
        return VecDeque::new();
    }
    match Spell::bare(number).cast(target, state) {
        Line::Send(lines) => lines,
        _ => VecDeque::new(),
    }
}

/// A spell step: its number, how it is sent, and what came after.
pub(super) struct Spell {
    number: u16,
    verb: Verb,
    /// Words beyond the verb (`open`, an element): sent as written, since
    /// the casting step does not take them.
    extra: String,
    /// Cast at the creature unless the spell is a self-cast one.
    aimed: bool,
    /// Written `incant N`: after a stance spell, back to the stance before.
    incanted: bool,
}

impl Spell {
    /// A spell cast with no target (`Spell[N].cast`).
    pub(super) const fn bare(number: u16) -> Self {
        Self {
            number,
            verb: Verb::Cast,
            extra: String::new(),
            aimed: false,
            incanted: false,
        }
    }

    /// A spell cast at the creature (`force_cast("#id")`).
    pub(super) const fn at(number: u16) -> Self {
        Self {
            number,
            verb: Verb::Cast,
            extra: String::new(),
            aimed: true,
            incanted: false,
        }
    }

    /// A spell step, as `cmd_spell` casts it (`bigshot.lic:5839-5910`): its
    /// early returns, and a spell it cannot afford said so, for the rest
    /// bigshot takes on it. The other handlers cast through [`Self::cast`],
    /// which only skips.
    pub(super) fn cast_step(&self, target: i64, state: &GameState) -> Line {
        if self.refused(state) || self.marked(target, state) {
            return Line::Skip;
        }
        // Mana Leech's recovery (597) costs 5 mana more (`:5849`).
        let penalty = state
            .game_time_now()
            .is_some_and(|now| state.effects.active("597", now) == Some(true));
        match cast::ready(state, self.number, 1, if penalty { 5 } else { 0 }) {
            Err(NotReady::Mana(..) | NotReady::Spirit | NotReady::Stamina) => {
                Line::Unaffordable(self.number)
            }
            _ => self.cast(target, state),
        }
    }

    /// `cmd_spell`'s returns for one spell at a time (`:5854-5864`):
    /// Celerity while it is up, five cooldowns, the short buffs' shared
    /// cooldowns, and Camouflage while hidden.
    fn refused(&self, state: &GameState) -> bool {
        let Some(now) = state.game_time_now() else {
            return false;
        };
        let cooling = |name: &str| up_in(state, "Cooldowns", name);
        let named =
            || cena_session::spells::spell(self.number).is_some_and(|spell| cooling(&spell.name));
        match self.number {
            506 => state.effects.active("506", now) == Some(true),
            9605 => cooling("Surge of Strength"),
            9625 => cooling("Burst of Swiftness"),
            335 => state.effects.active_in("Cooldowns", "335", now) == Some(true),
            720 => cooling("Implosion"),
            608 => state.status.known().hidden() == Some(true),
            n if SHORT_BUFFS.contains(&n) => named(),
            _ => false,
        }
    }

    /// Corrupt Essence (703) and Aura of the Arkati (1614) are not cast at a
    /// creature they already hold (`cmd_spell`, `:5859-5860`).
    fn marked(&self, target: i64, state: &GameState) -> bool {
        let mark = match self.number {
            703 => SpellMark::BloodRedHaze,
            1614 => SpellMark::Rebuked,
            _ => return false,
        };
        state
            .creatures()
            .get(target)
            .is_some_and(|creature| creature.marked(mark))
    }

    /// The lines, or a skip when the spell is not known or not affordable
    /// (`cmd_spell`'s early returns, Lich's `check_energy`).
    pub(super) fn cast(&self, target: i64, state: &GameState) -> Line {
        match cast::ready(state, self.number, 1, 0) {
            Err(NotReady::NotKnown | NotReady::Mana(..) | NotReady::Spirit | NotReady::Stamina) => {
                return Line::Skip;
            }
            Ok(()) | Err(NotReady::CastRoundtime(_)) => {}
        }
        if !self.extra.is_empty() {
            let verb = match self.verb {
                Verb::Cast => String::new(),
                other => format!(" {}", other.word()),
            };
            let line = format!("incant {}{verb} {}", self.number, self.extra);
            return Line::Send(self.stanced(VecDeque::from([line]), state));
        }
        // Celerity and 902 go untargeted whatever the step says (`cmd_spell`,
        // `bigshot.lic:5885-5886`).
        let aimed =
            self.aimed && !SELF_CAST.contains(&self.number) && !UNAIMED.contains(&self.number);
        let casting = Casting {
            spell: self.number,
            target: aimed.then(|| format!("#{target}")),
            count: None,
            verb: self.verb,
        };
        Line::Send(self.stanced(casting.lines(state).into(), state))
    }

    /// Lich's stance for a spell its table marks as wanting one
    /// (`spell.rb:762-764`, `:786-787`): offensive for the cast, then back.
    /// Back is the stance before for a step written `incant N` (bigshot's
    /// own `after_stance`, `bigshot.lic:5899-5904`), and otherwise the
    /// safest, `guarded` as a cast roundtime runs (`spell.rb:814-827`,
    /// `stance.rb:84-86`). The hunting stance is taken again at the next
    /// step that wants it.
    fn stanced(&self, mut lines: VecDeque<String>, state: &GameState) -> VecDeque<String> {
        if !cena_session::spells::spell(self.number).is_some_and(|spell| spell.extras.stance) {
            return lines;
        }
        let before = state.character.stance_typed();
        if before != Some(Stance::Offensive) {
            // After a `release`, as Lich releases first (`spell.rb:717-724`).
            let at = usize::from(lines.front().is_some_and(|line| line == "release"));
            lines.insert(at, "stance offensive".to_owned());
        }
        let after = if self.incanted {
            before.filter(|stance| *stance != Stance::Offensive)
        } else {
            Some(Stance::Guarded)
        };
        if let Some(after) = after {
            lines.push_back(format!("stance {}", after.as_str()));
        }
        lines
    }
}

/// `incant 611`, `611 evoke`: bigshot's spell pattern (`:4062`).
pub(super) fn spell_step(first: &str, words: &[&str]) -> Option<Spell> {
    let (number, after) = if first == "incant" {
        (
            words.get(1)?.parse().ok()?,
            words.get(2..).unwrap_or_default(),
        )
    } else {
        (first.parse().ok()?, words.get(1..).unwrap_or_default())
    };
    let mut verb = Verb::Cast;
    let mut extra = Vec::new();
    for word in after {
        match Verb::parse(word) {
            Some(v) if v != Verb::Cast || word.eq_ignore_ascii_case("cast") => verb = v,
            _ => extra.push(*word),
        }
    }
    Some(Spell {
        number,
        verb,
        extra: extra.join(" "),
        aimed: true,
        incanted: first == "incant",
    })
}

/// Tangleweed at the creature, unless a plant is here already.
pub(super) fn weed(evoked: bool, target: i64, state: &GameState) -> Line {
    let planted = state.room.objects.iter().any(|object| {
        let text = object.text.to_ascii_lowercase();
        let words: Vec<&str> = text.split(|c: char| !c.is_ascii_alphanumeric()).collect();
        PLANTS.iter().any(|plant| {
            let parts: Vec<&str> = plant.split(' ').collect();
            words
                .windows(parts.len())
                .any(|run| run == parts.as_slice())
        })
    });
    if planted {
        return Line::Skip;
    }
    let mut spell = Spell::at(610);
    if evoked {
        spell.verb = Verb::Evoke;
    }
    spell.cast(target, state)
}

/// `caststop N [extra]`: the spell at the creature, then `stop N`.
pub(super) fn caststop(words: &[&str], target: i64, state: &GameState) -> Line {
    let Some(number) = words.get(1).and_then(|n| n.parse().ok()) else {
        return Line::Skip;
    };
    match Spell::at(number).cast(target, state) {
        Line::Send(mut lines) => {
            lines.push_back(format!("stop {number}"));
            Line::Send(lines)
        }
        other => other,
    }
}

/// Spells whose cost does not send bigshot to rest (`cmd_spell`, `:5875`).
pub(super) const NO_REST_SPELLS: &[u16] = &[9605, 506, 902, 411];

/// Soothe (1201) before a command while a spell that calms the character
/// is on it (`cmd`, `bigshot.lic:4003-4010`), when 1201 is known and
/// affordable.
pub(super) fn soothe(state: &GameState) -> Option<String> {
    let now = state.game_time_now()?;
    let calmed = [201, 216, 1015, 1016, 1108, 1120]
        .iter()
        .any(|spell| state.effects.active(&spell.to_string(), now) == Some(true));
    let ready =
        state.known_spells.knows(1201) == Some(true) && cast::ready(state, 1201, 1, 0).is_ok();
    (calmed && ready).then(|| "incant 1201".to_owned())
}
