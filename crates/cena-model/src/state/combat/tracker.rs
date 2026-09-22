//! The combat consumer: what survives between chunks, and the entry point.
//!
//! Ports the cross-chunk half of `processor.rb` -- its module ivars
//! (`inventory/11` §3a) -- and `Tracker`'s one real job, feeding a
//! prompt-bounded chunk to the state machine. Everything Lich's tracker does
//! besides that (a `DownstreamHook`, a buffer, a worker thread, settings in
//! `DB_Store`) is plumbing the chunk mechanism and the session already own.
//!
//! # Three facts cross a chunk boundary
//!
//! | Field | Lich | Why it must survive the prompt |
//! |---|---|---|
//! | `held_cast` | `@held_cast` | *"You gesture at X."* ends a chunk; the spell's result opens the next. Held for ONE chunk, then superseded or emitted as itself. |
//! | `held_pre_flares` | `@held_pre_flares` | A bow's dispel fires on the NOCK and resolves before *"You fire"* prints -- in the next chunk. |
//! | [`active_assault`](CombatTracker::active_assault) | `@active_assault` | A flurry's rounds arrive over several chunks; the opener named the only target they can strike. |
//!
//! Lich's `@death_watch` / `@death_announced` (a creature an event touched,
//! watched until a room refresh flags it dead) belong to `persist_event` and
//! arrive with the creature registry. `@position_recovered` is reset at the
//! top of every chunk and is therefore chunk-local here, on the cursor.
//!
//! # No settings
//!
//! Lich gates damage, wounds, statuses and UCS behind `track_*` settings and
//! the event payload behind `emit_attacks`. Every one of those has exactly one
//! value here, on (`plan/05` Rule -1: no config option with one value), and
//! `inventory/11` §3c records why: *"the emit carried a crit-shaped hole"*
//! when a gate was wrong.
//!
//! # The crit tables are a handle, not a global
//!
//! `crit.rs` holds `CritTables` behind an `Arc` and forbids a static, for
//! the reason multi-session depends on. So the tracker is *handed* its tables
//! by whoever built the `GameState` -- [`CombatTracker::set_crit_tables`] --
//! and without them the crit lookahead is skipped and every [`Hit`](super::event::Hit) carries
//! `crit: None`. A test that asserts crits sets them; a session sets them
//! once.

use std::collections::VecDeque;
use std::sync::Arc;

use super::bracket::AssaultName;
use super::event::{AttackEvent, ChunkFacts, FlareEvent};
use super::target::Actor;
use crate::crit::CritTables;
use crate::state::chunks::Chunk;

/// The open assault bracket, across chunks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveAssault {
    /// Which assault.
    pub name: AssaultName,
    /// The only target its rounds can strike. `None` until the first named
    /// round backfills it (barrage's opener names nobody).
    pub target: Option<Actor>,
}

/// How many chunks' facts are kept for a consumer to drain.
///
/// A recorder reads them as they arrive; a bounded queue means a session
/// with no recorder attached does not grow without limit. Oldest dropped
/// first, and counted, per Rule 2.2.
pub const MAX_PENDING_CHUNKS: usize = 64;

/// The combat consumer's state between chunks.
#[derive(Clone, Default)]
pub struct CombatTracker {
    /// A bare `cast` gesture whose spell result has not arrived yet.
    pub(super) held_cast: Option<AttackEvent>,
    /// Pre-flares no swing claimed, held for the next chunk's first swing.
    pub(super) held_pre_flares: Vec<FlareEvent>,
    /// The open single-target multi-round attack.
    pub(super) active_assault: Option<ActiveAssault>,
    /// The crit tables, when the session has provided them.
    crit: Option<Arc<CritTables>>,
    /// Facts not yet drained by a consumer, oldest first.
    pending: VecDeque<ChunkFacts>,
    /// Chunks' facts dropped to the cap before anyone drained them.
    dropped: usize,
    /// How many chunks have been consumed, ever.
    chunks_seen: u64,
}

impl std::fmt::Debug for CombatTracker {
    /// `CritTables` is 2,394 compiled patterns and does not print itself;
    /// whether the handle is set is the fact worth showing.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CombatTracker")
            .field("held_cast", &self.held_cast)
            .field("held_pre_flares", &self.held_pre_flares)
            .field("active_assault", &self.active_assault)
            .field("crit_tables", &self.crit.is_some())
            .field("pending", &self.pending.len())
            .field("dropped", &self.dropped)
            .field("chunks_seen", &self.chunks_seen)
            .finish()
    }
}

impl PartialEq for CombatTracker {
    /// Everything but the tables themselves: whether they are set is
    /// compared, their 2,394 rows are not. `GameState` derives `Eq` over
    /// this, and no `f64` here comes from anywhere but parsed digits.
    fn eq(&self, other: &Self) -> bool {
        self.held_cast == other.held_cast
            && self.held_pre_flares == other.held_pre_flares
            && self.active_assault == other.active_assault
            && self.crit.is_some() == other.crit.is_some()
            && self.pending == other.pending
            && self.dropped == other.dropped
            && self.chunks_seen == other.chunks_seen
    }
}

impl Eq for CombatTracker {}

impl CombatTracker {
    /// Give the tracker its crit tables.
    ///
    /// The session builds one `CritTables` and shares it as an `Arc`. Until
    /// this is called, hits carry no crit.
    pub fn set_crit_tables(&mut self, tables: Arc<CritTables>) {
        self.crit = Some(tables);
    }

    /// The crit tables, if provided.
    #[must_use]
    pub fn crit_tables(&self) -> Option<&CritTables> {
        self.crit.as_deref()
    }

    /// The open assault, if any.
    #[must_use]
    pub const fn active_assault(&self) -> Option<&ActiveAssault> {
        self.active_assault.as_ref()
    }

    /// Is a bare cast being held for the next chunk?
    #[must_use]
    pub const fn holds_a_cast(&self) -> bool {
        self.held_cast.is_some()
    }

    /// How many pre-flares are held for the next chunk.
    #[must_use]
    pub fn held_pre_flare_count(&self) -> usize {
        self.held_pre_flares.len()
    }

    /// Chunks consumed so far.
    #[must_use]
    pub const fn chunks_seen(&self) -> u64 {
        self.chunks_seen
    }

    /// Facts dropped to the pending cap before being drained.
    #[must_use]
    pub const fn dropped(&self) -> usize {
        self.dropped
    }

    /// Take every pending chunk's facts, oldest first.
    pub fn take_facts(&mut self) -> Vec<ChunkFacts> {
        self.pending.drain(..).collect()
    }

    /// The most recent chunk's facts, without taking them.
    #[must_use]
    pub fn last_facts(&self) -> Option<&ChunkFacts> {
        self.pending.back()
    }

    /// Parse one completed chunk into its facts, without publishing them.
    ///
    /// `at` is the chunk's prompt time -- the server epoch second the prompt
    /// carried -- which stamps every event. `GameState::close_chunk` applies
    /// the facts to the creature registry between this and [`Self::publish`],
    /// which is Lich's `process`: parse, persist, then emit.
    pub fn parse_chunk(&mut self, chunk: &Chunk, at: Option<u32>) -> ChunkFacts {
        self.chunks_seen = self.chunks_seen.saturating_add(1);
        super::parse::parse_chunk(self, chunk, at)
    }

    /// Queue a chunk's facts for a consumer to drain. Empty facts are not
    /// queued.
    pub fn publish(&mut self, facts: ChunkFacts) {
        if facts.is_empty() {
            return;
        }
        if self.pending.len() >= MAX_PENDING_CHUNKS {
            self.pending.pop_front();
            self.dropped = self.dropped.saturating_add(1);
        }
        self.pending.push_back(facts);
    }

    /// Parse and publish in one step: the registry-less path.
    pub fn consume_chunk(&mut self, chunk: &Chunk, at: Option<u32>) {
        let facts = self.parse_chunk(chunk, at);
        self.publish(facts);
    }

    /// Forget everything a new connection has not re-taught.
    ///
    /// A held cast or pre-flare belongs to a chunk the old connection never
    /// finished; an assault bracket cannot outlive the fight it was in.
    pub(crate) fn invalidate_for_reconnect(&mut self) {
        self.held_cast = None;
        self.held_pre_flares.clear();
        self.active_assault = None;
    }
}
