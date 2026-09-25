//! The recorder's own thread, and the handle a session feeds it through.
//!
//! # Why a thread, and why its own channel
//!
//! `SQLite` writes block, and the session actor is an async task whose every
//! wait has a deadline (`plan/12` §5.5). A write on the actor would be a
//! stall nothing bounds. Lich has the same shape for the same reason: its
//! recorder runs on the `AsyncProcessor` worker, never on the game thread.
//!
//! The broadcast ring is the wrong pipe for it. That ring is for OBSERVERS,
//! for whom `Lagged` is an honest answer (`actor.rs`, `EVENT_CHANNEL_BOUND`);
//! a recorder that misses a chunk is wrong forever. So it gets a bounded
//! queue of its own, fed directly by the actor, and the one way it can lose
//! a chunk -- the queue full because the disk stopped -- is counted and
//! surfaced rather than silent (Rule 2.2).
//!
//! # The actor never waits on it
//!
//! [`RecorderHandle::offer`] is `try_send`. A full queue drops the chunk and
//! says so; it does not block the session. Combat keeps working when the
//! database does not.
//!
//! # Idle close without a second clock
//!
//! A hunt must close even if no further combat ever arrives, and the
//! recorder's time is the server's. The actor sends the prompt's own time on
//! every quiet prompt ([`RecorderHandle::tick`]), so the idle gap is judged
//! on the one clock the rows are stamped with.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use cena_model::state::combat::ChunkFacts;

use super::CombatRecorder;

/// Chunks the queue holds before it drops. A chunk is a prompt's worth of
/// combat; a thousand of them is minutes of the heaviest hunting, against a
/// write that takes milliseconds.
pub const QUEUE_BOUND: usize = 1024;

enum Msg {
    Chunk(Arc<ChunkFacts>),
    Tick(f64),
}

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

/// The session's end of the recorder. Cloneable: a supervisor holds one and
/// hands a clone to each connection's actor, so one hunt spans a reconnect.
#[derive(Debug, Clone)]
pub struct RecorderHandle {
    tx: SyncSender<Msg>,
    stats: Arc<Stats>,
}

impl RecorderHandle {
    /// Queue one chunk's facts. Returns whether it was queued; a refusal is
    /// already counted in [`Stats`]. Never blocks.
    #[must_use]
    pub fn offer(&self, facts: Arc<ChunkFacts>) -> bool {
        if facts.at.is_none() {
            self.stats.untimed.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        self.send(Msg::Chunk(facts))
    }

    /// A quiet prompt's server time, so an idle hunt can close.
    pub fn tick(&self, at: u32) {
        // A lost tick is not a lost fact: the next one carries a later time.
        let _ = self.tx.try_send(Msg::Tick(f64::from(at)));
    }

    /// What the worker has done so far.
    #[must_use]
    pub fn stats(&self) -> &Stats {
        &self.stats
    }

    fn send(&self, msg: Msg) -> bool {
        match self.tx.try_send(msg) {
            Ok(()) => true,
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
                self.stats.dropped.fetch_add(1, Ordering::Relaxed);
                false
            }
        }
    }
}

/// Move `recorder` onto its own thread.
///
/// The thread ends when every [`RecorderHandle`] is dropped: it drains what
/// is queued, closes any open session at its last event, and releases the
/// database. Join the returned handle to wait for that flush.
///
/// # Errors
///
/// The OS refused the thread.
pub fn spawn(recorder: CombatRecorder) -> std::io::Result<(RecorderHandle, JoinHandle<()>)> {
    let (tx, rx) = sync_channel(QUEUE_BOUND);
    let stats = Arc::new(Stats::default());
    let worker_stats = Arc::clone(&stats);
    let join = std::thread::Builder::new()
        .name("combat-recorder".to_owned())
        .spawn(move || run(recorder, &rx, &worker_stats))?;
    Ok((RecorderHandle { tx, stats }, join))
}

/// Open a character's live database under `dir` and start its worker.
///
/// ```text
/// <dir>/<game>_<character>_combat.db
/// ```
///
/// Game and character both, for the reason the character store gives: two
/// characters of one name on different instances are different characters,
/// and merging their hunts would be silent. Sessions are auto-bounded by
/// [`DEFAULT_IDLE_TIMEOUT`](super::DEFAULT_IDLE_TIMEOUT), the owner's ruling.
///
/// # Errors
///
/// A name that sanitises to nothing, a directory that cannot be created, a
/// database that cannot be opened, or a thread the OS refused -- each as an
/// `io::Error` naming which. A caller should say so and run without a
/// recorder: combat does not depend on one.
pub fn open_live(
    dir: &Path,
    game: &str,
    character: &str,
) -> std::io::Result<(RecorderHandle, JoinHandle<()>, PathBuf)> {
    let path = database_path(dir, game, character)?;
    std::fs::create_dir_all(dir)?;
    let recorder = CombatRecorder::open(
        &path,
        Some(character),
        "live",
        Some(super::DEFAULT_IDLE_TIMEOUT),
    )
    .map_err(std::io::Error::other)?;
    let (handle, join) = spawn(recorder)?;
    Ok((handle, join, path))
}

/// Where a character's database is: `<dir>/<game>_<character>_combat.db`,
/// the names sanitised as the character store sanitises them. The loot
/// ledger's tables live in the same file, and `;loot` reads it by this path.
///
/// # Errors
///
/// A game or character name that sanitises to nothing.
pub fn database_path(dir: &Path, game: &str, character: &str) -> std::io::Result<PathBuf> {
    use crate::character_store::safe_component;
    let (game, name) = (safe_component(game), safe_component(character));
    if game.is_empty() || name.is_empty() {
        return Err(std::io::Error::other(
            "no usable game or character name for a combat database",
        ));
    }
    Ok(dir.join(format!("{game}_{name}_combat.db")))
}

fn run(mut recorder: CombatRecorder, rx: &Receiver<Msg>, stats: &Stats) {
    let mut last_at = 0.0;
    while let Ok(msg) = rx.recv() {
        let result = match msg {
            Msg::Chunk(facts) => {
                let Some(at) = facts.at.map(f64::from) else {
                    continue;
                };
                last_at = at;
                recorder.record_chunk(&facts, at).map(|()| {
                    stats.recorded.fetch_add(1, Ordering::Relaxed);
                })
            }
            Msg::Tick(at) => {
                last_at = at;
                recorder.check_idle(at).map(|_| ())
            }
        };
        if let Err(e) = result {
            fail(stats, &e);
        }
    }
    if let Err(e) = recorder.close(last_at) {
        fail(stats, &e);
    }
}

fn fail(stats: &Stats, error: &super::Error) {
    stats.failed.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut slot) = stats.last_error.lock() {
        *slot = Some(error.to_string());
    }
}
