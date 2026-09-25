//! The hunt profile: one TOML file, the policy for one kind of hunt
//! (`plan/30` §4 and §5).
//!
//! Every key is a fact the hunt acts on -- where, what, and when to stop --
//! and none is an instruction for how. The shape follows Nisugi's
//! `ojandhaart` profile key by key (`plan/30` §4's table), because that is
//! the hunt M6 is built toward; a key another profile needs is added when
//! that profile arrives, not ahead of it (`plan/12` §6a.1: few, opinionated
//! settings).
//!
//! ```toml
//! prepare = ["ready weapon", "incant 515"]
//! signs = ["515", "506", "605 evoke"]
//! targets = [
//!   { name = "mastodon", routine = "b" },
//!   { any = true, routine = "f" },
//! ]
//!
//! [rooms]
//! hunting = 29902
//! boundaries = [29900, 30115]
//! resting = 29877
//!
//! [rest]
//! fried = 101
//! encumbered = 20
//! until = { experience = 100, mana = 90 }
//! when = { bleeding = true, health_at_most = 60, cannot_use_ranged = true }
//! commands = ["store all"]
//!
//! [routines]
//! b = ["kweed (expiring \"Tangleweed Vigor\" 5)", "volley", "coupdegrace (thp 20 empowered_below 30)", "fire"]
//! f = ["hide (!hidden)", "fire (hidden)"]
//!
//! [sequences]
//! volley = ["store weapon", "ready 2weapon", "weapon volley", "ready weapon"]
//! ```
//!
//! # Steps
//!
//! A routine is a list of [`Step`]s, each a string: what to send, then the
//! guards in parentheses (`guard`). A step naming a sequence stands for that
//! sequence's steps. A step the importer could not carry is **held**, and
//! written as a table so that it cannot be mistaken for one that runs:
//!
//! ```toml
//! a = ["fire", { step = "attack(stunned)", held = "guard `stunned` is not built yet" }]
//! ```
//!
//! # A mistyped key is refused
//!
//! Every table denies unknown fields, so `hunting = 29902` under
//! `[room]` rather than `[rooms]` is an error naming the key, not a profile
//! with no hunting room. A player who mistyped a setting should be told, not
//! silently given the behaviour they were trying to change
//! (`cena_session::settings_store`, the same rule).

use std::collections::BTreeMap;
use std::fmt;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::guard::Condition;
use crate::stance::Want;

/// Everything a hunt is told. See the module docs for the shape.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Profile {
    /// Where to hunt and where to rest.
    pub rooms: Rooms,
    /// The stance for each phase.
    pub stance: Stances,
    /// When to stop hunting, what to do at the rest room, and when to go back.
    pub rest: Rest,
    /// Sent once, on arriving at the hunting ground (`hunting_prep_commands`).
    pub prepare: Vec<String>,
    /// What Maintain keeps up: a spell number, or a number and its words, as
    /// sent (`signs`).
    pub signs: Vec<String>,
    /// Cast Ethereal Censer (320) between routine steps whenever it is off
    /// cooldown and affordable: bigshot's `censer` word, which the author
    /// meant for the whole routine (`plan/33` §2h).
    pub censer_between_actions: bool,
    /// Cast a Voln symbol sign (Courage, Protection, Supremacy) only when
    /// the favor it costs is there (`check_favor`, `bigshot.lic:9264`).
    pub check_favor: bool,
    /// Creatures never attacked, and never counted toward fleeing
    /// (`flee.count`), lowercase. Hydra's own; bigshot has no such setting.
    /// Its nearest is the `untargetable` list it learns from the game
    /// refusing `target`, which it uses the same two ways
    /// (`bigshot.lic:8583`, `:8624-8634`). Not `invalid_targets`, which
    /// is [`Flee::uncounted`].
    pub never_attack: Vec<String>,
    /// When to leave the room.
    pub flee: Flee,
    /// How the dead are looted.
    pub loot: Loot,
    /// How the ground is walked between fights.
    pub wander: Wander,
    /// What the hunt does about the game's incidents (`hunt/react.rs`).
    pub react: React,
    /// Where attacks are aimed (`hunt/aim.rs`).
    pub aim: Aim,
    /// Wands for a `wand` step (`hunt/wand.rs`).
    pub wand: Wands,
    /// Boon traits to leave alone or flee from (`hunt/boons.rs`).
    pub boons: Boons,
    /// What to attack, in order of preference, each with its routine.
    pub targets: Vec<Target>,
    /// Leave the current target for a better-ranked one that appears
    /// (`priority`, `bigshot.lic:8703-8720`). Off, the current target is
    /// fought until it dies or goes.
    pub priority: bool,
    /// The routines, by name: the steps taken against a target, in order.
    pub routines: BTreeMap<String, Vec<Step>>,
    /// Named lists of steps a routine step may stand for, such as `volley`.
    pub sequences: BTreeMap<String, Vec<Step>>,
}

/// Rooms, by the map's numbers (`cena_map::RoomId`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Rooms {
    /// Where the hunt starts and returns to (`hunting_room_id`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hunting: Option<u32>,
    /// The rooms Wander stays within (`hunting_boundaries`).
    pub boundaries: Vec<u32>,
    /// Rooms walked through, in order, on the way back to hunt
    /// (`rallypoint_room_ids`, `bigshot.lic:7253-7262`).
    pub rally: Vec<u32>,
    /// Where to rest (`resting_room_id`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resting: Option<u32>,
}

/// A stance per phase: a stance's name, or a percent to defense
/// (`crate::stance::Want`). `None` leaves the stance alone.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Stances {
    /// While fighting (`hunting_stance`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hunting: Option<String>,
    /// While walking between fights (`wander_stance`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wander: Option<String>,
    /// After standing up (`stand_stance`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stand: Option<String>,
}

/// When to stop hunting and go rest, and when to come back.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Rest {
    /// Rest when the mind is at or above this percent (`fried`); 101 never.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fried: Option<u32>,
    /// Kills allowed after the mind is full before resting (`overkill`).
    pub overkill: u32,
    /// Rest when encumbrance is at or above this percent (`encumbered`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encumbered: Option<u32>,
    /// Rest when mana falls below this percent (`oom`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mana_below: Option<u32>,
    /// Rest until each of these is met (`rest_till_*`).
    pub until: Until,
    /// Rest at once when any of these holds (`wounded_eval`, typed).
    pub when: When,
    /// End the hunt after this many rests: an overnight run's stop.
    /// Hydra's own; bigshot has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_after: Option<u32>,
    /// Fried, spend up to this many Long-Term Experience Boosts (`boost
    /// longterm`) before resting (`lte_boost`, `bigshot.lic:8826-8846`).
    pub lte_boost: u32,
    /// Before resting for mana, use a society's mana ability
    /// (`use_wracking`, `hunt/wrack.rs`).
    pub wracking: bool,
    /// The least spirit Sign of Wracking is used at (`wracking_spirit`).
    pub wracking_spirit: u32,
    /// Sent on leaving the hunt for a rest, before any walking: a spell or
    /// symbol that carries the character toward town (`fog_return`,
    /// `custom_fog`, `bigshot.lic:7677-7723`). Empty walks the whole way.
    pub fog: Vec<String>,
    /// Fog only when resting wounded or encumbered (`fog_optional`).
    pub fog_optional: bool,
    /// Fog again when the first lands in room 2635, the rift
    /// (`fog_rift`, `:7635`).
    pub fog_rift: bool,
    /// Rooms walked through, in order, on the way to rest
    /// (`return_waypoint_ids`, `:7494-7496`).
    pub waypoints: Vec<u32>,
    /// Sent on arriving at the rest room (`resting_commands`).
    pub commands: Vec<String>,
}

/// What a rest waits for, each a percent (`rest_till_*`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Until {
    /// The mind at or below this percent (`rest_till_exp`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experience: Option<u32>,
    /// Mana at or above this percent (`rest_till_mana`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mana: Option<u32>,
    /// Spirit at or above this many **points**, not a percent
    /// (`rest_till_spirit`; bigshot compares `Char.spirit`, `rest.rb:239`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spirit: Option<u32>,
    /// Stamina at or above this percent (`rest_till_percentstamina`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stamina: Option<u32>,
}

/// What sends the hunter to rest at once: bigshot's `wounded_eval`, which
/// was a Ruby expression, as typed thresholds (`plan/30` §5). Any one
/// holding is enough.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is one of bigshot's rest switches, carried as it is"
)]
pub struct When {
    /// I am bleeding.
    pub bleeding: bool,
    /// Health at or below this percent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_at_most: Option<u32>,
    /// A wound stops me casting (`Injuries::able_to_cast`).
    pub cannot_cast: bool,
    /// A wound stops me using a ranged weapon (`Injuries::able_to_use_ranged`).
    pub cannot_use_ranged: bool,
    /// Creeping Dread at or above this many stacks (`creeping_dread`,
    /// `bigshot.lic:8892-8901`: the debuff's `(N)`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creeping_dread: Option<u32>,
    /// Crushing Dread at or above this many stacks (`crushing_dread`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crushing_dread: Option<u32>,
    /// Wall of Thorns Poison is on me (`wot_poison`).
    pub wot_poison: bool,
    /// The Confused debuff is on me (`confusion`).
    pub confused: bool,
    /// Spirit at or below this percent. Hydra's own: bigshot rests on
    /// spirit only through `wounded_eval`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spirit_at_most: Option<u32>,
    /// A wound or scar of this rank or worse on any part (the rank that
    /// governs it, `Injuries::effective_rank`: rank-1 scars do not count).
    /// Hydra's own.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wound_rank: Option<u8>,
}

/// When to leave the room.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "bigshot's four hazard switches, carried as they are"
)]
pub struct Flee {
    /// Leave when more than this many hostile creatures are here
    /// (`flee_count`). Every creature the hunt could fight counts, whether or
    /// not the target list names it, as bigshot counts its hostile roster
    /// (`bigshot.lic:8579-8591`), less [`Flee::uncounted`] and
    /// [`Profile::never_attack`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    /// Creatures that do not count toward `count` (`invalid_targets`,
    /// bigshot's *"but don't count these"*, `bigshot.lic:3484`), lowercase.
    /// They are still fought when the target list names them.
    pub uncounted: Vec<String>,
    /// Leave when any of these is here (`always_flee_from`), lowercase.
    pub from: Vec<String>,
    /// Leave a room with a cloud or a breath in it, or an intense
    /// shimmering circle (`flee_clouds`, `bigshot.lic:8556`).
    pub clouds: bool,
    /// ... a vine (`flee_vines`).
    pub vines: bool,
    /// ... a web (`flee_webs`).
    pub webs: bool,
    /// ... a black void (`flee_voids`).
    pub voids: bool,
    /// On entering a room, leave if more than one creature could be fought
    /// (`lone_targets_only`, `bigshot.lic:8590`).
    pub lone_only: bool,
    /// Leave when the game says any of these, ignoring case
    /// (`flee_message`, a regex in bigshot; its `|` alternatives here).
    pub messages: Vec<String>,
}

/// How the dead are looted.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Loot {
    /// The looter (`loot_script`): `eloot`, once it is ported (`plan/30` §4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    /// While targets remain, loot no oftener than every fifteen seconds
    /// (`delay_loot`, bigshot's `time_between(:need_to_loot?, 15)`); the
    /// first corpse is looted at once, as bigshot's first call passes.
    pub delay: bool,
    /// Go defensive to loot while creatures are present (`loot_stance`).
    pub defensive: bool,
    /// The looter may leave a box in hand (`box_in_hand`).
    pub box_in_hand: bool,
}

/// How the ground is walked between fights.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Wander {
    /// Seconds to wait in each room before moving on (`wander_wait`).
    pub wait: f64,
    /// A stranger's floating disk does not make a room theirs
    /// (`ignore_disks`, `bigshot.lic:7097`). Off, a room entered with one
    /// in it is not fought in, as bigshot does.
    pub ignore_disks: bool,
}

impl Default for Wander {
    fn default() -> Self {
        Self {
            wait: 0.3,
            ignore_disks: false,
        }
    }
}

/// Boon traits, by bigshot's names (`blink`, `boosted_hp`, ...,
/// `cena_session::boons::TRAITS`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Boons {
    /// A creature with any of these is neither fought nor counted
    /// (`boons_ignore`).
    pub ignore: Vec<String>,
    /// A creature with any of these sends the hunt out of the room
    /// (`boons_flee`).
    pub flee: Vec<String>,
}

/// Wands, and where they are kept (`hunt/wand.rs`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Wands {
    /// The wands to use, in order, by name (`wand`).
    pub names: Vec<String>,
    /// The container fresh wands are got from (`fresh_wand_container`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fresh: Option<String>,
    /// The container a spent wand is put in; none drops it
    /// (`dead_wand_container`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dead: Option<String>,
    /// A spell step the character cannot afford waves a wand instead
    /// (`wand_if_oom`).
    pub if_oom: bool,
}

/// Body parts to aim at, in order, each lowercase (`hunt/aim.rs`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Aim {
    /// For an `ambush` step (`ambush`); empty is bigshot's default, head,
    /// right leg, left leg, chest.
    pub ambush: Vec<String>,
    /// For a `fire` step (`archery_aim`); empty sends no `aim`.
    pub archery: Vec<String>,
    /// Where an item the game would not fire goes when the stow container is
    /// closed (`ammo_container`, `bigshot.lic:6391-6398`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ammo_container: Option<String>,
}

/// The hunt's answers to incidents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "bigshot's switches, carried as they are"
)]
pub struct React {
    /// Get a weapon back that was knocked, pulled or webbed away.
    pub recover: bool,
    /// Take a weapon reaction when the game offers one
    /// (`weapon_reaction`, on by default in bigshot, `bigshot.lic:1380`).
    pub weapon_reaction: bool,
    /// Pull a fallen player to their feet while something hostile is
    /// here; a group member always (`pull`, on by default in bigshot,
    /// `bigshot.lic:3902-3919`).
    pub pull: bool,
    /// End the hunt when a dead player is in the room (`deader`,
    /// `:3921-3927`; bigshot pauses).
    pub deader: bool,
    /// Bless a weapon whose blessing is shrugged off or gone, with Bless
    /// (304) or the Voln symbol (`bless`, `bigshot.lic:5553-5581`).
    pub bless: bool,
    /// On Shattered, quit the game when dead or below 40 percent health
    /// (`dead_man_switch`, `bigshot.lic:6760-6768`; `hunt/death.rs`).
    pub dead_man_switch: bool,
    /// Dead: depart, recover, and hunt again (`depart_switch`,
    /// `bigshot.lic:6770-6784`; `hunt/death.rs`).
    pub depart_switch: bool,
}

impl Default for React {
    fn default() -> Self {
        Self {
            recover: true,
            weapon_reaction: true,
            pull: true,
            deader: false,
            bless: false,
            dead_man_switch: false,
            depart_switch: false,
        }
    }
}

/// One entry in the target list. Matched against a creature's noun or
/// whole name, ignoring case, as bigshot does (`bigshot.lic:7170`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Target {
    /// The creature, by noun or whole name, lowercase.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Any creature: the list's catch-all.
    pub any: bool,
    /// The routine run against it, by name under `[routines]`.
    pub routine: String,
}

impl Default for Target {
    fn default() -> Self {
        Self {
            name: None,
            any: false,
            routine: "a".to_owned(),
        }
    }
}

/// One step of a routine or sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    /// What is sent, guards removed: `coupdegrace`, `incant 611`, or a
    /// sequence's name. For a held step, the whole line as it was.
    pub send: String,
    /// The guards, all of which must hold for the step to run.
    pub when: Vec<Condition>,
    /// Why this step never runs: it was imported with a guard Hydra does not
    /// have yet, or a shape it does not read. `None` for a step that runs.
    pub held: Option<String>,
}

impl Step {
    /// Read a step: what to send, then optionally the guards in parentheses,
    /// such as `coupdegrace (thp 20 empowered_below 30)`.
    ///
    /// # Errors
    ///
    /// Nothing to send, an unbalanced parenthesis, or a guard
    /// [`Condition::parse_group`] refuses.
    pub fn parse(text: &str) -> Result<Self, String> {
        let text = text.trim();
        let (send, group) = split_guards(text)?;
        if send.is_empty() {
            return Err(format!("{text:?} has nothing to send"));
        }
        let when = group.map_or_else(|| Ok(Vec::new()), Condition::parse_group)?;
        Ok(Self {
            send: send.to_owned(),
            when,
            held: None,
        })
    }

    /// A step that never runs, kept with the reason.
    #[must_use]
    pub fn held(text: &str, why: &str) -> Self {
        Self {
            send: text.trim().to_owned(),
            when: Vec::new(),
            held: Some(why.to_owned()),
        }
    }
}

/// `send` and the text inside the trailing parentheses, split at the first
/// `(` outside quotes. A line not ending in `)` has no guards.
fn split_guards(text: &str) -> Result<(&str, Option<&str>), String> {
    let Some(body) = text.strip_suffix(')') else {
        return Ok((text, None));
    };
    let mut quoted = false;
    for (at, c) in body.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '(' if !quoted => {
                let (send, group) = body.split_at(at);
                return Ok((send.trim(), Some(group.get(1..).unwrap_or("").trim())));
            }
            _ => {}
        }
    }
    Err(format!(
        "{text:?} ends with `)` and has no `(` to open the guards"
    ))
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.send)?;
        if self.when.is_empty() {
            return Ok(());
        }
        f.write_str(" (")?;
        for (i, condition) in self.when.iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{condition}")?;
        }
        f.write_str(")")
    }
}

/// How a step is written: a string, or a table for a held one.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Written {
    Line(String),
    Held { step: String, held: String },
}

impl Serialize for Step {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let written = match &self.held {
            Some(why) => Written::Held {
                step: self.send.clone(),
                held: why.clone(),
            },
            None => Written::Line(self.to_string()),
        };
        written.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Step {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Written::deserialize(deserializer)? {
            Written::Line(text) => Self::parse(&text).map_err(D::Error::custom),
            Written::Held { step, held } => Ok(Self::held(&step, &held)),
        }
    }
}

impl Profile {
    /// Read a profile from TOML.
    ///
    /// # Errors
    ///
    /// Not TOML, a key no table has, a value of the wrong shape, or a step
    /// with a guard Hydra does not know. The message names the key or the
    /// word.
    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    /// The profile as TOML, ready to write.
    ///
    /// # Errors
    ///
    /// A value TOML cannot hold, which no field here is.
    pub fn to_toml(&self) -> Result<String, String> {
        toml::to_string_pretty(self).map_err(|e| e.to_string())
    }

    /// What is wrong with a profile that read cleanly: a stance word the
    /// game would refuse, a target with no creature or two, a target whose
    /// routine is not written, a routine with no steps. Empty when nothing is.
    #[must_use]
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        let stances = [
            ("hunting", &self.stance.hunting),
            ("wander", &self.stance.wander),
            ("stand", &self.stance.stand),
        ];
        for (phase, word) in stances {
            if let Some(word) = word
                && let Err(why) = Want::parse(word)
            {
                out.push(format!("stance.{phase}: {why}"));
            }
        }
        for (i, target) in self.targets.iter().enumerate() {
            let n = i + 1;
            match (&target.name, target.any) {
                (Some(_), true) => {
                    out.push(format!(
                        "target {n} has both a name and any = true; one or the other"
                    ));
                }
                (None, false) => {
                    out.push(format!(
                        "target {n} names no creature: give it a name, or any = true"
                    ));
                }
                _ => {}
            }
            if !self.routines.contains_key(&target.routine) {
                out.push(format!(
                    "target {n} uses routine {:?}, which is not under [routines]",
                    target.routine
                ));
            }
        }
        for (name, steps) in &self.routines {
            if steps.is_empty() {
                out.push(format!("routine {name} has no steps"));
            }
        }
        out
    }

    /// Every held step, with where it is: `routine b, step 3`.
    pub fn held_steps(&self) -> impl Iterator<Item = (String, &Step)> {
        let routines = self
            .routines
            .iter()
            .flat_map(|(name, steps)| held_in("routine", name, steps));
        let sequences = self
            .sequences
            .iter()
            .flat_map(|(name, steps)| held_in("sequence", name, steps));
        routines.chain(sequences)
    }

    /// Sequences named and not yet written: the importer's stand-ins for
    /// `script <name>`.
    pub fn unwritten_sequences(&self) -> impl Iterator<Item = &str> {
        self.sequences
            .iter()
            .filter(|(_, steps)| steps.is_empty())
            .map(|(name, _)| name.as_str())
    }
}

/// The held steps of one list, each placed.
fn held_in<'a>(
    kind: &'a str,
    name: &'a str,
    steps: &'a [Step],
) -> impl Iterator<Item = (String, &'a Step)> {
    steps
        .iter()
        .enumerate()
        .filter(|(_, step)| step.held.is_some())
        .map(move |(i, step)| (format!("{kind} {name}, step {}", i + 1), step))
}
