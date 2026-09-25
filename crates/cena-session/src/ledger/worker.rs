//! The ledger's own thread, and the handle a session feeds it through.
//!
//! The combat recorder's worker, for the same reasons (`combat_recorder/
//! worker.rs`): `SQLite` writes block and the actor must not; a recorder that
//! misses a chunk is wrong forever, so it gets a bounded queue of its own
//! rather than the observers' ring; the actor's offer is `try_send` and a full
//! queue is counted, never waited on. Two workers rather than one generic
//! one: they differ in what they carry and in the tick the combat one needs
//! for its idle close, and Rule -1's rule of three has not been met.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use cena_model::state::ledger::LootChunk;

use super::Ledger;

/// Chunks the queue holds before it drops: a prompt's worth of loot each.
pub const QUEUE_BOUND: usize = 1024;

/// What the worker has done, readable from any thread.
#[derive(Debug, Default)]
pub struct Stats {
    recorded: AtomicU64,
    failed: AtomicU64,
    dropped: AtomicU64,
    untimed: AtomicU64,
    last_error: Mutex<Option<String>>,
}

impl Stats {
    /// Chunks written.
    #[must_use]
    pub fn recorded(&self) -> u64 {
        self.recorded.load(Ordering::Relaxed)
    }

    /// Chunks whose write failed and was rolled back.
    #[must_use]
    pub fn failed(&self) -> u64 {
        self.failed.load(Ordering::Relaxed)
    }

    /// Chunks dropped because the queue was full or the worker was gone.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Chunks dropped because no server time was known to stamp them with.
    #[must_use]
    pub fn untimed(&self) -> u64 {
        self.untimed.load(Ordering::Relaxed)
    }

    /// The most recent write failure, if any.
    #[must_use]
    pub fn last_error(&self) -> Option<String> {
        self.last_error.lock().ok().and_then(|e| e.clone())
    }
}

/// The session's end of the ledger. Cloneable: a supervisor holds one and
/// hands a clone to each connection's actor.
#[derive(Debug, Clone)]
pub struct LedgerHandle {
    tx: SyncSender<Arc<LootChunk>>,
    stats: Arc<Stats>,
}

impl LedgerHandle {
    /// Queue one chunk. Returns whether it was queued; a refusal is already
    /// counted in [`Stats`]. Never blocks.
    #[must_use]
    pub fn offer(&self, chunk: Arc<LootChunk>) -> bool {
        if chunk.at.is_none() {
            self.stats.untimed.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        match self.tx.try_send(chunk) {
            Ok(()) => true,
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
                self.stats.dropped.fetch_add(1, Ordering::Relaxed);
                false
            }
        }
    }

    /// What the worker has done so far.
    #[must_use]
    pub fn stats(&self) -> &Stats {
        &self.stats
    }
}

/// Move `ledger` onto its own thread. The thread ends when every handle is
/// dropped, after draining what is queued; join the returned handle to wait
/// for that flush.
///
/// # Errors
///
/// The OS refused the thread.
pub fn spawn(ledger: Ledger) -> std::io::Result<(LedgerHandle, JoinHandle<()>)> {
    let (tx, rx) = sync_channel(QUEUE_BOUND);
    let stats = Arc::new(Stats::default());
    let worker_stats = Arc::clone(&stats);
    let join = std::thread::Builder::new()
        .name("loot-ledger".to_owned())
        .spawn(move || run(ledger, &rx, &worker_stats))?;
    Ok((LedgerHandle { tx, stats }, join))
}

/// Open the ledger's tables in a character's database at `path` -- the
/// combat recorder's file, so one character's hunts and loot are one file --
/// and start its worker.
///
/// # Errors
///
/// A database that cannot be opened, or a thread the OS refused, as an
/// `io::Error`. A caller should say so and run without a ledger: nothing
/// depends on one.
pub fn open_live(path: &Path, character: &str) -> std::io::Result<(LedgerHandle, JoinHandle<()>)> {
    let ledger = Ledger::open(path, Some(character)).map_err(std::io::Error::other)?;
    spawn(ledger)
}

fn run(mut ledger: Ledger, rx: &Receiver<Arc<LootChunk>>, stats: &Stats) {
    while let Ok(chunk) = rx.recv() {
        let Some(at) = chunk.at.map(f64::from) else {
            continue;
        };
        match ledger.record(&chunk, at) {
            Ok(()) => {
                stats.recorded.fetch_add(1, Ordering::Relaxed);
            }
            Err(e) => fail(stats, &e),
        }
    }
    if let Err(e) = ledger.close() {
        fail(stats, &e);
    }
}

fn fail(stats: &Stats, error: &super::Error) {
    stats.failed.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut slot) = stats.last_error.lock() {
        *slot = Some(error.to_string());
    }
}
