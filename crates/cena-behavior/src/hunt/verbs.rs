//! bigshot's routine verbs (`bigshot.lic:3995-4160`): what a routine step
//! puts on the wire.
//!
//! bigshot hands each verb to a handler, and the handler sends a game
//! command aimed at the creature by id: `coupdegrace` is `cman coupdegrace
//! #42` (`cmd_cmans`, `:5083`), `fire` is `fire #42` (`cmd_ranged`,
//! `:6391`), and `kweed` is Tangleweed (610) evoked at the creature
//! (`cmd_weed`, `:5750-5765`). Sent as written, `kweed` and `coupdegrace`
//! are words the game does not take, and `shout`, `cry` and `disarm` are
//! other verbs altogether. So a step whose first word is a bigshot verb is
//! turned into what bigshot would send; any other step is a game command,
//! sent as written (`store weapon`, `ready 2weapon` and `weapon volley` in
//! the author's own sequence, `plan/30` §5).
//!
//! | Step | Sent | bigshot |
//! |---|---|---|
//! | `incant N [verb]`, `N [verb]` | `prepare N` then `cast #id`; a self-cast spell `incant N` | `cmd_spell`, Lich's `Spell#cast`, via [`crate::cast`] |
//! | `kweed`, `weed` | Tangleweed evoked, or cast, at the target; not while a plant is already here | `cmd_weed` |
//! | a combat maneuver, `bearhug` | `cman <step> #id`; not while it is cooling | `cmd_cmans`, `cmd_rogue_cmans`, `cmd_bearhug` |
//! | a weapon technique | `weapon <step> #id`; not while it is cooling | `cmd_weapons`, `cmd_assault` |
//! | `shield bash` and the other shield moves | `shield <move> #id` | `cmd_shields` |
//! | `chastise`, `excoriate` | `feat <step> #id` | `cmd_feats` |
//! | `shout`, `yowlp`, `holler`, `bellow`, `growl`, `cry` | `warcry <step>`, at the target for bellow, growl and cry alone; not short of stamina | `cmd_warrior_shouts` |
//! | `fire` | `fire #id` | `cmd_ranged` |
//! | `jewel <mnemonic>` | the jewel's activation (`src/gemstone/jewel.rs`); not while it is cooling, nor for a mnemonic bigshot does not know | `cmd_jewel` |
//! | `phase` | Phase (704) at the target | `cmd_phase` |
//! | `caststop N` | N at the target, then `stop N` | `cmd_caststop` |
//! | `celerity`, `haste`, `506`; `slayer`, `240`; `tonis`, `1035`, each before a step | the buff first when it is down or has three seconds or less (Celerity only when down), then the step | `cmd`, `:4014-4046` |
//! | `resonance N N ...` | one of the spells, at random but never the last one twice running, incanted at the game's target | `cmd_resonance_bolt` |
//! | `throw`, `smite`, `sacrifice`, `burst`, `surge`, `rapid`, `leech`, `stomp`, `curse`, `store`, `stance`, `depress`, `unravel`, `efury`, `tether` | gated as bigshot gates them, holding or reading the answer where it does | [`gated`], [`super::follow`] |
//! | `wait N`, `sleep N`, `berserk`, `hide [N]`, `dhurl`, `dislodge`, `assume` | the same, with the hunt's own settings: the wander stance, the ambush list, the parts an arrow is lodged in | [`gated`] |
//!
//! Before any of it, as bigshot's `cmd` does (`:3974-4001`): `kick` is
//! `punch` while the character is rooted, and the word `target` is the
//! creature's `#id`. `ambush`, `wand` and `script` are the engine's own
//! ([`super::aim`], [`super::wand`], the importer), and so are `eachtarget`
//! and `force` ([`super::repeat`]). Every verb bigshot dispatches is sent;
//! a `jewel` mnemonic bigshot does not know is skipped, and said once.

mod gated;
mod spell;
mod tables;
mod ucs;

use std::collections::VecDeque;

use cena_session::{GameState, PsmCategory};

use self::gated::coup_refused;
use self::spell::{
    NO_REST_SPELLS, Spell, buff_first, caststop, resonance, soothe, spell_step, weed,
};
use self::tables::{ASSAULTS, CMANS, WARCRIES, WEAPONS};
use super::engine::Hunt;
use super::follow::{ASSAULT_ENDS, BEARHUG_ENDS, End, Hold, Next};
use super::said::{Said, Why};
use crate::gemstone::jewel;

/// What a step sends.
#[derive(Debug)]
pub(super) enum Line {
    /// These lines, in order: the first now, the rest right after it.
    Send(VecDeque<String>),
    /// These lines, then a hold or an answer ([`super::follow`]).
    Then(VecDeque<String>, Next),
    /// Say this instead of sending.
    Said(Said),
    /// Not now: bigshot's handler would return without sending.
    Skip,
    /// A routine spell the character cannot afford (`cmd_spell`'s rest).
    Unaffordable(u16),
    /// A bigshot verb Hydra does not send yet: skipped, and said once.
    Unported(&'static str),
}

/// What the engine does with a step whose guards held.
pub(super) enum Go {
    /// Send this line.
    Send(String),
    /// Say this instead of sending.
    Said(Said),
    /// Skip the step.
    Skip,
}

impl Hunt {
    /// What the routine step `send` (recorded as `key`) does against
    /// creature `target`: its first line, with the rest kept as followups.
    pub(super) fn verb_step(
        &mut self,
        send: &str,
        key: &str,
        target: i64,
        state: &GameState,
        now: Option<u32>,
    ) -> Go {
        let send = self.before_dispatch(send, target);
        let words: Vec<&str> = send.split_whitespace().collect();
        let first = words
            .first()
            .map(|w| w.to_ascii_lowercase())
            .unwrap_or_default();
        let rest = words.get(1..).unwrap_or_default().join(" ");
        let line = if let Some(line) = self.own_verb(&first, &rest, target, state, now) {
            line
        } else if let Some(spells) = resonance(&send) {
            self.resonance(&spells, target, state, now)
        } else {
            line(&send, target, state)
        };
        let line = match (line, soothe(state)) {
            (Line::Send(mut lines), Some(first)) => {
                lines.push_front(first);
                Line::Send(lines)
            }
            (Line::Then(mut lines, next), Some(first)) => {
                lines.push_front(first);
                Line::Then(lines, next)
            }
            (line, _) => line,
        };
        match line {
            Line::Send(mut lines) => match lines.pop_front() {
                None => Go::Skip,
                Some(first) => {
                    self.followups = lines;
                    Go::Send(first)
                }
            },
            Line::Then(mut lines, next) => {
                self.follow_with(next, now);
                match lines.pop_front() {
                    None => {
                        self.used.record(key, Some(target), now);
                        Go::Said(Said::Wait(1))
                    }
                    Some(first) => {
                        self.followups = lines;
                        Go::Send(first)
                    }
                }
            }
            Line::Said(said) => Go::Said(said),
            Line::Skip => Go::Skip,
            Line::Unaffordable(spell) => {
                if self.profile.rest.when.unaffordable && !NO_REST_SPELLS.contains(&spell) {
                    self.must_rest = Some(Why::Mana);
                }
                Go::Skip
            }
            Line::Unported(verb) => {
                if self.told_unported.insert(verb) {
                    self.notes.push(format!(
                        "`{verb}` is a bigshot verb Hydra does not send yet; skipped"
                    ));
                }
                Go::Skip
            }
        }
    }
}

impl Hunt {
    /// What bigshot does to every command before its handler (`cmd`,
    /// `bigshot.lic:3974-4001`): `kick` is `punch` while rooted, and the
    /// word `target` is the creature's `#id`.
    fn before_dispatch(&self, send: &str, target: i64) -> String {
        send.split(' ')
            .map(|word| match word {
                "kick" if self.follow.rooted => "punch".to_owned(),
                "target" => format!("#{target}"),
                other => other.to_owned(),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// What `send`, a routine step with its guards gone, puts on the wire
/// against creature `target`.
pub(super) fn line(send: &str, target: i64, state: &GameState) -> Line {
    let words: Vec<&str> = send.split_whitespace().collect();
    let Some(first) = words.first().map(|w| w.to_ascii_lowercase()) else {
        return Line::Skip;
    };
    let rest = words.get(1..).unwrap_or_default().join(" ");
    let at = format!("#{target}");
    let one = |text: String| Line::Send(VecDeque::from([text]));
    // `celerity fire` and its kin carry a step after them; bare, 506 is a spell.
    if let Some(buffed) = buff_first(&first, &rest, target, state) {
        return buffed;
    }
    if let Some(spell) = spell_step(&first, &words) {
        return spell.cast_step(target, state);
    }
    match first.as_str() {
        "kweed" | "weed" => return weed(first == "kweed", target, state),
        "fire" => return one(format!("fire {at}")),
        "throw" => return gated::throw(target, state),
        "smite" => return gated::smite(target, state),
        "sacrifice" => return gated::sacrifice(target, state),
        "burst" | "surge" => return gated::burst_or_surge(&first, state),
        "chastise" | "excoriate" => return gated::feat(&first, send, target, state),
        "depress" => return gated::depress(state),
        "rapid" => return gated::rapid(rest.starts_with("ignore"), target, state),
        "leech" => return gated::leech(target, state),
        "stomp" => return gated::stomp(state),
        "phase" => return Spell::at(704).cast(target, state),
        "unravel" | "barddispel" => return gated::unravel(&rest, target, state),
        "efury" => return gated::efury(&rest, target, state),
        "tether" => return gated::tether(target, state),
        "curse" if !rest.is_empty() => return gated::curse(&rest, target, state),
        "store" => return gated::store(&rest, send, state),
        "stance" if !rest.is_empty() => return gated::stance(&rest, send, state),
        "caststop" => return caststop(&words, target, state),
        "jewel" => {
            return match jewel::activate(&rest) {
                Some((_, name)) if cooling(state, name) => Line::Skip,
                Some((line, _)) => one(line),
                None => Line::Unported("jewel with a mnemonic bigshot does not know"),
            };
        }
        "shield" => return gated::shield(&rest, send, target, state),
        "wield" if !rest.is_empty() => return gated::wield(&rest, state),
        "briar" if !rest.is_empty() => return gated::briar(&rest, state),
        _ => {}
    }
    if let Some((_, name)) = CMANS.iter().find(|(w, _)| *w == first) {
        let unavailable = gated::unavailable(state, PsmCategory::CombatManeuver, &first);
        if cooling(state, name)
            || unavailable
            || (first == "coupdegrace" && coup_refused(target, state))
        {
            return Line::Skip;
        }
        if first == "bearhug" {
            let hold = Hold::new(17, target, End::Heard(BEARHUG_ENDS));
            return Line::Then([format!("cman {send} {at}")].into(), Next::Hold(hold));
        }
        return one(format!("cman {send} {at}"));
    }
    if let Some((_, name)) = WEAPONS.iter().find(|(w, _)| *w == first) {
        if cooling(state, name) || gated::unavailable(state, PsmCategory::Weapon, &first) {
            return Line::Skip;
        }
        // An assault runs for rounds; bigshot waits it out. Not ported: its
        // `swap` when Barrage refuses the attack type, and Fury with the
        // tier 3 attack.
        if ASSAULTS.contains(&first.as_str()) {
            let hold = Hold::new(12, target, End::Heard(ASSAULT_ENDS));
            return Line::Then([format!("weapon {send} {at}")].into(), Next::Hold(hold));
        }
        return one(format!("weapon {send} {at}"));
    }
    let cry = send.to_ascii_lowercase();
    if let Some((_, stamina)) = WARCRIES.iter().find(|(w, _)| *w == cry || *w == first) {
        let overexerted = up_in(state, "Debuffs", "Overexerted");
        let short = state
            .stamina()
            .and_then(|v| v.current)
            .is_some_and(|have| have < *stamina);
        if overexerted || short {
            return Line::Skip;
        }
        let aimed = !cry.contains("all") && matches!(first.as_str(), "bellow" | "growl" | "cry");
        return one(if aimed {
            format!("warcry {cry} {at}")
        } else {
            format!("warcry {cry}")
        });
    }
    one(send.to_owned())
}

/// Whether a maneuver is cooling: the Cooldowns dialog lists it, or the
/// game refused it and has not said it is ready.
fn cooling(state: &GameState, name: &str) -> bool {
    up_in(state, "Cooldowns", name) || state.maneuvers.said_at(name).is_some()
}

/// Whether an effect whose name starts `name` is up in the dialog `title`.
pub(super) fn up_in(state: &GameState, title: &str, name: &str) -> bool {
    let Some(now) = state.game_time_now() else {
        return false;
    };
    state
        .effects
        .in_category(title)
        .filter(|(_, effect)| effect.text.starts_with(name))
        .any(|(id, _)| state.effects.active(id, now) == Some(true))
}
