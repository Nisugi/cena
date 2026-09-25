//! Classified chunks waiting for the session's ledger.
//!
//! The same handoff the combat tracker makes (`combat/tracker.rs`,
//! `pending`): `GameState::close_chunk` classifies each chunk as its prompt
//! closes it and queues the result here, and the actor drains the queue once
//! per prompt and offers each chunk to the ledger's worker. The queue is
//! bounded, and what the cap drops is counted (Rule 2.2).
//!
//! # What travels with the facts
//!
//! Two things the classifier cannot read off the chunk, taken at the moment
//! the chunk closes because both can change by the next prompt:
//!
//! - **the room**, `state.room.id`, the wire's `<nav rm=>` -- the fix for
//!   `plan/34` §3's Red Forest bug, where a map room's list of uids was used
//!   as a key;
//! - **a box's contents**, for each [`LootFact::BoxOpened`]: the `<inv>`
//!   lines the inventory model holds under the box's id. loottracker read
//!   them from the raw `<inv id=>` tags in the chunk; here they are frames,
//!   not lines, and the inventory already typed them.

use std::collections::{BTreeMap, VecDeque};

use super::LootFact;
use crate::state::containers::ItemRef;

/// One chunk's loot facts with what the ledger needs beside them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LootChunk {
    /// What the chunk stated, in line order.
    pub facts: Vec<LootFact>,
    /// For each box opened in this chunk, its contents as the inventory
    /// listed them, keyed by the box's `exist` id. Absent when the game
    /// listed nothing.
    pub contents: BTreeMap<String, Vec<ItemRef>>,
    /// The prompt that closed the chunk, in server seconds; `None` until the
    /// first prompt of a connection has said what time it is.
    pub at: Option<u32>,
    /// The room the chunk closed in: `<nav rm=>`.
    pub room: Option<String>,
}

/// How many chunks wait before the oldest is dropped. Loot chunks are rarer
/// than combat chunks and the actor drains every prompt, so this is depth
/// against a stalled drain, not a working buffer.
pub const MAX_PENDING_LOOT: usize = 256;

/// The queue, drained by the actor.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LootQueue {
    chunks: VecDeque<LootChunk>,
    dropped: u64,
}

impl LootQueue {
    /// Queue one chunk; at the cap the oldest goes and is counted.
    pub fn push(&mut self, chunk: LootChunk) {
        if self.chunks.len() >= MAX_PENDING_LOOT {
            self.chunks.pop_front();
            self.dropped = self.dropped.saturating_add(1);
        }
        self.chunks.push_back(chunk);
    }

    /// Every waiting chunk, oldest first, leaving the queue empty.
    pub fn take(&mut self) -> Vec<LootChunk> {
        self.chunks.drain(..).collect()
    }

    /// Chunks lost to the cap before being drained.
    #[must_use]
    pub const fn dropped(&self) -> u64 {
        self.dropped
    }

    /// How many wait.
    #[must_use]
    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    /// Nothing waits.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }
}
