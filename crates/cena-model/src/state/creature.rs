//! The bestiary: what a creature is, and what the wire says it is doing.
//!
//! Two halves that do not touch each other:
//!
//! - **[`Creature`]** -- static game knowledge for 627 creatures, ported from
//!   `lib/gemstone/creatures/` (3.3 MB of Ruby) via
//!   `tools/extract_creatures.rb`. **Everything a template says**: levels,
//!   hit points, every attack and its strength, every defense, the room UIDs
//!   each is found in, what it drops, its abilities, and every line it prints
//!   -- arrival, attack, casting, standing up, shaking off a stun, dying.
//! - **[`status::CreatureStatus`]** -- a stateless classifier over `<crtrStatus>`,
//!   which the wire sends continuously during a fight.
//!
//! `plan/13` §4a names creature templates as a port-aggressively target, for
//! the reason this data makes obvious: none of it is derivable. Somebody cast
//! Limb Disruption at 329 creatures to find out which have limbs.
//!
//! # All of it, and why that was not always so
//!
//! > **AUTHOR, 2026-09-24:** *"The reason it was all there was multiple
//! > reasons, one of which is a comprehensive beastiary, the other is the
//! > messages have uses just because they haven't been made apparent yet. For
//! > example when get to implementing kswole's behaviors that requires their
//! > casting prep line."*
//!
//! The first port (2026-09-20) kept 4 of the 13 message kinds and a subset of
//! the rest, on the grounds that data with no consumer rots. It does not: the
//! extractor regenerates it from upstream. The case the author names is exact
//! -- `kswole.lic` keeps **134 hand-written regexes** of creature casting
//! preparations (`kswole.lic:104-296`), and the bestiary's
//! [`MessageKind::SpellPrep`] is **281 lines across 169 creatures**, dropped
//! until 2026-09-24. The extractor now aborts on any key it does not read, and
//! [`load_problems`] reports any row or column this side does not.
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
//! | `boxes` | 357 | 106 | **164** |
//!
//! So [`Creature::sleepable`] is `None` for **342 of 627**. A sorcerer asking
//! "can I Sleep this" must be able to tell "measured, and no" from "nobody has
//! tried" -- the same three-valued discipline as `PsmSet::has_table` and
//! `Effects::active_in`. The treasure flags were read as plain `bool` until
//! 2026-09-24, which told a looter that 164 creatures carry no boxes when
//! nobody had looked.
//!
//! # A defense is usually a RANGE
//!
//! MEASURED: `melee` is a range 511 times, a scalar 37 times, absent 79 times.
//! Creature defenses vary by spawn, and [`Stat`] keeps that distinction --
//! collapsing a range to its midpoint would report a precision the bestiary
//! never claimed.

mod load;
mod parts;
pub mod status;

pub use parts::{Ability, Attack, AttackCategory, Entry, Message, MessageKind, Treasure};

use std::collections::BTreeMap;
use std::sync::OnceLock;

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
    /// Height in feet, where known.
    pub height: Option<i32>,
    /// Speed, as the bestiary records it.
    pub speed: Option<Stat>,
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
    /// The template's `bcs` flag, three-valued; the template does not say
    /// what it stands for.
    pub bcs: Option<bool>,
    /// Is a boss encounter.
    pub boss: bool,
    /// `pack`, `miniboss`, `boss`, where classified.
    pub boss_type: Option<String>,
    /// Extra classifications: `Living`, `Construct`, `Spirit`.
    pub other_class: Vec<String>,
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
    /// Target defense by spell circle, keyed as the bestiary keys it: the
    /// professions' `bar_td` .. `wiz_td`, and the minor and major circles
    /// `mje_td`, `mne_td`, `mjs_td`, `mns_td`, `mnm_td`.
    ///
    /// A map rather than twelve named fields: a caster wants their own
    /// circle's number, and a consumer asking for one it does not know about
    /// should get `None` rather than fail to compile. Until 2026-09-24 four
    /// of the twelve were not carried.
    pub target_defense: BTreeMap<String, Stat>,
    /// Spells it has up in its own defense.
    pub defensive_spells: Vec<String>,
    /// Defensive abilities, with the bestiary's note on each.
    pub defensive_abilities: Vec<Entry>,
    /// What it cannot be harmed by, or cannot be affected by.
    pub immunities: Vec<String>,
    /// Defenses that fit nowhere else.
    pub special_defenses: Vec<String>,
    /// What it drops.
    pub treasure: Treasure,
    /// What it is seen carrying, from its LOOK gear line.
    pub equipment: Vec<String>,
    /// Where it is found.
    pub areas: Vec<Area>,
    /// Every attack, of every category.
    pub attacks: Vec<Attack>,
    /// Notes on how it fights that fit nowhere else.
    pub special_notes: Vec<String>,
    /// Anything else the bestiary says that has no field.
    pub special_other: Option<String>,
    /// Its declared abilities.
    pub abilities: Vec<Ability>,
    /// Every line it prints, by kind.
    pub messages: BTreeMap<MessageKind, Vec<Message>>,
    /// Tips written about it: `general`, `miscellany`, or a profession.
    pub info: BTreeMap<String, Vec<String>>,
    /// Its wiki page.
    pub url: Option<String>,
    /// A picture of it, where the bestiary links one.
    pub picture: Option<String>,
    /// The template schema this creature was written against.
    pub schema_version: Option<u32>,
}

impl Creature {
    /// The lines this creature prints for one kind of event.
    #[must_use]
    pub fn messages_of(&self, kind: MessageKind) -> &[Message] {
        self.messages.get(&kind).map_or(&[], Vec::as_slice)
    }

    /// What LOOK shows, where the bestiary records it.
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.messages_of(MessageKind::Description)
            .first()
            .map(|message| message.text.as_str())
    }

    /// The tips written for one section: `general`, `miscellany`, or a
    /// profession such as `wizard`.
    #[must_use]
    pub fn tips(&self, section: &str) -> &[String] {
        self.info.get(section).map_or(&[], Vec::as_slice)
    }

    /// Is this creature found in the given room?
    #[must_use]
    pub fn found_at(&self, room_uid: u32) -> bool {
        self.areas.iter().any(|area| area.contains(room_uid))
    }

    /// The highest attack strength any of its attacks reaches, physical or
    /// bolt.
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

fn bestiary() -> &'static load::Bestiary {
    static BESTIARY: OnceLock<load::Bestiary> = OnceLock::new();
    BESTIARY.get_or_init(load::build)
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

/// How many attack and casting strengths were kept verbatim because the
/// source is malformed.
///
/// Fourteen, across every attack category. Exposed so a caller can report
/// the shortfall rather than silently reading `None` (Rule 2.2).
#[must_use]
pub fn unparsed_attack_strengths() -> usize {
    bestiary().unparsed_strengths
}

/// Everything in the shipped tables this side did not read: a column no
/// field takes, a row whose kind, category or list is unknown, a row naming
/// a creature that does not exist.
///
/// **Empty, and a test holds it there.** The first port shipped `height`,
/// `speed` and `bcs` in `creatures.tsv` and never read them; a non-empty
/// list is that failure, reported instead of silent.
#[must_use]
pub fn load_problems() -> &'static [String] {
    &bestiary().problems
}
