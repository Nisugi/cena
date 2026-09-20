//! The bestiary: what a creature is, and what the wire says it is doing.
//!
//! Two halves that do not touch each other:
//!
//! - **[`Creature`]** -- static game knowledge for 627 creatures, ported from
//!   `lib/gemstone/creatures/` (3.3 MB of Ruby) via
//!   `tools/extract_creatures.rb`. Levels, hit points, AS and TD ranges, the
//!   room UIDs each is found in, what it skins into, and what it says when it
//!   dies.
//! - **[`status::CreatureStatus`]** -- a stateless classifier over `<crtrStatus>`,
//!   which the wire sends continuously during a fight.
//!
//! `plan/13` §4a names creature templates as a port-aggressively target, for
//! the reason this data makes obvious: none of it is derivable. Somebody cast
//! Limb Disruption at 329 creatures to find out which have limbs.
//!
//! # NOT ported: the live registry
//!
//! `creature_base.rb` (748 lines) and `CreatureInstance` (`creature.rb:322-710`)
//! track per-fight state -- accumulated damage, stun estimates, amputated body
//! parts, a room roster with eviction and housekeeping. That is a **stateful
//! consumer** in `plan/12` §3a's sense, and it overlaps the combat tracker the
//! author deferred. What lives here is the classifier it would sit on.
//!
//! # The tri-state flags are measured facts, and absence is a third answer
//!
//! `_creature_template.rb` is explicit: *"true/false/nil if unknown"*, and for
//! `limbs`, *"'has no limbs left!' after repeated casts = true; a refusal on
//! the very first cast = false"*. Those were measured in game, one creature at
//! a time.
//!
//! MEASURED across the 627 templates:
//!
//! | Flag | true | false | **unknown** |
//! |---|---|---|---|
//! | `sleepable` | 127 | 158 | **342** |
//! | `limbs` | 328 | 1 | **298** |
//! | `blood` | 320 | 146 | **161** |
//!
//! So [`Creature::sleepable`] is `None` for **342 of 627**. A sorcerer asking
//! "can I Sleep this" must be able to tell "measured, and no" from "nobody has
//! tried" -- the same three-valued discipline as `PsmSet::has_table` and
//! `Effects::active_in`.
//!
//! # A defense is usually a RANGE
//!
//! MEASURED: `melee` is a range 511 times, a scalar 37 times, absent 79 times.
//! Creature defenses vary by spawn, and [`Stat`] keeps that distinction --
//! collapsing a range to its midpoint would report a precision the bestiary
//! never claimed.

pub mod status;

use std::collections::BTreeMap;
use std::sync::OnceLock;

/// The transcribed bestiary.
const CREATURES_TSV: &str = include_str!("../../data/creatures.tsv");
/// Where each creature is found, as room UID ranges.
const AREAS_TSV: &str = include_str!("../../data/creature_areas.tsv");
/// Each creature's physical attacks.
const ATTACKS_TSV: &str = include_str!("../../data/creature_attacks.tsv");
/// Death, flee, arrival and decay lines.
const MESSAGES_TSV: &str = include_str!("../../data/creature_messages.tsv");

/// A number the bestiary states as a range, a scalar, or not at all.
///
/// `(106..116)` in the Ruby. A creature's attack strength and defenses vary
/// between spawns, and the templates record the observed span; a few are known
/// exactly, and many are not recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stat {
    /// Known exactly.
    Exact(i32),
    /// Known to fall in this span, inclusive.
    Range(i32, i32),
}

impl Stat {
    /// The lowest value this stat takes.
    #[must_use]
    pub const fn low(self) -> i32 {
        match self {
            Self::Exact(value) | Self::Range(value, _) => value,
        }
    }

    /// The highest value this stat takes.
    #[must_use]
    pub const fn high(self) -> i32 {
        match self {
            Self::Exact(value) | Self::Range(_, value) => value,
        }
    }

    /// Is this stat known to vary?
    #[must_use]
    pub const fn is_range(self) -> bool {
        matches!(self, Self::Range(..))
    }

    /// Parse the TSV spelling: `120`, `106..116`, or empty.
    fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        if let Some((lo, hi)) = text.split_once("..") {
            return Some(Self::Range(
                lo.trim().parse().ok()?,
                hi.trim().parse().ok()?,
            ));
        }
        Some(Self::Exact(text.parse().ok()?))
    }
}

/// Parse a tri-state flag: `true`, `false`, or empty for **unknown**.
fn tri(text: &str) -> Option<bool> {
    match text.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Where a creature is found: a named area and one span of room UIDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Area {
    /// The hunting ground's name, e.g. `The Graveyard`.
    pub name: String,
    /// The first room UID in this span.
    pub uid_low: u32,
    /// The last room UID in this span, inclusive.
    pub uid_high: u32,
}

impl Area {
    /// Does this span contain the given room UID?
    #[must_use]
    pub const fn contains(&self, uid: u32) -> bool {
        self.uid_low <= uid && uid <= self.uid_high
    }
}

/// One of a creature's physical attacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attack {
    /// What the attack is called: `Claw`, `Pincer`, `Bite`.
    pub name: String,
    /// Its attack strength, where the bestiary records a usable number.
    pub attack_strength: Option<Stat>,
    /// The AS field verbatim, when it could not be parsed.
    ///
    /// **Six attacks across the 627 templates have malformed AS values** --
    /// `"566 to"`, `"(lunge) 245-276"`, `"390 UAF"`, `""`. That is data-entry
    /// damage in Lich's bestiary rather than a shape worth modelling, so the
    /// text is kept and the number is `None`. Rule 2.2: nothing is dropped
    /// without saying so, and [`unparsed_attack_strengths`] counts them.
    pub attack_strength_raw: Option<String>,
    /// `slash`, `crush`, `puncture`, where recorded.
    pub damage_type: Option<String>,
}

/// What a creature drops when it dies.
///
/// Its own type rather than four fields on [`Creature`]: these are one
/// question -- *is this worth killing for loot* -- and a looting consumer
/// wants them together. `Treasure` is also the name Lich gives the same group
/// (`creature.rb:816-840`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Treasure {
    /// What it skins into, where it skins.
    ///
    /// `None` means it does not skin. MEASURED: 308 of 627 templates record a
    /// skin.
    pub skin: Option<String>,
    /// Drops coins.
    pub coins: bool,
    /// Drops boxes.
    pub boxes: bool,
    /// Drops gems.
    pub gems: bool,
}

impl Treasure {
    /// Does it drop anything at all worth stopping for?
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.skin.is_none() && !self.coins && !self.boxes && !self.gems
    }
}

/// What kind of line a creature message is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MessageKind {
    /// What the room prints when it dies.
    Death,
    /// What it prints when it runs.
    Flee,
    /// What it prints when it arrives.
    Arrival,
    /// What it prints when the corpse decays.
    Decay,
}

impl MessageKind {
    /// Every kind.
    pub const ALL: [Self; 4] = [Self::Death, Self::Flee, Self::Arrival, Self::Decay];

    /// The TSV spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Death => "death",
            Self::Flee => "flee",
            Self::Arrival => "arrival",
            Self::Decay => "decay",
        }
    }

    /// Parse the TSV spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == text)
    }
}

/// What the bestiary knows about one creature.
///
/// Every field but [`Self::id`] and [`Self::name`] may be absent: the
/// templates are a community catalogue, filled in as people measure things.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Creature {
    /// The template's file stem: `albino_tomb_spider`.
    pub id: String,
    /// The display name: `albino tomb spider`.
    pub name: String,
    /// The wire noun a command targets it by: `spider`.
    pub noun: Option<String>,
    /// Creature level, where known.
    pub level: Option<i32>,
    /// Typical maximum hit points.
    pub max_hp: Option<Stat>,
    /// `Arachnid`, `Bear`, `Gigas`.
    pub family: Option<String>,
    /// `Quadruped`, `Biped`, `Ooze`.
    pub kind: Option<String>,
    /// `small`, `medium`, `large`, `huge`.
    pub size: Option<String>,
    /// Is it undead?
    pub undead: Option<bool>,
    /// Corporeal, flesh and blood.
    pub blood: Option<bool>,
    /// Has a skeletal structure.
    pub bones: Option<bool>,
    /// Has limbs that Limb Disruption (708) can target.
    pub limbs: Option<bool>,
    /// Has a body that Wither (1115) can attack.
    pub witherable: Option<bool>,
    /// Can be affected by Sympathy (1120).
    pub sympathy: Option<bool>,
    /// Carries coin worth mugging.
    pub muggable: Option<bool>,
    /// Can be put to sleep. **Unknown for 342 of 627.**
    pub sleepable: Option<bool>,
    /// Is a boss encounter.
    pub boss: bool,
    /// `pack`, `miniboss`, `boss`, where classified.
    pub boss_type: Option<String>,
    /// Armor sub-group as the bestiary writes it: `12N`, `1`, `8N`.
    ///
    /// A `String` rather than a number because the `N` suffix is part of the
    /// value and 167 templates record none at all.
    pub asg: Option<String>,
    /// Defensive strength against melee.
    pub melee_ds: Option<Stat>,
    /// Defensive strength against ranged.
    pub ranged_ds: Option<Stat>,
    /// Defensive strength against bolts.
    pub bolt_ds: Option<Stat>,
    /// Unarmed defense factor.
    pub udf: Option<Stat>,
    /// Target defense by caster profession, keyed `bar_td`, `cle_td`, ...
    ///
    /// A map rather than eight named fields: a caster wants their own circle's
    /// number, and a consumer asking for one it does not know about should get
    /// `None` rather than fail to compile.
    pub target_defense: BTreeMap<String, Stat>,
    /// What it drops.
    pub treasure: Treasure,
    /// Where it is found.
    pub areas: Vec<Area>,
    /// Its physical attacks.
    pub attacks: Vec<Attack>,
    /// Its death, flee, arrival and decay lines.
    pub messages: BTreeMap<MessageKind, Vec<String>>,
}

impl Creature {
    /// The lines this creature prints for one kind of event.
    #[must_use]
    pub fn messages_of(&self, kind: MessageKind) -> &[String] {
        self.messages.get(&kind).map_or(&[], Vec::as_slice)
    }

    /// Is this creature found in the given room?
    #[must_use]
    pub fn found_at(&self, room_uid: u32) -> bool {
        self.areas.iter().any(|area| area.contains(room_uid))
    }

    /// The highest attack strength any of its attacks reaches.
    ///
    /// What a defensive check wants: the worst case, not an average. `None`
    /// when no attack has a usable number.
    #[must_use]
    pub fn max_attack_strength(&self) -> Option<i32> {
        self.attacks
            .iter()
            .filter_map(|attack| attack.attack_strength)
            .map(Stat::high)
            .max()
    }
}

/// The loaded bestiary.
struct Bestiary {
    /// Keyed by template id.
    by_id: BTreeMap<String, Creature>,
    /// Malformed AS values kept verbatim rather than dropped.
    unparsed_as: usize,
}

fn bestiary() -> &'static Bestiary {
    static BESTIARY: OnceLock<Bestiary> = OnceLock::new();
    BESTIARY.get_or_init(build_bestiary)
}

/// Iterate a TSV's data rows as already-split columns.
fn rows(tsv: &str) -> impl Iterator<Item = Vec<&str>> {
    tsv.lines()
        .skip(1)
        .filter(|line| !line.is_empty())
        .map(|line| line.split('\t').collect())
}

/// Read a column, treating out-of-range and empty alike as absent.
fn column<'a>(cols: &[&'a str], index: usize) -> Option<&'a str> {
    cols.get(index)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
}

fn build_bestiary() -> Bestiary {
    let mut by_id: BTreeMap<String, Creature> = BTreeMap::new();

    for cols in rows(CREATURES_TSV) {
        let (Some(id), Some(name)) = (column(&cols, 0), column(&cols, 1)) else {
            continue;
        };
        let mut target_defense = BTreeMap::new();
        // Columns 26..=33, in the header's order.
        for (offset, key) in [
            "bar_td", "cle_td", "emp_td", "pal_td", "ran_td", "sor_td", "wiz_td", "mjs_td",
        ]
        .into_iter()
        .enumerate()
        {
            if let Some(stat) = column(&cols, 26 + offset).and_then(Stat::parse) {
                target_defense.insert(key.to_owned(), stat);
            }
        }

        by_id.insert(
            id.to_owned(),
            Creature {
                id: id.to_owned(),
                name: name.to_owned(),
                noun: column(&cols, 2).map(str::to_owned),
                level: column(&cols, 3).and_then(|v| v.parse().ok()),
                max_hp: column(&cols, 4).and_then(Stat::parse),
                family: column(&cols, 5).map(str::to_owned),
                kind: column(&cols, 6).map(str::to_owned),
                size: column(&cols, 7).map(str::to_owned),
                undead: cols.get(10).copied().and_then(tri),
                blood: cols.get(11).copied().and_then(tri),
                bones: cols.get(12).copied().and_then(tri),
                limbs: cols.get(13).copied().and_then(tri),
                witherable: cols.get(14).copied().and_then(tri),
                sympathy: cols.get(15).copied().and_then(tri),
                muggable: cols.get(16).copied().and_then(tri),
                sleepable: cols.get(17).copied().and_then(tri),
                boss: column(&cols, 19) == Some("true"),
                boss_type: column(&cols, 20).map(str::to_owned),
                asg: column(&cols, 21).map(str::to_owned),
                melee_ds: column(&cols, 22).and_then(Stat::parse),
                ranged_ds: column(&cols, 23).and_then(Stat::parse),
                bolt_ds: column(&cols, 24).and_then(Stat::parse),
                udf: column(&cols, 25).and_then(Stat::parse),
                target_defense,
                treasure: Treasure {
                    skin: column(&cols, 34).map(str::to_owned),
                    coins: column(&cols, 35) == Some("true"),
                    boxes: column(&cols, 36) == Some("true"),
                    gems: column(&cols, 37) == Some("true"),
                },
                ..Creature::default()
            },
        );
    }

    join_areas(&mut by_id);
    let unparsed_as = join_attacks(&mut by_id);
    join_messages(&mut by_id);

    Bestiary { by_id, unparsed_as }
}

/// Join `creature_areas.tsv` onto the loaded creatures.
fn join_areas(by_id: &mut BTreeMap<String, Creature>) {
    for cols in rows(AREAS_TSV) {
        let (Some(id), Some(name), Some(lo), Some(hi)) = (
            column(&cols, 0),
            column(&cols, 1),
            column(&cols, 2).and_then(|v| v.parse().ok()),
            column(&cols, 3).and_then(|v| v.parse().ok()),
        ) else {
            continue;
        };
        if let Some(creature) = by_id.get_mut(id) {
            creature.areas.push(Area {
                name: name.to_owned(),
                uid_low: lo,
                uid_high: hi,
            });
        }
    }
}

/// Join `creature_attacks.tsv`, returning how many AS values were unparsable.
fn join_attacks(by_id: &mut BTreeMap<String, Creature>) -> usize {
    let mut unparsed_as = 0;
    for cols in rows(ATTACKS_TSV) {
        let (Some(id), Some(name)) = (column(&cols, 0), column(&cols, 1)) else {
            continue;
        };
        let raw = column(&cols, 3).map(str::to_owned);
        if raw.is_some() {
            unparsed_as += 1;
        }
        if let Some(creature) = by_id.get_mut(id) {
            creature.attacks.push(Attack {
                name: name.to_owned(),
                attack_strength: column(&cols, 2).and_then(Stat::parse),
                attack_strength_raw: raw,
                damage_type: column(&cols, 4).map(str::to_owned),
            });
        }
    }

    unparsed_as
}

/// Join `creature_messages.tsv` onto the loaded creatures.
fn join_messages(by_id: &mut BTreeMap<String, Creature>) {
    for cols in rows(MESSAGES_TSV) {
        let (Some(id), Some(kind), Some(text)) = (
            column(&cols, 0),
            column(&cols, 1).and_then(MessageKind::parse),
            column(&cols, 2),
        ) else {
            continue;
        };
        if let Some(creature) = by_id.get_mut(id) {
            creature
                .messages
                .entry(kind)
                .or_default()
                .push(text.to_owned());
        }
    }
}

/// Look up one creature by its template id: `albino_tomb_spider`.
#[must_use]
pub fn creature(id: &str) -> Option<&'static Creature> {
    bestiary().by_id.get(id)
}

/// Every creature in the bestiary, ordered by id.
pub fn creatures() -> impl Iterator<Item = &'static Creature> {
    bestiary().by_id.values()
}

/// Find creatures by display name, case-insensitively.
///
/// Returns every match rather than the first: names are not unique keys, and a
/// caller told there is one answer when there are two gets the wrong stats.
/// Same reasoning as `resolve_noun` and the armament aliases.
#[must_use]
pub fn by_name(name: &str) -> Vec<&'static Creature> {
    let wanted = name.trim();
    creatures()
        .filter(|c| c.name.eq_ignore_ascii_case(wanted))
        .collect()
}

/// Find creatures by the wire noun they are targeted with.
///
/// **A noun is emphatically not unique** -- `spider` names many creatures at
/// many levels -- so this returns all of them and a caller that needs one
/// narrows by room (see [`in_room`]).
#[must_use]
pub fn by_noun(noun: &str) -> Vec<&'static Creature> {
    let wanted = noun.trim();
    creatures()
        .filter(|c| {
            c.noun
                .as_deref()
                .is_some_and(|n| n.eq_ignore_ascii_case(wanted))
        })
        .collect()
}

/// Every creature the bestiary places in the given room.
#[must_use]
pub fn in_room(room_uid: u32) -> Vec<&'static Creature> {
    creatures().filter(|c| c.found_at(room_uid)).collect()
}

/// How many attack-strength values were kept verbatim because the source is
/// malformed.
///
/// Six, at the time of the port. Exposed so a caller can report the shortfall
/// rather than silently reading `None` (Rule 2.2).
#[must_use]
pub fn unparsed_attack_strengths() -> usize {
    bestiary().unparsed_as
}
