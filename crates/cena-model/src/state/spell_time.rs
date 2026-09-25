//! A spell's duration and cost for this character: the table's Ruby,
//! evaluated against what the model knows (`plan/37` Stage 2). Lich's
//! `Spell#time_per` and `Spell#mana_cost`.
//!
//! The names resolved: `Spells.<list>` (a circle's ranks, matched to the
//! printed circle name without spaces or case), `Skills.<skill>` (by Lich's
//! key or its short form, `emc`, `slreligion`), `Stats.level`,
//! `Society.rank`, and `Spell[N].known?`, `.active?`, `.timeleft`. Anything
//! the model has not been told, or a name it has no word for
//! (`Spellsong.timeleft` among them), leaves the whole answer `None`.

use crate::spells::expr::{self, Name, Value};
use crate::spells::{self, CastType, Duration};
use crate::state::GameState;
use crate::state::character::skills::SkillKind;

impl GameState {
    /// Minutes one cast of spell `number` lasts, cast `cast`. A target
    /// duration the table does not give is the self one, as Lich's
    /// `time_per` falls back.
    #[must_use]
    pub fn spell_minutes(&self, number: u16, cast: CastType) -> Option<f64> {
        let spell = spells::spell(number)?;
        let duration = spell
            .duration(cast)
            .or_else(|| spell.duration(CastType::SelfCast))?;
        match duration {
            Duration::Fixed(text) => text.parse().ok(),
            Duration::Derived(source) => self.evaluate(source),
            Duration::Unknown(_) => None,
        }
    }

    /// Spell `number`'s cost of `kind` (`mana`, `spirit`, `stamina`,
    /// `renew`): the table's number, else its expression evaluated. `Some(0)`
    /// when the spell states no cost of that kind.
    #[must_use]
    pub fn spell_cost(&self, number: u16, kind: &str) -> Option<f64> {
        let spell = spells::spell(number)?;
        match spell.extras.costs.iter().find(|c| c.kind == kind) {
            None => Some(0.0),
            Some(cost) => cost
                .text
                .trim()
                .parse()
                .ok()
                .or_else(|| self.evaluate(&cost.text)),
        }
    }

    /// Evaluate a spell-table expression against this character.
    #[must_use]
    pub fn evaluate(&self, source: &str) -> Option<f64> {
        expr::evaluate(source, &|name| self.resolve(name))
    }

    fn resolve(&self, name: &Name) -> Option<Value> {
        let num = |n: u16| Some(Value::Num(f64::from(n)));
        match name {
            Name::Path(module, method) => match (module.as_str(), method.as_str()) {
                ("Spells", list) => self
                    .character
                    .skills
                    .circles()
                    .find(|(printed, _)| squash(printed) == list.replace('_', ""))
                    .and_then(|(_, ranks)| num(ranks))
                    // A circle the `skills` table did not list has no ranks.
                    .or_else(|| {
                        self.character
                            .skills
                            .known_count()
                            .gt(&0)
                            .then_some(Value::Num(0.0))
                    }),
                ("Skills", short) => {
                    let kind = SkillKind::ALL
                        .into_iter()
                        .find(|k| k.key() == short || lich_short(&k.key()) == short)?;
                    let ranks = self.character.skills.get(kind)?.ranks?;
                    num(ranks)
                }
                ("Stats", "level") => {
                    // `Level 100`, verbatim: the number is the last word.
                    let label = self.character.experience.level.as_deref()?;
                    let level: u16 = label.split_whitespace().last()?.parse().ok()?;
                    num(level)
                }
                ("Society", "rank") => num(u16::from(self.character.standing.society_rank?)),
                _ => None,
            },
            Name::Spell(number, method) => {
                let id = number.to_string();
                match method.as_str() {
                    "known?" => self.known_spells.knows(u32::from(*number)).map(Value::Bool),
                    "active?" => {
                        let now = self.game_time_now()?;
                        self.effects.active(&id, now).map(Value::Bool)
                    }
                    "timeleft" => {
                        let now = self.game_time_now()?;
                        let secs = self.effects.remaining(&id, now).unwrap_or(0);
                        Some(Value::Num(f64::from(secs) / 60.0))
                    }
                    _ => None,
                }
            }
        }
    }
}

/// A printed name without spaces, punctuation or case.
fn squash(printed: &str) -> String {
    printed
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Lich's short form of a skill key (`lib/attributes/skills.rb:52-62`):
/// underscores out, then the lore and mana-control prefixes shortened.
fn lich_short(key: &str) -> String {
    key.replace('_', "")
        .replace("elementallore", "el")
        .replace("spirituallore", "sl")
        .replace("sorcerouslore", "sl")
        .replace("mentallore", "ml")
        .replace("elementalmanacontrol", "emc")
        .replace("spiritmanacontrol", "smc")
        .replace("mentalmanacontrol", "mmc")
}

#[cfg(test)]
mod tests {
    use super::{SkillKind, lich_short, squash};

    #[test]
    fn lichs_short_names_come_from_the_keys() {
        let short: Vec<String> = SkillKind::ALL
            .iter()
            .map(|k| lich_short(&k.key()))
            .collect();
        for name in [
            "emc",
            "smc",
            "mmc",
            "slreligion",
            "slnecromancy",
            "mltelepathy",
            "elair",
        ] {
            assert!(short.iter().any(|s| s == name), "{name}");
        }
        assert_eq!(squash("Minor Spiritual"), "minorspiritual");
        assert_eq!(squash("Major Elemental"), "majorelemental");
    }
}
