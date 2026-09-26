//! The handlers that gate what they send on the character and the creature,
//! and those that hold or read the answer ([`super::super::follow`]), each
//! as bigshot's does, cited per function.
//!
//! Left out, and said here: `throw` does not empty the hands and fill them
//! again after (`empty_hands`/`fill_hands` are Lich's stash settings);
//! `tether recast` does not follow the tether to the next creature; and a
//! `store` naming something other than a hand goes as written, where bigshot
//! sends `store both` (`cmd_store`'s `nil` hand, `:4570-4584`), since
//! `store weapon` is a game command a profile means.

use std::collections::VecDeque;

use cena_session::{GameState, PsmCategory, StatusName, gameobj};

use super::super::engine::Hunt;
use super::super::follow::{Answer, EFURY_ENDS, End, Hold, Next, TETHER_ENDS};
use super::super::wand;
use super::spell::Spell;
use super::tables::SHIELD_MOVES;
use super::{Line, cooling, up_in};
use crate::cast::{self, NotReady};
use crate::stance::{self, Want};

/// The curses `curse` takes (`bigshot.lic:4144`); another is sent as written.
const CURSES: &[&str] = &[
    "clumsy",
    "weakness",
    "darkness",
    "itch",
    "hex",
    "pox",
    "nightmare",
    "star",
];

pub(super) fn one(line: impl Into<String>) -> Line {
    Line::Send(VecDeque::from([line.into()]))
}

fn then(lines: impl IntoIterator<Item = String>, next: Next) -> Line {
    Line::Then(lines.into_iter().collect(), next)
}

/// `Spell[n].known? && Spell[n].affordable?`, as [`cast::ready`] answers
/// it: an unread list lets it go.
pub(super) fn castable(state: &GameState, number: u16) -> bool {
    !matches!(
        cast::ready(state, number, 1, 0),
        Err(NotReady::NotKnown | NotReady::Mana(..) | NotReady::Spirit | NotReady::Stamina)
    )
}

/// Seconds left on the effect named `name` in dialog `title`, the most
/// if several; `None` when none is up.
fn left_in(state: &GameState, title: &str, name: &str) -> Option<u32> {
    let now = state.game_time_now()?;
    state
        .effects
        .in_category(title)
        .filter(|(id, effect)| {
            effect.text.starts_with(name) && state.effects.active(id, now) == Some(true)
        })
        .map(|(id, _)| state.effects.remaining(id, now).unwrap_or(u32::MAX))
        .max()
}

fn below(vital: Option<cena_session::Vital>, points: i32) -> bool {
    vital
        .and_then(|v| v.current)
        .is_some_and(|have| have < points)
}

/// Lich's `available?` says no (`psms.rb:142-145`): not trained, not
/// affordable, cooling or overexerted. Unknown lets it go, as the game
/// says so if it is wrong.
pub(super) fn unavailable(state: &GameState, category: PsmCategory, mnemonic: &str) -> bool {
    state.psm_availability(category, mnemonic).available() == Some(false)
}

/// A shield move (`cmd_shields`, `bigshot.lic:4779-4846`): `shield bash` as
/// the Shield Bash maneuver when that is available, otherwise the shield
/// specialization, each only when available; a word that is not a move goes
/// as written.
pub(super) fn shield(rest: &str, send: &str, target: i64, state: &GameState) -> Line {
    let mv = rest
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !SHIELD_MOVES.contains(&mv.as_str()) {
        return one(send);
    }
    let sbash = state
        .psm_availability(PsmCategory::CombatManeuver, "sbash")
        .available();
    if mv == "bash" && sbash == Some(true) {
        return one(format!("cman sbash #{target}"));
    }
    if unavailable(state, PsmCategory::Shield, &mv) {
        return Line::Skip;
    }
    one(format!("shield {mv} #{target}"))
}

/// A feat (`cmd_feats`, `bigshot.lic:5104-5143`): only when available.
pub(super) fn feat(which: &str, send: &str, target: i64, state: &GameState) -> Line {
    if unavailable(state, PsmCategory::Feat, which) {
        return Line::Skip;
    }
    one(format!("feat {send} #{target}"))
}

/// `wield <noun> [left|right]` (`cmd_wield`, `bigshot.lic:4549-4564`):
/// nothing when that hand holds it already; the hand stored, then the item
/// removed when it is worn, got from a container otherwise.
pub(super) fn wield(rest: &str, state: &GameState) -> Line {
    let mut words = rest.split_whitespace();
    let Some(noun) = words.next() else {
        return Line::Skip;
    };
    let left = words.next() == Some("left");
    let hand = if left {
        &state.left_hand
    } else {
        &state.right_hand
    };
    if hand.noun() == Some(noun) {
        return Line::Skip;
    }
    let store = if left { "store left" } else { "store right" };
    let take = if state.worn.wears(noun) == Some(true) {
        format!("remove my {noun}")
    } else {
        format!("get my {noun}")
    };
    Line::Send(VecDeque::from([store.to_owned(), take]))
}

/// `briar <noun>` (`cmd_briar`, `bigshot.lic:5650-5673`): each weapon of
/// that noun in a hand or worn is measured, and raised once its briars are
/// at 100 percent; not while Briar (9105) is up.
pub(super) fn briar(noun: &str, state: &GameState) -> Line {
    let up = state
        .game_time_now()
        .is_some_and(|now| state.effects.active("9105", now) == Some(true));
    if up {
        return Line::Skip;
    }
    let hands = [&state.right_hand, &state.left_hand]
        .into_iter()
        .filter(|hand| hand.noun() == Some(noun))
        .filter_map(|hand| hand.id().map(str::to_owned));
    let worn = state
        .worn
        .items()
        .unwrap_or_default()
        .iter()
        .filter(|item| item.noun == noun)
        .map(|item| item.id.clone());
    let mut weapons: VecDeque<String> = hands.chain(worn).collect();
    let Some(id) = weapons.pop_front() else {
        return Line::Skip;
    };
    then(
        [format!("measure #{id}")],
        Next::Answer(Answer::Measured { id, rest: weapons }),
    )
}

/// `stomp` (`cmd_stomp`, `:6026-6040`): Tremors known; `stomp` while it is
/// up and 5 mana are left; channelled first when it is down.
pub(super) fn stomp(state: &GameState) -> Line {
    let Some(now) = state.game_time_now() else {
        return Line::Skip;
    };
    if state.known_spells.knows(909) == Some(false) {
        return Line::Skip;
    }
    if state.effects.active("909", now) == Some(true) {
        return if below(state.mana(), 5) {
            Line::Skip
        } else {
            one("stomp")
        };
    }
    if !castable(state, 909) {
        return Line::Skip;
    }
    Line::Send(
        ["prepare 909", "channel", "stomp"]
            .map(str::to_owned)
            .into(),
    )
}

/// `leech` (`cmd_leech`, `:6045-6055`): Mana Leech once its cooldown has
/// less than 15 s to run.
pub(super) fn leech(target: i64, state: &GameState) -> Line {
    if left_in(state, "Cooldowns", "Mana Leech").is_some_and(|left| left >= 15) {
        return Line::Skip;
    }
    Spell::bare(516).cast(target, state)
}

/// `rapid [ignore]` (`cmd_rapid`, `:6061-6073`): Rapid Fire, not while it
/// is up with more than three seconds left, nor while it is recovering
/// unless told to ignore that.
pub(super) fn rapid(ignore: bool, target: i64, state: &GameState) -> Line {
    if left_in(state, "Buffs", "Rapid Fire").is_some_and(|left| left > 3) {
        return Line::Skip;
    }
    if !ignore && up_in(state, "Cooldowns", "Rapid Fire Recovery") {
        return Line::Skip;
    }
    Spell::bare(515).cast(target, state)
}

/// `burst`, `surge` (`cmd_burst`, `cmd_surge`, `:6449-6490`): not while
/// the enhancement it gives is up; 30 stamina, or 60 while it cools.
pub(super) fn burst_or_surge(which: &str, state: &GameState) -> Line {
    // `CMan.known?`, the one test of Lich's `available?` bigshot makes here.
    let known = state
        .psm_availability(PsmCategory::CombatManeuver, which)
        .known;
    if known == Some(false) {
        return Line::Skip;
    }
    let (name, buff) = if which == "burst" {
        ("Burst of Swiftness", "Enh. Dexterity")
    } else {
        ("Surge of Strength", "Enh. Strength")
    };
    let enhanced = state.game_time_now().is_some_and(|now| {
        state.effects.in_category("Buffs").any(|(id, effect)| {
            effect.text.contains(buff) && state.effects.active(id, now) == Some(true)
        })
    });
    let need = if up_in(state, "Cooldowns", name) {
        60
    } else {
        30
    };
    if enhanced || below(state.stamina(), need) {
        return Line::Skip;
    }
    one(format!("cman {which}"))
}

/// `smite` (`cmd_volnsmite`, `:5418-5434`): an undead or noncorporeal
/// creature not yet smitten.
pub(super) fn smite(target: i64, state: &GameState) -> Line {
    let Some(creature) = state.creatures().get(target) else {
        return Line::Skip;
    };
    let of = |kind: &str| {
        creature
            .noun
            .as_deref()
            .is_some_and(|noun| gameobj::classify(noun, &creature.name).is(kind))
    };
    if !(of("undead") || of("noncorporeal")) || creature.smote(state.game_time_now()) {
        return Line::Skip;
    }
    one(format!("smite #{target}"))
}

/// `throw` (`cmd_throw`, `:5680-5689`): not at a creature lying down.
pub(super) fn throw(target: i64, state: &GameState) -> Line {
    let down = state
        .creatures()
        .get(target)
        .is_some_and(|creature| creature.has_status(StatusName::Prone, state.game_time_now()));
    if down {
        Line::Skip
    } else {
        one(format!("throw #{target}"))
    }
}

/// `curse <kind>` (`cmd_curse`, `:4674-4694`): Curse (715) prepared, once,
/// released from another spell first, then `curse #id <kind>`; not the
/// star while its bonus has more than 30 s left.
pub(super) fn curse(kind: &str, target: i64, state: &GameState) -> Line {
    if !CURSES.contains(&kind) {
        return one(format!("curse {kind}"));
    }
    let bonus = left_in(state, "Active Spells", "Curse of the Star (bonus)");
    if kind == "star" && bonus.is_some_and(|left| left > 30) {
        return Line::Skip;
    }
    let mut lines = VecDeque::new();
    let prepared = state.prepared.as_deref().unwrap_or("None");
    if prepared != "Curse" {
        if !castable(state, 715) {
            return Line::Skip;
        }
        if prepared != "None" {
            lines.push_back("release".to_owned());
        }
        lines.push_back("prep 715".to_owned());
    }
    lines.push_back(format!("curse #{target} {kind}"));
    Line::Send(lines)
}

/// `store left|right|both` (`cmd_store`, `:4570-4584`): nothing when that
/// hand is empty. Another word goes as written (the module doc).
pub(super) fn store(hand: &str, send: &str, state: &GameState) -> Line {
    let (left, right) = (state.left_hand.is_empty(), state.right_hand.is_empty());
    let empty = match hand {
        "right" => right,
        "left" => left,
        "both" => left && right,
        _ => return one(send),
    };
    if empty { Line::Skip } else { one(send) }
}

/// `stance <name or percent>` (`change_stance`, `:6884-6912`): nothing
/// when it is taken already; a percent by Stance Perfection when trained.
pub(super) fn stance(arg: &str, send: &str, state: &GameState) -> Line {
    match Want::parse(arg) {
        // `command` sends nothing for a stance already taken.
        Ok(want) => stance::command(want, state).map_or(Line::Skip, one),
        Err(_) => one(send),
    }
}

/// `sacrifice` (`cmd_sacrifice`, `:6611-6622`): 2 spirit and not cooling;
/// the creature appraised first, and sacrificed if it reads frail.
pub(super) fn sacrifice(target: i64, state: &GameState) -> Line {
    if below(state.spirit(), 2) || cooling(state, "Sacrifice") {
        return Line::Skip;
    }
    then(
        [format!("appraise #{target}")],
        Next::Answer(Answer::Appraised { target }),
    )
}

/// `depress` (`cmd_depress`, `:4870-4892`): `renew 1015`, the song begun
/// when it is not being sung.
pub(super) fn depress(state: &GameState) -> Line {
    if !castable(state, 1015) {
        return Line::Skip;
    }
    then(["renew 1015".to_owned()], Next::Answer(Answer::Depress))
}

/// `unravel [extra]`, `barddispel` (`cmd_unravel`, `:4915-4957`): 1013 at
/// the creature, the song stopped as the answer asks.
pub(super) fn unravel(extra: &str, target: i64, state: &GameState) -> Line {
    match Spell::at(1013).cast(target, state) {
        Line::Send(mut lines) => {
            if let Some(last) = lines.back_mut().filter(|_| !extra.is_empty()) {
                last.push(' ');
                last.push_str(extra);
            }
            let again = lines.clone();
            then(lines, Next::Answer(Answer::Unravel { again, tries: 0 }))
        }
        other => other,
    }
}

/// `efury [fire|cold]` (`cmd_efury`, `:6080-6120`): incanted, then held
/// until the fury ends.
pub(super) fn efury(extra: &str, target: i64, state: &GameState) -> Line {
    if !castable(state, 917) {
        return Line::Skip;
    }
    let line = format!("incant 917 {extra}").trim_end().to_owned();
    then(
        [line],
        Next::Hold(Hold::new(12, target, End::Heard(EFURY_ENDS))),
    )
}

/// `tether` (`cmd_tether`, `:6630-6693`): incanted, then held until it
/// completes, breaks or passes on.
pub(super) fn tether(target: i64, state: &GameState) -> Line {
    if !castable(state, 706) {
        return Line::Skip;
    }
    then(
        ["incant 706".to_owned()],
        Next::Hold(Hold::new(12, target, End::Heard(TETHER_ENDS))),
    )
}

impl Hunt {
    /// The handlers that read the hunt's own settings or memory; `None`
    /// for a step that is not one of them.
    pub(super) fn own_verb(
        &mut self,
        first: &str,
        rest: &str,
        target: i64,
        state: &GameState,
        now: Option<u32>,
    ) -> Option<Line> {
        let wander = || Self::stance_for(self.profile.stance.wander.as_deref(), state);
        let seconds = rest.split_whitespace().next().and_then(|n| n.parse().ok());
        Some(match first {
            // `wait_for_swing`, `:6919-6962`: in the wander stance unless the
            // creature is down, until it swings or the time is up.
            "wait" => {
                let seconds = seconds?;
                let creature = state.creatures().get(target);
                let down = creature
                    .is_some_and(|c| c.has_status(StatusName::Prone, state.game_time_now()));
                let noun = creature.and_then(|c| c.noun.clone()).unwrap_or_default();
                let stance = if down { None } else { wander() };
                then(
                    stance,
                    Next::Hold(Hold::new(seconds, target, End::Swing(noun))),
                )
            }
            // `cmd_sleep`, `:6529-6538`.
            "sleep" => {
                let seconds = seconds?;
                let stance = if rest.contains("nostance") {
                    None
                } else {
                    wander()
                };
                then(stance, Next::Hold(Hold::new(seconds, target, End::Time)))
            }
            // `cmd_berserk`, `:6495-6506`: held while Berserk (9607) is up.
            "berserk" => {
                if below(state.stamina(), 20) {
                    return Some(Line::Send(
                        ["target random", "kill"].map(str::to_owned).into(),
                    ));
                }
                let lines = wander().into_iter().chain(["berserk".to_owned()]);
                then(lines, Next::Hold(Hold::new(60, target, End::Effect(9607))))
            }
            // `cmd_hide`, `:6126-6137`: until hidden, the tries after the first
            // left to the repeat.
            "hide" => {
                if state.status.known().hidden() == Some(true) {
                    return Some(Line::Skip);
                }
                let lines = wander().into_iter().chain(["hide".to_owned()]).collect();
                self.hide_tries(seconds.filter(|n| *n > 0).unwrap_or(3));
                Line::Send(lines)
            }
            "dhurl" => self.dhurl(rest, target),
            "wandolier" => self.wandolier(rest, state),
            "nudgeweapon" | "nudgeweapons" => self.nudge(state),
            "mstrike" => {
                let (mut lines, strike) =
                    self.mstrike(&format!("{first} {rest}"), target, state, now);
                lines.extend(strike);
                Line::Send(lines)
            }
            "unarmed" if !rest.is_empty() => self.unarmed(rest, target, state, now),
            "dislodge" => self.dislodge(rest, target, state),
            // `cmd_assume`, `:5588-5644`, as maintain casts it for a sign.
            "assume" => match self.assume_aspect(&format!("650 {rest}"), state, now?) {
                Some(said) => Line::Said(said),
                None => Line::Skip,
            },
            _ => return None,
        })
    }

    /// `wandolier [stance] [noreserve]` (`cmd_wandolier`,
    /// `bigshot.lic:5980-6021`): the profile's wand from a hand or the
    /// reserve, else got from the fresh container (rubbed when it has none);
    /// put in the reserve from the hand unless `noreserve`; waved in the
    /// stance named, offensive by default. The reserve is asked for once
    /// (`reserve list`), as bigshot asks while it is unknown.
    fn wandolier(&mut self, rest: &str, state: &GameState) -> Line {
        let Some(fresh) = self.profile.wand.fresh.clone() else {
            return Line::Skip;
        };
        let words: Vec<String> = rest
            .to_ascii_lowercase()
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        let Some(reserve) = state.reserve.items() else {
            if std::mem::replace(&mut self.follow.reserve_asked, true) {
                return Line::Skip;
            }
            return one("reserve list");
        };
        let Some(name) = self.profile.wand.names.get(self.wanding.at()).cloned() else {
            return Line::Skip;
        };
        let in_hand = [&state.right_hand, &state.left_hand]
            .into_iter()
            .find(|hand| {
                hand.name()
                    .is_some_and(|text| wand::named_like(&name, text))
            })
            .and_then(|hand| hand.id().map(str::to_owned));
        let in_reserve = reserve
            .iter()
            .find(|item| wand::named_like(&name, &item.text))
            .map(|item| item.id.clone());
        let Some(id) = in_hand.clone().or(in_reserve) else {
            return then(
                [format!("get {name} from my {fresh}")],
                Next::Answer(Answer::WandGot { fresh }),
            );
        };
        let mut lines = VecDeque::new();
        if in_hand.is_some() && !words.iter().any(|w| w == "noreserve") {
            lines.push_back(format!("reserve #{id}"));
        }
        let stance = words
            .iter()
            .find_map(|w| Want::parse(w).ok())
            .unwrap_or(Want::Named(cena_session::Stance::Offensive));
        lines.extend(stance::command(stance, state));
        lines.push_back(format!("wave #{id}"));
        Line::Then(lines, Next::Answer(Answer::Waved))
    }

    /// `dhurl [part]` (`cmd_dhurl`, `:6280-6322`): the part named, or the
    /// ambush list's, `chest` past its end; `recover hurl` after.
    fn dhurl(&self, rest: &str, target: i64) -> Line {
        let parts: Vec<&str> = if rest.is_empty() {
            self.profile.aim.ambush.iter().map(String::as_str).collect()
        } else {
            vec![rest]
        };
        let part = if parts.is_empty() {
            ""
        } else {
            parts.get(self.aiming.at()).copied().unwrap_or("chest")
        };
        let line = format!("hurl #{target} {part}").trim_end().to_owned();
        then([line], Next::Answer(Answer::Hurled))
    }

    /// `dislodge <parts>` (`cmd_dislodge`, `:6415-6444`): the first part
    /// named that an arrow is lodged in on this creature; not while the
    /// maneuver cools.
    fn dislodge(&self, rest: &str, target: i64, state: &GameState) -> Line {
        if cooling(state, "Dislodge") || unavailable(state, PsmCategory::CombatManeuver, "dislodge")
        {
            return Line::Skip;
        }
        let lodged = &self.aiming.stuck;
        let Some(part) = rest
            .split(' ')
            .take(9)
            .find(|part| lodged.iter().any(|stuck| stuck.eq_ignore_ascii_case(part)))
        else {
            return Line::Skip;
        };
        then(
            [format!("cman dislodge #{target} {part}")],
            Next::Answer(Answer::Dislodged {
                part: part.to_owned(),
            }),
        )
    }
}

/// bigshot's last-moment coup gate (`cmd_cmans`, `:4967-4979`): trained,
/// the creature's health known, and not eligible now.
pub(super) fn coup_refused(target: i64, state: &GameState) -> bool {
    let rank = state
        .character
        .psms
        .get(PsmCategory::CombatManeuver, "coupdegrace")
        .map_or(0, |ranks| u32::from(ranks.ranks));
    let Some(creature) = state.creatures().get(target) else {
        return false;
    };
    let known = creature.hp_is_stated() || creature.has_template();
    rank > 0 && known && !creature.coup_eligible(rank, state.game_time_now())
}

/// Weapons `nudgeweapons` carries out, by a part of their noun, unanchored
/// as bigshot matches them (`cmd_nudge_weapons`, `bigshot.lic:6583`).
const NUDGED: &[&str] = &[
    "axe",
    "scythe",
    "pitchfork",
    "falchion",
    "sword",
    "lance",
    "dagger",
    "estoc",
    "handaxe",
    "katana",
    "katar",
    "gauche",
    "rapier",
    "scimitar",
    "whip-blade",
    "cudgel",
    "crowbill",
    "whip",
    "mace",
    "star",
    "hammer",
    "claidhmore",
    "flail",
    "flamberge",
    "maul",
    "pick",
    "staff",
    "mattock",
];

/// The way back from a compass direction (Lich's `reverse_direction`,
/// `global_defs.rb:713-735`).
fn back(dir: &str) -> Option<&'static str> {
    Some(match dir {
        "n" => "s",
        "ne" => "sw",
        "e" => "w",
        "se" => "nw",
        "s" => "n",
        "sw" => "ne",
        "w" => "e",
        "nw" => "se",
        "up" => "down",
        "down" => "up",
        "out" => "out",
        _ => return None,
    })
}

impl Hunt {
    /// `nudgeweapons` (`cmd_nudge_weapons`, `bigshot.lic:6577-6605`): each
    /// weapon lying here carried out by the next exit, dropped, and walked
    /// back from, in the wander stance, sheathed first when both hands are
    /// full and girded after. Nothing without exits or weapons.
    fn nudge(&mut self, state: &GameState) -> Line {
        let exits = state.room.exits.clone().unwrap_or_default();
        let weapons: Vec<String> = state
            .room
            .objects
            .iter()
            .filter(|object| {
                let noun = object.noun.to_ascii_lowercase();
                NUDGED.iter().any(|weapon| noun.contains(weapon))
            })
            .map(|object| object.id.clone())
            .collect();
        let full = state.right_hand.is_holding() && state.left_hand.is_holding();
        let mut lines: VecDeque<String> =
            Self::stance_for(self.profile.stance.wander.as_deref(), state)
                .into_iter()
                .collect();
        for (id, dir) in weapons.iter().zip(exits.iter()) {
            let Some(back) = back(dir) else {
                continue;
            };
            if full {
                lines.push_back("sheath".to_owned());
            }
            lines.extend([
                format!("get #{id}"),
                dir.clone(),
                format!("drop #{id}"),
                back.to_owned(),
            ]);
            if full {
                lines.push_back("gird".to_owned());
            }
        }
        if !lines.iter().any(|line| line.starts_with("get #")) {
            return Line::Skip;
        }
        let first = lines.pop_front().unwrap_or_default();
        self.repeats.nudging = lines;
        one(first)
    }
}
