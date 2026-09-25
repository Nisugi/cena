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
//! | a combat maneuver, `bearhug`, `dislodge` | `cman <step> #id`; not while it is cooling | `cmd_cmans`, `cmd_rogue_cmans`, `cmd_bearhug` |
//! | a weapon technique | `weapon <step> #id`; not while it is cooling | `cmd_weapons`, `cmd_assault` |
//! | `shield bash` and the other shield moves | `shield <move> #id` | `cmd_shields` |
//! | `chastise`, `excoriate` | `feat <step> #id` | `cmd_feats` |
//! | `shout`, `yowlp`, `holler`, `bellow`, `growl`, `cry` | `warcry <step>`, at the target for bellow, growl and cry alone; not short of stamina | `cmd_warrior_shouts` |
//! | `fire`, `throw`, `smite`, `sacrifice` | the verb `#id` | `cmd_ranged`, `cmd_throw`, `cmd_volnsmite`, `cmd_sacrifice` |
//! | `burst`, `surge` | `cman burst`, `cman surge` | `cmd_burst`, `cmd_surge` |
//! | `jewel <mnemonic>` | the jewel's activation (`src/gemstone/jewel.rs`); not while it is cooling, nor for a mnemonic bigshot does not know | `cmd_jewel` |
//! | `rapid`, `leech`, `phase` | Rapid Fire (515), 516, Phase (704) at the target | `cmd_rapid`, `cmd_leech`, `cmd_phase` |
//! | `depress` | `renew 1015` | `cmd_depress` |
//! | `curse <kind>` | `prep 715` then `curse #id <kind>` | `cmd_curse` |
//! | `caststop N` | N at the target, then `stop N` | `cmd_caststop` |
//! | `unravel`, `barddispel` | 1013 at the target | `cmd_unravel` |
//! | `dhurl <part>` | `hurl #id <part>` | `cmd_dhurl` |
//! | `sleep N`, `wait N` | nothing for N seconds | `cmd_sleep`, `wait_for_swing` |
//!
//! `hide`, `stance <name>`, `store <anything>`, `assume`, `berserk` and
//! `stomp` are game commands as written and go as written. `ambush`, `wand`
//! and `script` are the engine's own ([`super::aim`], [`super::wand`], the
//! importer). The forms not ported yet ([`UNPORTED`]) are skipped, and the
//! player is told once.

use std::collections::VecDeque;

use cena_session::{GameState, PsmCategory};

use super::engine::Hunt;
use super::said::Said;
use crate::cast::{self, Casting, NotReady, Verb};
use crate::gemstone::jewel;

/// What a step sends.
#[derive(Debug, PartialEq, Eq)]
enum Line {
    /// These lines, in order: the first now, the rest right after it.
    Send(VecDeque<String>),
    /// Nothing for this many seconds.
    Wait(u32),
    /// Not now: bigshot's handler would return without sending.
    Skip,
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
        match line(send, target, state) {
            Line::Send(mut lines) => match lines.pop_front() {
                None => Go::Skip,
                Some(first) => {
                    self.followups = lines;
                    Go::Send(first)
                }
            },
            Line::Wait(seconds) => {
                self.used.record(key, Some(target), now);
                Go::Said(Said::Wait(seconds))
            }
            Line::Skip => Go::Skip,
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

/// Combat maneuvers, `cman <step> #id`, with the name the Cooldowns dialog
/// lists: bigshot's tables for `cmd_cmans` (`bigshot.lic:5038-5061`),
/// `cmd_bearhug`, and the rogue's `cmd_rogue_cmans` (`:5318-5333`).
const CMANS: &[(&str, &str)] = &[
    ("bullrush", "Bull Rush"),
    ("coupdegrace", "Coup de Grace"),
    ("cpress", "Crowd Press"),
    ("dirtkick", "Dirtkick"),
    ("disarm", "Disarm Weapon"),
    ("exsanguinate", "Exsanguinate"),
    ("feint", "Feint"),
    ("gkick", "Groin Kick"),
    ("hamstring", "Hamstring"),
    ("haymaker", "Haymaker"),
    ("headbutt", "Headbutt"),
    ("kifocus", "Ki Focus"),
    ("leapattack", "Leap Attack"),
    ("mblow", "Mighty Blow"),
    ("sattack", "Spin Attack"),
    ("sbash", "Shield Bash"),
    ("sblow", "Staggering Blow"),
    ("scleave", "Spell Cleave"),
    ("sthieve", "Spell Thieve"),
    ("sunder", "Sunder Shield"),
    ("tackle", "Tackle"),
    ("trip", "Trip"),
    ("truestrike", "True Strike"),
    ("vaultkick", "Vault Kick"),
    ("bearhug", "Bearhug"),
    ("cutthroat", "Cutthroat"),
    ("divert", "Divert"),
    ("shroud", "Dust Shroud"),
    ("eviscerate", "Eviscerate"),
    ("eyepoke", "Eyepoke"),
    ("footstomp", "Footstomp"),
    ("garrote", "Garrote"),
    ("kneebash", "Kneebash"),
    ("mug", "Mug"),
    ("nosetweak", "Nosetweak"),
    ("spunch", "Sucker Punch"),
    ("subdue", "Subdue"),
    ("sweep", "Sweep"),
    ("swiftkick", "Swiftkick"),
    ("templeshot", "Templeshot"),
    ("throatchop", "Throatchop"),
];

/// Weapon techniques, `weapon <step> #id` (`:4623-4628`, `:4728-4738`).
const WEAPONS: &[(&str, &str)] = &[
    ("barrage", "Barrage"),
    ("flurry", "Flurry"),
    ("fury", "Fury"),
    ("gthrusts", "Guardant Thrusts"),
    ("pummel", "Pummel"),
    ("thrash", "Thrash"),
    ("charge", "Charge"),
    ("clash", "Clash"),
    ("cripple", "Cripple"),
    ("cyclone", "Cyclone"),
    ("dizzyingswing", "Dizzying Swing"),
    ("pindown", "Pin Down"),
    ("pulverize", "Pulverize"),
    ("twinhammer", "Twin Hammerfists"),
    ("volley", "Volley"),
    ("wblade", "Whirling Blade"),
    ("whirlwind", "Whirlwind"),
];

/// The shield moves after `shield` (`:4076`).
const SHIELD_MOVES: &[&str] = &[
    "throw", "bash", "charge", "strike", "pin", "trample", "push",
];

/// Warcries and the stamina each needs (`cmd_warrior_shouts`, `:4913-4921`).
const WARCRIES: &[(&str, i32)] = &[
    ("shout", 25),
    ("yowlp", 11),
    ("holler", 31),
    ("bellow all", 21),
    ("bellow", 11),
    ("growl all", 15),
    ("growl", 8),
    ("cry all", 31),
    ("cry", 16),
];

/// What a plant spell leaves in the room: Tangleweed is not cast again
/// while one is here (`cmd_weed`, `:5754`).
const PLANTS: &[&str] = &[
    "vine",
    "bramble",
    "widgeonweed",
    "vathor club",
    "swallowwort",
    "smilax",
    "creeper",
    "briar",
    "ivy",
    "tumbleweed",
];

/// Spells cast on oneself, whatever the target (`spell_is_selfcast?`).
const SELF_CAST: &[u16] = &[
    106, 109, 115, 117, 120, 130, 140, 205, 206, 211, 213, 215, 218, 219, 220, 240, 303, 307, 310,
    313, 314, 319, 350, 401, 402, 403, 404, 405, 406, 414, 418, 419, 425, 430, 503, 506, 507, 508,
    509, 511, 513, 515, 517, 520, 535, 540, 601, 602, 604, 605, 606, 608, 612, 613, 617, 618, 620,
    625, 630, 640, 650, 707, 712, 905, 911, 913, 916, 919, 1003, 1006, 1007, 1009, 1010, 1011,
    1012, 1014, 1017, 1018, 1019, 1020, 1025, 1035, 1040, 1109, 1119, 1125, 1130, 1150, 1202, 1204,
    1208, 1213, 1214, 1215, 1216, 1220, 1235, 1601, 1605, 1606, 1607, 1608, 1609, 1610, 1611, 1612,
    1613, 1616, 1617, 1618, 1619, 1635,
];

/// bigshot verbs not sent yet, by their first word.
pub(super) const UNPORTED: &[&str] = &[
    "eachtarget",
    "celerity",
    "haste",
    "slayer",
    "tonis",
    "resonance",
    "briar",
    "efury",
    "tether",
    "nudgeweapon",
    "nudgeweapons",
    "unarmed",
    "mstrike",
    "wandolier",
    // Needs the worn-items list to choose `remove` or `get` (`cmd_wield`).
    "wield",
];

/// What `send`, a routine step with its guards gone, puts on the wire
/// against creature `target`.
fn line(send: &str, target: i64, state: &GameState) -> Line {
    let words: Vec<&str> = send.split_whitespace().collect();
    let Some(first) = words.first().map(|w| w.to_ascii_lowercase()) else {
        return Line::Skip;
    };
    let rest = words.get(1..).unwrap_or_default().join(" ");
    let at = format!("#{target}");
    let one = |text: String| Line::Send(VecDeque::from([text]));
    if let Some(word) = UNPORTED.iter().find(|w| **w == first) {
        return Line::Unported(word);
    }
    // `celerity fire` and its kin carry a step after them; bare, 506 is a spell.
    if matches!(first.as_str(), "506" | "240" | "1035") && !rest.is_empty() {
        return Line::Unported("a buff before a step (celerity, slayer, tonis)");
    }
    if let Some(spell) = spell_step(&first, &words) {
        return spell.cast(target, state);
    }
    match first.as_str() {
        "kweed" | "weed" => return weed(first == "kweed", target, state),
        "fire" | "throw" | "smite" | "sacrifice" => return one(format!("{first} {at}")),
        "burst" | "surge" => return one(format!("cman {first}")),
        "chastise" | "excoriate" => return one(format!("feat {send} {at}")),
        "depress" => return one("renew 1015".to_owned()),
        "rapid" => return Spell::bare(515).cast(target, state),
        "leech" => return Spell::bare(516).cast(target, state),
        "phase" => return Spell::at(704).cast(target, state),
        "unravel" | "barddispel" => return Spell::at(1013).cast(target, state),
        "dislodge" => return one(format!("cman dislodge {at} {rest}").trim_end().to_owned()),
        "dhurl" => return one(format!("hurl {at} {rest}").trim_end().to_owned()),
        "curse" if !rest.is_empty() => {
            return Line::Send(VecDeque::from([
                "prep 715".to_owned(),
                format!("curse {at} {rest}"),
            ]));
        }
        "caststop" => return caststop(&words, target, state),
        "jewel" => {
            return match jewel::activate(&rest) {
                Some((_, name)) if cooling(state, name) => Line::Skip,
                Some((line, _)) => one(line),
                None => Line::Unported("jewel with a mnemonic bigshot does not know"),
            };
        }
        "sleep" | "wait" => {
            return rest
                .split_whitespace()
                .next()
                .and_then(|n| n.parse().ok())
                .map_or(Line::Skip, Line::Wait);
        }
        "shield" => {
            let known = words
                .get(1)
                .is_some_and(|m| SHIELD_MOVES.contains(&m.to_ascii_lowercase().as_str()));
            return if known {
                one(format!("{send} {at}"))
            } else {
                one(send.to_owned())
            };
        }
        _ => {}
    }
    if let Some((_, name)) = CMANS.iter().find(|(w, _)| *w == first) {
        if cooling(state, name) || (first == "coupdegrace" && coup_refused(target, state)) {
            return Line::Skip;
        }
        return one(format!("cman {send} {at}"));
    }
    if let Some((_, name)) = WEAPONS.iter().find(|(w, _)| *w == first) {
        if cooling(state, name) {
            return Line::Skip;
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

/// A spell step: its number, how it is sent, and what came after.
struct Spell {
    number: u16,
    verb: Verb,
    /// Words beyond the verb (`open`, an element): sent as written, since
    /// the casting step does not take them.
    extra: String,
    /// Cast at the creature unless the spell is a self-cast one.
    aimed: bool,
}

impl Spell {
    /// A spell cast with no target (`Spell[N].cast`).
    const fn bare(number: u16) -> Self {
        Self {
            number,
            verb: Verb::Cast,
            extra: String::new(),
            aimed: false,
        }
    }

    /// A spell cast at the creature (`force_cast("#id")`).
    const fn at(number: u16) -> Self {
        Self {
            number,
            verb: Verb::Cast,
            extra: String::new(),
            aimed: true,
        }
    }

    /// The lines, or a skip when the spell is not known or not affordable
    /// (`cmd_spell`'s early returns, Lich's `check_energy`).
    fn cast(&self, target: i64, state: &GameState) -> Line {
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
            return Line::Send(VecDeque::from([format!(
                "incant {}{verb} {}",
                self.number, self.extra
            )]));
        }
        let casting = Casting {
            spell: self.number,
            target: (self.aimed && !SELF_CAST.contains(&self.number)).then(|| format!("#{target}")),
            count: None,
            verb: self.verb,
        };
        Line::Send(casting.lines(state).into())
    }
}

/// `incant 611`, `611 evoke`: bigshot's spell pattern (`:4062`).
fn spell_step(first: &str, words: &[&str]) -> Option<Spell> {
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
    })
}

/// Tangleweed at the creature, unless a plant is here already.
fn weed(evoked: bool, target: i64, state: &GameState) -> Line {
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
fn caststop(words: &[&str], target: i64, state: &GameState) -> Line {
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

/// Whether a maneuver is cooling: the Cooldowns dialog lists it, or the
/// game refused it and has not said it is ready.
fn cooling(state: &GameState, name: &str) -> bool {
    up_in(state, "Cooldowns", name) || state.maneuvers.said_at(name).is_some()
}

/// bigshot's last-moment coup gate (`cmd_cmans`, `:4967-4979`): trained,
/// the creature's health known, and not eligible now.
fn coup_refused(target: i64, state: &GameState) -> bool {
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

/// Whether an effect whose name starts `name` is up in the dialog `title`.
fn up_in(state: &GameState, title: &str, name: &str) -> bool {
    let Some(now) = state.game_time_now() else {
        return false;
    };
    state
        .effects
        .in_category(title)
        .filter(|(_, effect)| effect.text.starts_with(name))
        .any(|(id, _)| state.effects.active(id, now) == Some(true))
}
