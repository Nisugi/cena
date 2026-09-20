//! The chunk-local state of the state machine, and its predicates.
//!
//! `parse_events` keeps ~26 locals across its 1,100 lines. They are named
//! here as the fields of one struct, with the comment Lich gave each one
//! condensed to its reason; the branches in the sibling modules read and
//! write them through `&mut Cursor`.
//!
//! # Events live in an arena, not in the emit list
//!
//! Lich links events by object identity: `interrupted_own`, `spawn_root`,
//! `root_ref`, `pending_echoes[].owner` all *are* the hash they point to, and
//! `save_event` is `events.any? { |e| e.equal?(ev) }`. Here every event ever
//! opened in the chunk goes into [`Cursor::pool`] and is named by its index
//! ([`EvId`]); the emit order is a separate list of ids. A superseded cast or
//! a switch artifact stays in the pool unemitted, exactly as Lich's
//! unreferenced hash is dropped -- and a lineage reference to it resolves to
//! nothing at emit, as `process` does (`processor.rb:139-147`).

use std::collections::{BTreeMap, VecDeque};

use crate::state::chunks::ChunkLine;
use crate::state::combat::attack::AmbushPrefix;
use crate::state::combat::bracket::SequenceName;
use crate::state::combat::event::{AttackEvent, EventTarget, Fact, FlareEvent, Hit, Redirect};
use crate::state::combat::outcome::OutcomeKind;
use crate::state::combat::resolution::Resolution;
use crate::state::combat::target::Actor;
use crate::state::combat::tracker::CombatTracker;

use super::SPELL_RELEASING_FLARES;

/// An event's index in [`Cursor::pool`].
pub(super) type EvId = usize;

/// Where a flare sits: on an event, or in the pending (pre-flare) list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FlareRef {
    /// The `n`th flare of a pooled event.
    OnEvent(EvId, usize),
    /// The `n`th pending pre-flare.
    Pending(usize),
}

/// Where a roll or outcome lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Sink {
    /// On an event.
    Event(EvId),
    /// On a flare.
    Flare(FlareRef),
}

/// A spawn-class flare whose bracketed sequence is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ActiveSpawn {
    pub flare: String,
    pub sequence: SequenceName,
    pub weapon: Option<Actor>,
}

/// An echo flare whose spawned swing has not arrived yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Echo {
    pub flare: String,
    pub weapon: Option<Actor>,
    pub owner: Option<EvId>,
}

/// A spell-releasing flare's swing, waiting for the spell it released.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReleasedParent {
    pub event: EvId,
    pub flare: String,
    pub weapon: Option<Actor>,
}

/// The state machine's working state for one chunk.
pub(super) struct Cursor<'a> {
    pub tracker: &'a mut CombatTracker,
    pub lines: &'a [ChunkLine],
    pub at: Option<u32>,
    /// Every event opened this chunk.
    pub pool: Vec<AttackEvent>,
    /// The emit order, by pool id.
    pub events: Vec<EvId>,
    /// Casts a swing adopted after they were saved: re-emitted right after
    /// the swing (`_released_children`).
    pub released_children: BTreeMap<EvId, Vec<EvId>>,
    /// Facts as they were recognised; `Fact::Status::event` holds a pool id
    /// until the emit remaps it.
    pub facts: Vec<Fact>,
    /// The event being assembled.
    pub current: Option<EvId>,
    /// `parse_state == :seeking_damage`.
    pub seeking_damage: bool,
    pub current_target: Option<Actor>,
    /// Our own event an inbound attack cut into mid-swing.
    pub interrupted_own: Option<EvId>,
    /// The swing a flare-released spell cut into.
    pub released_parent: Option<ReleasedParent>,
    /// A cast that printed BEFORE the swing whose flare released it.
    pub pending_release_cast: Option<EvId>,
    /// Casts deferred behind their swing; flushed at the end.
    pub deferred_casts: Vec<EvId>,
    /// Attacker of the last inbound event saved this chunk.
    pub last_inbound_attacker: Option<Actor>,
    /// The event a one-hit side effect cut in front of.
    pub single_hit_parent: Option<EvId>,
    /// The bow a nock line named, until the swing that follows.
    pub nocked: Option<String>,
    /// The flare whose damage and crit lines are arriving.
    pub flare_ctx: Option<FlareRef>,
    /// Flares seen before the swing they belong to.
    pub pending_flares: Vec<FlareEvent>,
    /// A spawn-class flare awaiting its sequence bracket: `(name, weapon)`.
    pub spawn_pending: Option<(String, Option<Actor>)>,
    pub active_spawn: Option<ActiveSpawn>,
    /// Echo flares in announce order, consumed FIFO by bare own swings.
    pub pending_echoes: VecDeque<Echo>,
    /// The current spawn tree's root.
    pub spawn_root: Option<EvId>,
    pub active_sequence: Option<SequenceName>,
    /// Rolls that could not claim a virgin sink.
    pub pending_resolutions: Vec<Resolution>,
    pub pending_ambush: Option<AmbushPrefix>,
    pub pending_redirect: Option<Redirect>,
    /// Dispel-family flares this chunk, by target id; `None` is "any".
    pub chunk_dispels: Vec<Option<i64>>,
    /// Creatures this chunk killed.
    pub chunk_deaths: Vec<i64>,
    /// Ownership of `DoT` casts, per `(spell, victim)`: `Some(true)` is ours.
    ///
    /// Lich writes `nil` for a tick line, which overwrites an earlier
    /// `:self` -- so only the FIRST tick after our cast is ours. Ported as
    /// is; a fix is a behaviour change for the author to rule on.
    pub cast_owner: BTreeMap<(String, String), Option<bool>>,
    /// A nearby player's `AoE` opener, while its actor-less lines fan out.
    pub foreign_latch: Option<String>,
    /// Facts with no recognised initiation anywhere in the chunk.
    pub orphan_hits: Vec<Hit>,
    pub orphan_outcomes: Vec<OutcomeKind>,
}

impl<'a> Cursor<'a> {
    pub fn new(tracker: &'a mut CombatTracker, lines: &'a [ChunkLine], at: Option<u32>) -> Self {
        Self {
            tracker,
            lines,
            at,
            pool: Vec::new(),
            events: Vec::new(),
            released_children: BTreeMap::new(),
            facts: Vec::new(),
            current: None,
            seeking_damage: false,
            current_target: None,
            interrupted_own: None,
            released_parent: None,
            pending_release_cast: None,
            deferred_casts: Vec::new(),
            last_inbound_attacker: None,
            single_hit_parent: None,
            nocked: None,
            flare_ctx: None,
            pending_flares: Vec::new(),
            spawn_pending: None,
            active_spawn: None,
            pending_echoes: VecDeque::new(),
            spawn_root: None,
            active_sequence: None,
            pending_resolutions: Vec::new(),
            pending_ambush: None,
            pending_redirect: None,
            chunk_dispels: Vec::new(),
            chunk_deaths: Vec::new(),
            cast_owner: BTreeMap::new(),
            foreign_latch: None,
            orphan_hits: Vec::new(),
            orphan_outcomes: Vec::new(),
        }
    }

    // --- the pool -------------------------------------------------------

    pub fn ev(&self, id: EvId) -> &AttackEvent {
        &self.pool[id]
    }

    pub fn ev_mut(&mut self, id: EvId) -> &mut AttackEvent {
        &mut self.pool[id]
    }

    pub fn cur(&self) -> Option<&AttackEvent> {
        self.current.map(|id| &self.pool[id])
    }

    /// A fresh event with this chunk's prompt time.
    pub fn blank(&self, name: &str, target: EventTarget) -> AttackEvent {
        AttackEvent {
            name: name.to_owned(),
            target,
            at: self.at,
            ..AttackEvent::default()
        }
    }

    pub fn open(&mut self, ev: AttackEvent) -> EvId {
        self.pool.push(ev);
        self.pool.len() - 1
    }

    /// Identity-guarded push, plus the released children that must follow
    /// their parent (`save_event`, `processor.rb:369-381`).
    pub fn save_event(&mut self, id: EvId) {
        if self.events.contains(&id) {
            return;
        }
        self.events.push(id);
        if let Some(children) = self.released_children.remove(&id) {
            for c in children {
                if !self.events.contains(&c) {
                    self.events.push(c);
                }
            }
        }
    }

    /// Make `id` the open event and point the target cursor at its creature.
    pub fn resume(&mut self, id: EvId) {
        self.current = Some(id);
        if let Some(t) = self.pool[id].target.creature().filter(|a| a.id.is_some()) {
            self.current_target = Some(t.clone());
        }
    }

    // --- flares ---------------------------------------------------------

    pub fn flare(&self, r: FlareRef) -> &FlareEvent {
        match r {
            FlareRef::OnEvent(id, i) => &self.pool[id].flares[i],
            FlareRef::Pending(i) => &self.pending_flares[i],
        }
    }

    pub fn flare_mut(&mut self, r: FlareRef) -> &mut FlareEvent {
        match r {
            FlareRef::OnEvent(id, i) => &mut self.pool[id].flares[i],
            FlareRef::Pending(i) => &mut self.pending_flares[i],
        }
    }

    /// The flare cursor's target creature, if it has one.
    pub fn flare_ctx_target(&self) -> Option<&Actor> {
        self.flare_ctx.and_then(|r| self.flare(r).target.as_ref())
    }

    // --- predicates -----------------------------------------------------

    /// A fact-less bare gesture (`bare_cast?`, `processor.rb:266-272`).
    pub fn bare_cast(&self, id: EvId) -> bool {
        let e = &self.pool[id];
        e.attack_born
            && e.name == "cast"
            && e.hits.is_empty()
            && e.outcomes.is_empty()
            && e.flares
                .iter()
                .all(|f| f.hits.is_empty() && f.outcomes.is_empty())
            && !e.had_status
    }

    /// A target to apply to and data to apply (`event_worth_saving?`).
    fn worth_saving(e: &AttackEvent) -> bool {
        e.target.id().is_some()
            && (!e.hits.is_empty() || e.flares.iter().any(|f| !f.hits.is_empty()))
    }

    /// Does the event survive to emit? (`event_savable?`, with the recorder
    /// always subscribed: `include_attack_events` is `true` here.)
    pub fn savable(&self, id: EvId) -> bool {
        let e = &self.pool[id];
        let has_facts = !e.outcomes.is_empty() || !e.resolutions.is_empty() || !e.hits.is_empty();
        if e.target.id().is_none() {
            if !(e.inbound || e.is_foreign_target() || e.attack_born) {
                return false;
            }
            return e.attack_born || has_facts;
        }
        Self::worth_saving(e) || e.attack_born || has_facts || !e.flares.is_empty() || e.had_status
    }

    /// Never part of our spawn tree.
    pub fn not_ours(e: &AttackEvent) -> bool {
        e.inbound || e.is_foreign_target() || e.foreign_caster || e.unowned || e.orphan
    }

    /// Does the flare's linked weapon appear in the swing's weapon text?
    /// Flares without weapon info match any swing (`flare_matches_weapon?`).
    pub fn flare_matches_weapon(flare: &FlareEvent, weapon_text: Option<&str>) -> bool {
        match (&flare.weapon, weapon_text) {
            (Some(w), Some(t)) => t.contains(w.name.as_str()),
            _ => true,
        }
    }

    /// Does the open event already hold a flare from a DIFFERENT weapon?
    /// The back-to-back dual-wield signature (`flare_contradicts_weapon?`).
    pub fn flare_contradicts_weapon(flare: &FlareEvent, e: &AttackEvent) -> bool {
        let Some(fw) = flare.weapon.as_ref().map(|w| w.name.as_str()) else {
            return false;
        };
        e.flares
            .iter()
            .filter_map(|p| p.weapon.as_ref())
            .any(|pw| pw.name != fw)
    }

    /// Is this flare name one that releases an imbedded spell?
    pub fn releasing(name: &str) -> bool {
        SPELL_RELEASING_FLARES.contains(&name)
    }
}

/// `\AYou\b`: a second-person line.
pub(super) fn starts_you(text: &str) -> bool {
    text.strip_prefix("You").is_some_and(|rest| {
        rest.chars()
            .next()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
    })
}

/// `\A\s*Your?\b`: a line about us.
pub(super) fn starts_your_or_you(text: &str) -> bool {
    let t = text.trim_start();
    starts_you(t)
        || t.strip_prefix("Your").is_some_and(|rest| {
            rest.chars()
                .next()
                .is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
        })
}
