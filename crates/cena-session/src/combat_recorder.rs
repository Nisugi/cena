//! The combat recorder: a chunk's facts, persisted to `SQLite`.
//!
//! A port of `lib/gemstone/combat/recorder.rb`. The schema is Lich's, column
//! for column ([`schema`]), so the author's `combat_stats.db` and a database
//! written here are the same shape and the `cstats` reports are SQL over
//! both. Author decisions 2026-09-20 (`inventory/11` §8): `rusqlite`, and the
//! recorder lives here -- file I/O -- while the state machine that produces
//! its input lives in `cena-model`.
//!
//! # What one call records
//!
//! [`CombatRecorder::record_chunk`] takes what `GameState::close_chunk`
//! produced -- a `ChunkFacts` -- and writes it in Lich's emit order
//! (`processor.rb:117-172`): each attack with its resolutions, flares and
//! hits; then the facts, which `cena-model` already orders crit-derived,
//! deaths, parsed. Lich's recorder subscribes to six observer topics and
//! reassembles a chunk from them with per-chunk uids and a batch id; here the
//! chunk arrives whole, so an event's index IS its uid and none of that
//! bookkeeping exists.
//!
//! One chunk is one transaction. If any write fails the chunk is rolled back
//! and the in-memory state (its private `Live`) is restored with it, so a cached row id
//! can never outlive the row (Lich's `in_txn`, `recorder.rb:633-651`).
//!
//! # Time is the server's
//!
//! Every `at` is the caller's, in seconds -- the chunk's `<prompt time=>`.
//! Lich stamps statuses and judges the idle gap with `Time.now`, which is why
//! its recorder cannot replay a log deterministically without a driver
//! passing times in. One clock, passed in, makes live and replay the same
//! code path.
//!
//! # Sessions
//!
//! Owner ruling 2026-09-05 (`recorder.rb:83-88`): a hunt is bounded by a
//! five-minute gap without combat. With an idle timeout the recorder opens a
//! session on the first event that is OURS and closes it *at the last event's
//! time*, so town time never pads a hunt. A nearby player's cast neither
//! opens a session nor keeps one alive; a death confirmed after the gap
//! re-opens the hunt that fought the creature rather than starting another.
//!
//! # NOT ported, and why
//!
//! - **The mutex and the `_locked` twins.** Lich's recorder is called from a
//!   worker thread and polled from a script's loop. This one is `&mut self`:
//!   the borrow checker is the lock.
//! - **Receipts** (`combat.recorded_attack`), **ingestion provenance**, and
//!   the **session-finished callbacks**. Each serves a consumer Cena does not
//!   have. [`CombatRecorder::drain_finished_sessions`] is kept: it is the
//!   pollable half of the same hook and costs a `Vec`.
//! - **`migrate!`.** See [`schema`].
//!
//! # How a session feeds it
//!
//! Not by calling it. [`worker`] moves the recorder onto its own thread and
//! gives the session a [`worker::RecorderHandle`]; the actor offers each
//! closed chunk's facts to it and never waits (`actor/combat.rs`).
//!
//! # One loss inherited, marked
//!
//! A UCS tier-up names the followup attack (`jab`, `kick`), and the
//! `statuses.value` column is an integer, so Lich records the row with a NULL
//! value (`ucs_value`, `recorder.rb:929-933`). The attack's name is written
//! to `spell_name` here instead of being dropped (Rule 2.2); Lich leaves that
//! column NULL for `ucs` rows, so no report reads it wrongly.

mod attack;
pub mod schema;
mod status;
pub mod worker;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use cena_model::state::combat::ChunkFacts;
use rusqlite::{Connection, Transaction, params};

/// The owner's idle gap, in seconds (`combat_stats.lic`'s `IDLE_TIMEOUT`).
pub const DEFAULT_IDLE_TIMEOUT: f64 = 300.0;

/// A recording failure. The chunk it interrupted was rolled back whole.
pub type Error = rusqlite::Error;

/// The attack whose window a following fact may fall in.
#[derive(Debug, Clone, PartialEq)]
struct OpenAttack {
    id: i64,
    /// `exist` ids of every creature the attack or its flares touched.
    touched: BTreeSet<i64>,
    inbound: bool,
    flare_ids: Vec<i64>,
}

/// The session the idle gap just closed, kept so a trailing fact finds it.
#[derive(Debug, Clone, PartialEq)]
struct ClosedSession {
    id: i64,
    cache: BTreeMap<i64, i64>,
}

/// Everything the recorder remembers between chunks. `Clone` so a failed
/// chunk can be undone in memory as the transaction undoes it on disk.
#[derive(Debug, Clone, Default, PartialEq)]
struct Live {
    session_id: Option<i64>,
    /// Monotonic within a session: `attacks.seq`.
    seq: i64,
    /// Chunks with a recorded attack, for the recorder's lifetime:
    /// `attacks.chunk_seq` orders chunks that share a whole second.
    chunk_seq: i64,
    /// `exist` id -> `creatures.id`, for the open session.
    creature_cache: BTreeMap<i64, i64>,
    closed: Option<ClosedSession>,
    open_attack: Option<OpenAttack>,
    last_event_at: Option<f64>,
    /// Status rows that already absorbed their stun twin.
    merged_stun_rows: BTreeSet<i64>,
    finished: Vec<i64>,
}

/// Persists combat facts to one `SQLite` database.
#[derive(Debug)]
pub struct CombatRecorder {
    conn: Connection,
    character: Option<String>,
    source: String,
    idle_timeout: Option<f64>,
    live: Live,
}

/// One chunk's write, borrowing the recorder's halves disjointly.
struct Writer<'a> {
    tx: &'a Transaction<'a>,
    live: &'a mut Live,
    character: Option<&'a str>,
    source: &'a str,
    idle_timeout: Option<f64>,
    /// Event index -> `attacks.id`, for this chunk. `None`: not recorded.
    chunk_rows: Vec<Option<i64>>,
    /// Event index -> its flares' row ids.
    chunk_flares: Vec<Vec<i64>>,
}

/// Lich's `txt` (`recorder.rb:320-327`): trimmed, inner runs of spaces
/// squeezed, empty to NULL. *A creature revealed from hiding sometimes
/// arrives with a space injected into both attributes of its link --
/// `noun=" rogue" name="human  rogue"` -- and untouched, " rogue" is a noun
/// distinct from "rogue" and the creature reports as its own kind forever.*
fn txt(value: &str) -> Option<String> {
    let mut out = String::with_capacity(value.len());
    for word in value.split(' ').filter(|w| !w.is_empty()) {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    let out = out.trim().to_owned();
    (!out.is_empty()).then_some(out)
}

impl CombatRecorder {
    /// Open (creating if absent) the database at `path`.
    ///
    /// `idle_timeout` in seconds enables auto-sessioning; `None` means the
    /// caller brackets sessions itself with [`Self::start_session`] and
    /// [`Self::finish_session`], as a replay driver does.
    ///
    /// # Errors
    ///
    /// Any `SQLite` failure opening the file or creating the schema.
    pub fn open(
        path: &Path,
        character: Option<&str>,
        source: &str,
        idle_timeout: Option<f64>,
    ) -> Result<Self, Error> {
        Self::with_connection(Connection::open(path)?, character, source, idle_timeout)
    }

    /// A private in-memory database: tests, and a dry run.
    ///
    /// # Errors
    ///
    /// Any `SQLite` failure creating the schema.
    pub fn in_memory(
        character: Option<&str>,
        source: &str,
        idle_timeout: Option<f64>,
    ) -> Result<Self, Error> {
        Self::with_connection(
            Connection::open_in_memory()?,
            character,
            source,
            idle_timeout,
        )
    }

    fn with_connection(
        conn: Connection,
        character: Option<&str>,
        source: &str,
        idle_timeout: Option<f64>,
    ) -> Result<Self, Error> {
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        // `journal_mode` answers with a row; an in-memory database answers
        // "memory", which is not an error.
        conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
        conn.execute_batch("PRAGMA synchronous = NORMAL;")?;
        conn.execute_batch(schema::SCHEMA)?;
        conn.pragma_update(None, "user_version", schema::USER_VERSION)?;
        Ok(Self {
            conn,
            character: character.map(str::to_owned),
            source: source.to_owned(),
            idle_timeout,
            live: Live::default(),
        })
    }

    /// The database, for reports and tests. Read it; do not write the seven
    /// tables behind the recorder's back.
    #[must_use]
    pub const fn connection(&self) -> &Connection {
        &self.conn
    }

    /// The open session's id.
    #[must_use]
    pub const fn session_id(&self) -> Option<i64> {
        self.live.session_id
    }

    /// Every session finished since the last drain, oldest first. A session
    /// the idle gap closed and a trailing death re-opened closes again later
    /// and appears again.
    pub fn drain_finished_sessions(&mut self) -> Vec<i64> {
        std::mem::take(&mut self.live.finished)
    }

    /// Open a session explicitly, closing any open one at `at`.
    ///
    /// # Errors
    ///
    /// Any `SQLite` failure; nothing is changed.
    pub fn start_session(&mut self, at: f64) -> Result<i64, Error> {
        self.in_txn(|w| w.start_session(at))
    }

    /// Close the open session at `at`. `None` if none was open.
    ///
    /// # Errors
    ///
    /// Any `SQLite` failure; nothing is changed.
    pub fn finish_session(&mut self, at: f64) -> Result<Option<i64>, Error> {
        self.in_txn(|w| w.finish_session(at))
    }

    /// Close the session if the idle gap has elapsed -- at the LAST EVENT's
    /// time, not `now`. Cheap; call it from a timer so a finished hunt closes
    /// even when no further event ever arrives.
    ///
    /// # Errors
    ///
    /// Any `SQLite` failure; nothing is changed.
    pub fn check_idle(&mut self, now: f64) -> Result<Option<i64>, Error> {
        self.in_txn(|w| w.check_idle(now))
    }

    /// Finish any open session at its last event and release the database.
    ///
    /// # Errors
    ///
    /// Any `SQLite` failure closing the session or the file.
    pub fn close(mut self, now: f64) -> Result<(), Error> {
        let at = self.live.last_event_at.unwrap_or(now);
        self.finish_session(at)?;
        self.conn.close().map_err(|(_, e)| e)
    }

    /// Record one chunk's facts, stamped `at`.
    ///
    /// # Errors
    ///
    /// Any `SQLite` failure. The whole chunk is rolled back, on disk and in
    /// memory; the recorder is usable afterwards.
    pub fn record_chunk(&mut self, facts: &ChunkFacts, at: f64) -> Result<(), Error> {
        if facts.is_empty() {
            return Ok(());
        }
        self.in_txn(|w| {
            w.chunk_rows = vec![None; facts.events.len()];
            w.chunk_flares = vec![Vec::new(); facts.events.len()];
            let mut counted = false;
            for (index, event) in facts.events.iter().enumerate() {
                if !w.admit(true, !attack::is_foreign(event), at)? {
                    continue;
                }
                if !counted {
                    w.live.chunk_seq += 1;
                    counted = true;
                }
                w.record_attack(index, event, at)?;
            }
            for fact in &facts.facts {
                let ours = status::exist_id(fact).is_some_and(|id| w.knows(id));
                if w.admit(false, ours, at)? {
                    w.record_fact(fact, at)?;
                }
            }
            Ok(())
        })
    }

    /// Run `work` in one transaction; on failure restore [`Live`] too.
    fn in_txn<T>(
        &mut self,
        work: impl FnOnce(&mut Writer<'_>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let before = self.live.clone();
        let tx = self.conn.transaction()?;
        let mut writer = Writer {
            tx: &tx,
            live: &mut self.live,
            character: self.character.as_deref(),
            source: &self.source,
            idle_timeout: self.idle_timeout,
            chunk_rows: Vec::new(),
            chunk_flares: Vec::new(),
        };
        let result = work(&mut writer).and_then(|value| tx.commit().map(|()| value));
        if result.is_err() {
            self.live = before;
        }
        result
    }
}

impl Writer<'_> {
    fn start_session(&mut self, at: f64) -> Result<i64, Error> {
        self.finish_session(at)?;
        self.tx.execute(
            "INSERT INTO sessions (character, source, started_at) VALUES (?, ?, ?)",
            params![self.character.and_then(txt), txt(self.source), at],
        )?;
        let id = self.tx.last_insert_rowid();
        self.live.session_id = Some(id);
        self.live.seq = 0;
        self.live.creature_cache.clear();
        // a new hunt: the previous one takes no more facts
        self.live.closed = None;
        Ok(id)
    }

    fn finish_session(&mut self, at: f64) -> Result<Option<i64>, Error> {
        let Some(id) = self.live.session_id.take() else {
            return Ok(None);
        };
        self.tx.execute(
            "UPDATE sessions SET ended_at = ? WHERE id = ?",
            params![at, id],
        )?;
        self.live.finished.push(id);
        // Remember what the closed session fought: a death confirmed by the
        // room feed can arrive after the gap, and that kill belongs to THIS
        // session's creature rows.
        self.live.closed = Some(ClosedSession {
            id,
            cache: std::mem::take(&mut self.live.creature_cache),
        });
        // Lich also wipes its uid -> row maps here (real db: a status linked
        // to the previous hunt's attack across an idle timeout). There is
        // nothing to wipe: `chunk_rows` lives for one chunk, every item of a
        // chunk shares one `at`, and so a session can only close before the
        // chunk's first row is written.
        self.live.open_attack = None;
        Ok(Some(id))
    }

    fn check_idle(&mut self, now: f64) -> Result<Option<i64>, Error> {
        let (Some(timeout), Some(_), Some(last)) = (
            self.idle_timeout,
            self.live.session_id,
            self.live.last_event_at,
        ) else {
            return Ok(None);
        };
        if now - last > timeout {
            self.finish_session(last)
        } else {
            Ok(None)
        }
    }

    /// Is this creature one the open session -- or the one the gap just
    /// closed -- already recorded against? A bystander's fact about a
    /// creature we never fought matches neither and opens nothing.
    fn knows(&self, exist_id: i64) -> bool {
        self.live.creature_cache.contains_key(&exist_id)
            || self
                .live
                .closed
                .as_ref()
                .is_some_and(|c| c.cache.contains_key(&exist_id))
    }

    /// Re-open the session the gap just closed so a trailing fact files
    /// against its rows. `ended_at` is re-stamped by the next close, which
    /// closes at `last_event_at` -- untouched by a trailing fact.
    fn reopen_closed(&mut self) -> Result<(), Error> {
        let Some(closed) = self.live.closed.clone() else {
            return Ok(());
        };
        self.tx.execute(
            "UPDATE sessions SET ended_at = NULL WHERE id = ?",
            params![closed.id],
        )?;
        self.live.session_id = Some(closed.id);
        self.live.creature_cache = closed.cache;
        Ok(())
    }

    /// The session rules for one item (`record_locked`,
    /// `recorder.rb:529-548`). Returns whether a session is open to take it.
    ///
    /// Ownership is judged by the caller BEFORE this can close anything:
    /// closing wipes the cache that says a delayed fact is ours.
    fn admit(&mut self, is_attack: bool, ours: bool, at: f64) -> Result<bool, Error> {
        if self.idle_timeout.is_some() {
            // A bare fact about a creature we fought belongs to the hunt
            // that fought it: it holds the open session rather than letting
            // the gap split it, never opens a fresh one, and never extends
            // the hunt.
            let trailing = ours && !is_attack;
            if !(trailing && self.live.session_id.is_some()) {
                self.check_idle(at)?;
            }
            if trailing && self.live.session_id.is_none() {
                self.reopen_closed()?;
            }
            if ours && self.live.session_id.is_none() {
                self.start_session(at)?;
            }
            if ours && !trailing {
                self.live.last_event_at = Some(at);
            }
        }
        Ok(self.live.session_id.is_some())
    }
}
