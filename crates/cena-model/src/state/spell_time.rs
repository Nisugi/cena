//! A spell's duration and cost for this character: the table's Ruby,
//! evaluated against what the model knows (`plan/37` Stage 2). Lich's
//! `Spell#time_per` and `Spell#mana_cost`.
//!
//! The names resolved: `Spells.<list>` (a circle's ranks, matched to the
//! printed circle name without spaces or case), `Skills.<skill>` (by Lich's
//! key or its short form, `emc`, `slreligion`), `Stats.level`,
//! `Society.rank`, `Spellsong.timeleft`, and `Spell[N].known?`, `.active?`,
//! `.timeleft`. Anything the model has not been told, or a name it has no
//! word for, leaves the whole answer `None`.
//!
//! # A bard's songs
//!
//! Lich's `Spellsong.timeleft` and `renew_cost` (`attributes/spellsong.rb:26`,
//! `:60`) live here rather than beside the rest of the song arithmetic
//! (`character/spellsong.rs`), because both read the effects list and the
//! spell table, which a character alone does not hold.
//!
//! **`timeleft` reads the game, not a timestamp.** Lich keeps `@@renewed`, a
//! process-global a script sets when it renews, and works the time left
//! forward from it; its `sync` (`spellsong.rb:8-12`) re-aligns that clock to
//! the time left on the first active bard spell in the effects list. Here the
//! effects list is the answer itself: the songs are renewed together, so the
//! time left on an active song is the time left on the cycle. What Lich
//! caches, and a script has to remember to set, the model reads off the
//! game's own dialog each time.

use crate::spells::expr::{self, Name, Value};

/// The songs Lich renews and prices together (`spellsong.rb:63`).
pub const SONGS: [u16; 9] = [1003, 1006, 1009, 1010, 1012, 1014, 1018, 1019, 1025];
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

    /// Minutes until a bard's songs lapse: Lich's `Spellsong.timeleft`
    /// (`spellsong.rb:26-29`). `0.0` for any other profession, as Lich's first
    /// line has it. For a bard, the least time left on an active bard spell
    /// (`10xx`, Lich's `sync` pattern) in the effects list; `None` while no
    /// song is up, or the profession or the clock has not been told.
    #[must_use]
    pub fn spellsong_timeleft(&self) -> Option<f64> {
        let profession = self.character.identity.profession.as_deref()?;
        if !profession.eq_ignore_ascii_case("bard") {
            return Some(0.0);
        }
        let now = self.game_time_now()?;
        self.effects
            .iter()
            .filter(|(id, _)| is_song(id))
            .filter(|(id, _)| self.effects.active(id, now) == Some(true))
            .filter_map(|(id, _)| self.effects.remaining(id, now))
            .min()
            .map(|secs| f64::from(secs) / 60.0)
    }

    /// Mana to renew every song that is up: Lich's `Spellsong.renew_cost`
    /// (`spellsong.rb:60-73`), each active song's `renew` cost from the spell
    /// table, summed. `0.0` with none up. `None` when the clock has not been
    /// told, or an active song's cost cannot be worked out.
    ///
    /// Lich's own *"fixme: multi-spell penalty?"* stands: the sum is the
    /// sum, as Lich's is.
    #[must_use]
    pub fn spellsong_renew_cost(&self) -> Option<f64> {
        let now = self.game_time_now()?;
        let mut total = 0.0;
        for song in SONGS {
            if self.effects.active(&song.to_string(), now) == Some(true) {
                total += self.spell_cost(song, "renew")?;
            }
        }
        Some(total)
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
                    let ranks = self.character.skills.ranks(kind)?;
                    num(ranks)
                }
                ("Stats", "level") => {
                    // `Level 100`, verbatim: the number is the last word.
                    let label = self.character.experience.level.as_deref()?;
                    let level: u16 = label.split_whitespace().last()?.parse().ok()?;
                    num(level)
                }
                ("Society", "rank") => num(u16::from(self.character.standing.society_rank?)),
                ("Spellsong", "timeleft") => self.spellsong_timeleft().map(Value::Num),
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

/// A bard spell's id, as Lich's `sync` finds one: `10` and two digits.
fn is_song(id: &str) -> bool {
    id.len() == 4 && id.starts_with("10") && id.bytes().all(|b| b.is_ascii_digit())
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
    use super::{SONGS, SkillKind, is_song, lich_short, squash};
    use crate::effects::Effect;
    use crate::state::GameState;
    use cena_protocol::frame::Frame;

    /// A bard at game second 1000 with these songs up, `secs` left on each.
    fn bard(songs: &[(u16, u32)]) -> GameState {
        let mut state = GameState::default();
        state.apply(&Frame::Prompt {
            time: "1000".into(),
            text: ">".into(),
        });
        state.character.identity.profession = Some("Bard".to_owned());
        for (song, secs) in songs {
            state.effects.insert_lasting(
                song.to_string(),
                Effect {
                    category: "Active Spells".to_owned(),
                    text: format!("song {song}"),
                    ends_at: None,
                    percent: 100,
                },
                *secs,
                Some(1000),
            );
        }
        state
    }

    /// `spellsong.rb:26-29` and `sync`, `:8-12`: the time left on the songs
    /// is the time left on an active bard spell; the least of them.
    #[test]
    fn a_bards_song_time_is_read_off_the_effects() {
        let state = bard(&[(1003, 600), (1010, 540)]);
        assert_eq!(state.spellsong_timeleft(), Some(9.0));
        assert_eq!(bard(&[]).spellsong_timeleft(), None, "no song up");
    }

    /// `spellsong.rb:27`: not a bard, no song time.
    #[test]
    fn anyone_else_has_no_song_time() {
        let mut state = bard(&[(1003, 600)]);
        state.character.identity.profession = Some("Wizard".to_owned());
        assert_eq!(state.spellsong_timeleft(), Some(0.0));
        state.character.identity.profession = None;
        assert_eq!(state.spellsong_timeleft(), None, "not yet told");
    }

    /// `spellsong.rb:60-73`: each active song's renew cost, summed; none
    /// up, nothing to pay.
    #[test]
    fn the_renew_cost_is_the_active_songs_summed() {
        assert_eq!(bard(&[]).spellsong_renew_cost(), Some(0.0));
        // Fortitude Song renews for 1, Sonic Weapon Song for 4 (the table's
        // `renew_cost`, `data/spells.tsv`).
        assert_eq!(bard(&[(1003, 600)]).spellsong_renew_cost(), Some(1.0));
        assert_eq!(
            bard(&[(1003, 600), (1012, 600)]).spellsong_renew_cost(),
            Some(5.0)
        );
        // Song of Valor's is `3+((Spells.bard-10)/10)`: without the Bard
        // circle's ranks, the sum is not known either.
        assert_eq!(
            bard(&[(1003, 600), (1010, 600)]).spellsong_renew_cost(),
            None
        );
    }

    /// The bard songs' durations are written as `Spellsong.timeleft`
    /// (`data/spells.tsv`), which evaluates now.
    #[test]
    fn a_songs_duration_is_the_song_time() {
        let state = bard(&[(1003, 600)]);
        assert_eq!(state.evaluate("Spellsong.timeleft"), Some(10.0));
    }

    #[test]
    fn a_song_is_a_bard_spell_id() {
        assert!(SONGS.iter().all(|song| is_song(&song.to_string())));
        assert!(!is_song("515") && !is_song("1101") && !is_song("10a3"));
    }

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
