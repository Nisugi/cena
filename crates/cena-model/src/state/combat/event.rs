//! What a chunk of combat yields: attack events and the facts beside them.
//!
//! The shape is Lich's `:attack` payload (`processor.rb`, the `current_event`
//! hash) and its per-fact emits (`:status`, `:spell_loss`, `:ucs`), typed.
//! A recorder or a highlighter reads these; nothing here is a creature's
//! running state -- that is the registry's, applied from these.
//!
//! # Lineage is by index into the chunk's event list
//!
//! Lich links a blob's spawn tree with object references (`root_ref`,
//! `parent_ref`) and resolves them to per-chunk uids at emit time. Here an
//! event's [`AttackEvent::root`] and [`AttackEvent::parent`] are indices into
//! the same [`ChunkFacts::events`] list, resolved the same way: a reference to
//! an event that was not emitted degrades to self-root / no parent, exactly as
//! `process` does (`processor.rb:139-147`).

use super::attack::AmbushKind;
use super::outcome::OutcomeKind;
use super::resolution::Resolution;
use super::status::{StatusAction, StatusName};
use super::target::Actor;
use super::ucs::{PositionTier, UcsAttack};
use crate::crit::{CritEntry, Location, Position, SecondaryWound};

/// Who an event was aimed at.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum EventTarget {
    /// A creature, by link.
    Creature(Actor),
    /// A player or an unresolvable name: bound, and never to adopt a creature.
    Foreign(String),
    /// Nobody, or us.
    #[default]
    None,
}

impl EventTarget {
    /// The creature's `exist`, when the target is a creature.
    #[must_use]
    pub fn id(&self) -> Option<i64> {
        match self {
            Self::Creature(a) => a.id,
            _ => None,
        }
    }

    /// The creature, when the target is one.
    #[must_use]
    pub const fn creature(&self) -> Option<&Actor> {
        match self {
            Self::Creature(a) => Some(a),
            _ => None,
        }
    }
}

/// A critical, as recorded on the hit that produced it.
///
/// The crit table entry's facts without its pattern: Lich keeps *"the whole
/// `CritRanks` hash ... `:regex` is dropped"* (`processor.rb:1697-1707`), and
/// this is that. The coup de grace has no table row -- its success line IS
/// the killing blow -- and is recorded as a synthetic fatal at the struck
/// location (owner ruling 2026-09-07).
// `struct_excessive_bools` is right that this usually means a missing
// enum. These are Lich's flags, each an independent fact a recorder
// reads by name; an enum would encode a hierarchy the game does not have.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crit {
    /// Where it landed.
    pub location: Location,
    /// The table's damage type, or `None` for the coup's synthetic crit.
    pub damage_type: Option<crate::crit::DamageType>,
    /// Severity 0..=9 (`None` for the coup).
    pub rank: Option<u8>,
    /// The wound left, 0..=3 (`None` for the coup).
    pub wound_rank: Option<u8>,
    /// The target died of this.
    pub fatal: bool,
    /// A limb came off.
    pub amputated: bool,
    /// Rounds of stun; see `CritEntry::stun_is_known`.
    pub stunned: u16,
    /// Extra roundtime, seconds.
    pub roundtime: u8,
    /// Knocked to a floor position.
    pub position: Option<Position>,
    /// Silenced.
    pub silenced: bool,
    /// Slowed.
    pub slowed: bool,
    /// Dazed.
    pub dazed: bool,
    /// Put to sleep.
    pub sleeping: bool,
    /// Crippled.
    pub crippled: bool,
    /// Favouring a limb.
    pub limb_favored: bool,
    /// A second location wounded by the same crit.
    pub secondary_wound: Option<SecondaryWound>,
    /// Where the crit text sat in the chunk: crit statuses are applied after
    /// the whole chunk is parsed, so this is the only record of where the
    /// crit stood relative to the messages around it (`position_recovered?`).
    pub line: usize,
}

impl Crit {
    /// From a crit table entry, at the line it matched.
    #[must_use]
    pub fn from_entry(entry: &CritEntry, line: usize) -> Self {
        Self {
            location: entry.location,
            damage_type: Some(entry.damage_type),
            rank: Some(entry.rank),
            wound_rank: Some(entry.wound_rank),
            fatal: entry.fatal,
            amputated: entry.amputated,
            stunned: entry.stunned,
            roundtime: entry.roundtime,
            position: entry.position,
            silenced: entry.silenced,
            slowed: entry.slowed,
            dazed: entry.dazed,
            sleeping: entry.sleeping,
            crippled: entry.crippled,
            limb_favored: entry.limb_favored,
            secondary_wound: entry.secondary_wound,
            line,
        }
    }

    /// The coup de grace's synthetic fatal crit.
    #[must_use]
    pub fn coup_de_grace(location: Location, line: usize) -> Self {
        Self {
            location,
            damage_type: None,
            rank: None,
            wound_rank: None,
            fatal: true,
            amputated: false,
            stunned: 0,
            roundtime: 0,
            position: None,
            silenced: false,
            slowed: false,
            dazed: false,
            sleeping: false,
            crippled: false,
            limb_favored: false,
            secondary_wound: None,
            line,
        }
    }

    /// Is this the coup's synthetic crit rather than a table row?
    #[must_use]
    pub const fn is_coup_de_grace(&self) -> bool {
        self.damage_type.is_none()
    }
}

/// One landed hit: its damage, bound to the crit it produced.
///
/// *"ONE record per landed hit, damage bound to the crit it produced.
/// Parallel arrays could not express the pairing: their counts differ on
/// 28.7% of events and 48.5% of flares"* (`processor.rb:1674-1679`). `crit`
/// stays `None` when the lookahead finds none -- which IS the concussion
/// marker: holy fire prints *"ravaged for 65"* then *"... 5 points of
/// damage!"*, and only the 5 carries the fire crit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// Points of damage. Zero for the coup de grace.
    pub damage: u32,
    /// The crit this damage produced, if a table row matched within three
    /// lines.
    pub crit: Option<Crit>,
    /// The damage line's position in the chunk.
    pub line: usize,
}

/// A flare that rode an attack, with what it did.
#[allow(clippy::struct_excessive_bools)] // see `Crit`
#[derive(Debug, Clone, PartialEq)]
pub struct FlareEvent {
    /// The def name.
    pub name: String,
    /// A damage line is expected to follow.
    pub damaging: bool,
    /// May strike more than one target.
    pub aoe: bool,
    /// Casts an imbedded spell as a separate attack.
    pub spawns: bool,
    /// The creature the announce line named, when it named one -- an `AoE`
    /// flare can strike a different creature than the swing.
    pub target: Option<Actor>,
    /// The flaring weapon, when the line linked it.
    pub weapon: Option<Actor>,
    /// Whose item flared, for a third-person form. `None` is ours.
    pub attacker: Option<Actor>,
    /// Damage it dealt.
    pub hits: Vec<Hit>,
    /// Outcomes on its own roll.
    pub outcomes: Vec<OutcomeKind>,
    /// Its own roll lines.
    pub resolutions: Vec<Resolution>,
    /// Fired BEFORE the swing line (dispel-on-nock, ensorcell's veil): never
    /// the cause of a status the swing's own crit inflicts afterwards.
    pub pre: bool,
    /// The line it announced on.
    pub line: usize,
    /// Carried over from the previous chunk unclaimed.
    pub(super) held: bool,
    /// A spell-releasing flare whose one spell has been claimed.
    pub(super) release_claimed: bool,
}

impl FlareEvent {
    /// Our own item fired.
    #[must_use]
    pub const fn is_ours(&self) -> bool {
        self.attacker.is_none()
    }
}

/// A guardian took the hit meant for someone else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redirect {
    /// Who stepped in.
    pub interceptor: Actor,
    /// The intended victim's noun.
    pub intended: String,
    /// The attack actually resolved against the guardian. `false` for the
    /// UAC shape, where the announce follows the attack line and the roll
    /// still lands on the intended creature (`attacks.rb` corpus: 21 of 130).
    pub honored: bool,
}

/// The flare an event was spawned by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParentFlare {
    /// The flare's name.
    pub flare: String,
    /// The weapon that flared, when linked.
    pub weapon: Option<Actor>,
}

/// How sure the lineage is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    /// The game declared it: blink's bracketed cast.
    Bracket,
    /// Forced by a count constraint: N echo flares, N echo swings; one
    /// releasing flare, one released spell.
    Count,
}

/// One attack, assembled from its lines.
#[allow(clippy::struct_excessive_bools)] // see `Crit`
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AttackEvent {
    /// The def name, or `unknown` for an orphan.
    pub name: String,
    /// Who was attacked.
    pub target: EventTarget,
    /// Who attacked, when the line says. `None` for our own.
    pub attacker: Option<Actor>,
    /// Aimed at us.
    pub inbound: bool,
    /// A nearby player's attack.
    pub foreign_caster: bool,
    /// An effect tick with no owning cast: the creature's damage is real,
    /// but it is not our deal.
    pub unowned: bool,
    /// The orphan sink: facts whose initiation had no def.
    pub orphan: bool,
    /// Struck from hiding.
    pub ambush: bool,
    /// Which hiding maneuver, or a reaction.
    pub attack_kind: Option<AmbushKind>,
    /// A guardian redirect.
    pub redirect: Option<Redirect>,
    /// An aimed shot.
    pub aimed: bool,
    /// The weapon, from the swing line or the def.
    pub weapon: Option<String>,
    /// Opened by a bare `cast` gesture the spell line superseded.
    pub via_cast: bool,
    /// The flare that spawned it.
    pub parent_flare: Option<ParentFlare>,
    /// This chunk's tree root: an index into the chunk's events, self for a
    /// root. Resolved at emit.
    pub root: Option<usize>,
    /// The immediate spawner, only when asserted.
    pub parent: Option<usize>,
    /// How the parent was established.
    pub parent_confidence: Option<Confidence>,
    /// Damage dealt.
    pub hits: Vec<Hit>,
    /// Flares that rode it.
    pub flares: Vec<FlareEvent>,
    /// How its swings resolved.
    pub outcomes: Vec<OutcomeKind>,
    /// Its roll lines.
    pub resolutions: Vec<Resolution>,
    /// The chunk's prompt time, server epoch seconds.
    pub at: Option<u32>,
    /// Born from a real initiation line (`_attack_born`): multi-strike
    /// rolls keep attaching after outcomes and damage.
    pub(super) attack_born: bool,
    /// A status line applied to this event's target (`_had_status`): a
    /// per-target line whose only payload is the status keeps its event.
    pub(super) had_status: bool,
    /// The chunk line the target switcher created it on (`_line`), so the
    /// attack branch can tell a same-line artifact from a real event.
    pub(super) born_line: Option<usize>,
    /// Carried over from the previous chunk as a held cast (`_held`).
    pub(super) held: bool,
    /// A spell a flare released (`_released`).
    pub(super) released: bool,
}

impl AttackEvent {
    /// Was it born from a real initiation line, rather than by the target
    /// switcher, an outcome, or the orphan sink?
    #[must_use]
    pub const fn is_attack_born(&self) -> bool {
        self.attack_born
    }

    /// Is it a spell that one of our own flares released?
    #[must_use]
    pub const fn is_released(&self) -> bool {
        self.released
    }

    /// The def named a target that is not a creature.
    #[must_use]
    pub const fn is_foreign_target(&self) -> bool {
        matches!(self.target, EventTarget::Foreign(_))
    }

    /// Our own outbound attack: not inbound, not a nearby player's, not on a
    /// foreign target, not an unowned tick, not the orphan sink.
    ///
    /// The recorder's `ours` column, decided here rather than at write time.
    #[must_use]
    pub const fn is_ours(&self) -> bool {
        !self.inbound
            && !self.foreign_caster
            && !self.is_foreign_target()
            && !self.unowned
            && !self.orphan
    }

    /// Every hit, the attack's own and its flares'.
    pub fn all_hits(&self) -> impl Iterator<Item = &Hit> {
        self.hits
            .iter()
            .chain(self.flares.iter().flat_map(|f| f.hits.iter()))
    }

    /// Total damage across the attack and its flares.
    #[must_use]
    pub fn total_damage(&self) -> u32 {
        self.all_hits().map(|h| h.damage).sum()
    }
}

/// Who a status fact is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subject {
    /// A creature, by id and name.
    Creature(Actor),
    /// Us: a second-person line (`You are stunned!`).
    Us,
}

/// Why a spell left its subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LossCause {
    /// A dispel-family flare struck this chunk.
    Dispel,
    /// The subject died this chunk: death cleanup, not expiry.
    Death,
}

/// A fact recognised while parsing, beside the attacks.
///
/// Lich emits these on its event board (`emit_fact`) **after** the chunk's
/// attacks, so a recorder keying on the open attack files them correctly
/// (`processor.rb:158-166`). Here they are simply listed after
/// [`ChunkFacts::events`], and each carries the event it rode, when it rode
/// one.
#[derive(Debug, Clone, PartialEq)]
pub enum Fact {
    /// A status began or ended.
    Status {
        /// Who.
        subject: Subject,
        /// Which status.
        status: StatusName,
        /// Began or ended.
        action: StatusAction,
        /// The event it rode, when that event touched the subject.
        event: Option<usize>,
        /// The 1-based position of the flare it rode on that event.
        flare_seq: Option<usize>,
        /// Where the line sat in the chunk.
        line: usize,
    },
    /// A spell wore off.
    SpellLoss {
        /// Who lost it: creature, player, or bare name.
        subject: Actor,
        /// The spell number, `None` for the generic wear-off.
        spell: Option<u16>,
        /// Its name.
        spell_name: String,
        /// Why, when the chunk says.
        cause: Option<LossCause>,
    },
    /// A UCS fact.
    Ucs {
        /// The creature.
        creature: Actor,
        /// Which.
        kind: UcsKind,
    },
    /// A creature a room refresh confirmed dead after an event touched it.
    Dead {
        /// The creature.
        creature: Actor,
    },
}

/// A UCS fact's payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UcsKind {
    /// Our tier against it.
    Position(PositionTier),
    /// Its tier against us.
    PositionInbound(PositionTier),
    /// A followup opened.
    Tierup(UcsAttack),
    /// Smite applied.
    SmiteOn,
    /// Smite gone.
    SmiteOff,
}

/// Everything one chunk yielded.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChunkFacts {
    /// The attack events, in emit order.
    pub events: Vec<AttackEvent>,
    /// The facts beside them.
    pub facts: Vec<Fact>,
}

impl ChunkFacts {
    /// Nothing at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty() && self.facts.is_empty()
    }
}
