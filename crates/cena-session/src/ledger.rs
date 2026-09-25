//! The loot ledger: a chunk's loot facts, persisted to `SQLite`.
//!
//! The recording half of the author's `loottracker.lic` (`plan/34` §4), beside
//! the combat recorder and on its pattern: the facts are classified in
//! `cena-model` (`state/ledger.rs`) and arrive here whole, one prompt-bounded
//! chunk at a time, stamped with the server's clock; one chunk is one
//! transaction, and a failed chunk is rolled back on disk and in memory
//! together. The worker moves the ledger onto its own thread and the actor
//! offers chunks to it without waiting ([`worker`], `actor/combat.rs`).
//!
//! # What the processors did in Ruby, done here
//!
//! The classifier is stateless. What loottracker's processors kept between
//! chunks is the ledger's private `Live`:
//!
//! - **an offer awaiting its answer**: `You offer to sell your <item>` is one
//!   prompt, the pawnbroker's coins, chit or refusal the next;
//! - **a look awaiting its figure**: a shopkeeper turns the item over, then
//!   speaks; a bard's loresong resonates, then values;
//! - **a pool quote awaiting its drop**: the quote names the box by link, the
//!   confirmation only by noun.
//!
//! Each is held for [`PAIR_WINDOW`] seconds of server time and then let go,
//! so a merchant who never answers cannot claim the next sale.
//!
//! # Linking an item
//!
//! Every fact about an item after it was looted -- appraised, dropped in the
//! pool, returned, sold, refused, shattered -- finds its row as **the most
//! recent `loot_items` row with that `exist` id that is not yet gone**. An
//! item never looted here (bought, or looted before this database existed)
//! gets a row of kind `seen` at its first mention, so the sale is still
//! recorded against something. A box the pool hands back may come back under
//! a new `exist` id: it is found as the latest box of that noun dropped in
//! **this room** and not yet returned, and its row takes the new id, so the
//! opening that follows links to the box that was dropped. That is the
//! Red Forest fix (`plan/34` §3): the room is the wire's id, one string, and
//! never a map room's list of uids.
//!
//! # Not here
//!
//! No sessions: a search is stamped with its time and room, and a report
//! that wants hunts can join the combat recorder's `sessions` table in the
//! same file. No `Time.now`: every `at` is the prompt's.

pub mod schema;
pub mod worker;
mod write;

use std::path::Path;

use cena_model::state::containers::ItemRef;
use cena_model::state::ledger::LootChunk;
use rusqlite::{Connection, Transaction};

/// A recording failure. The chunk it interrupted was rolled back whole.
pub type Error = rusqlite::Error;

/// How long, in server seconds, a first half waits for its second.
pub const PAIR_WINDOW: f64 = 120.0;

/// A first line whose second line has not arrived.
#[derive(Debug, Clone, PartialEq)]
enum Half {
    /// `You offer to sell your <item>` / `You ask <who> … <item>`.
    Offered(ItemRef),
    /// A shopkeeper turned `<item>` over in their hands.
    ShopLooking(ItemRef),
    /// A loresong resonated from `<item>`.
    Singing(ItemRef),
}

/// A pool quote, awaiting the drop that names the box only by noun.
#[derive(Debug, Clone, PartialEq)]
struct Quote {
    row: i64,
    noun: String,
    at: f64,
}

/// Everything the ledger remembers between chunks. `Clone` so a failed
/// chunk can be undone in memory as the transaction undoes it on disk.
#[derive(Debug, Clone, Default, PartialEq)]
struct Live {
    half: Option<(Half, f64)>,
    quotes: Vec<Quote>,
}

/// Persists loot facts to one `SQLite` database.
#[derive(Debug)]
pub struct Ledger {
    conn: Connection,
    character: Option<String>,
    live: Live,
}

/// One chunk's write, borrowing the ledger's halves disjointly.
struct Writer<'a> {
    tx: &'a Transaction<'a>,
    live: &'a mut Live,
    character: Option<&'a str>,
    room: Option<&'a str>,
    at: f64,
}

impl Ledger {
    /// Open (creating if absent) the database at `path`; the tables are
    /// created if missing, beside whatever else the file holds.
    ///
    /// # Errors
    ///
    /// Any `SQLite` failure opening the file or creating the schema.
    pub fn open(path: &Path, character: Option<&str>) -> Result<Self, Error> {
        Self::with_connection(Connection::open(path)?, character)
    }

    /// A private in-memory database: tests, and a dry run.
    ///
    /// # Errors
    ///
    /// Any `SQLite` failure creating the schema.
    pub fn in_memory(character: Option<&str>) -> Result<Self, Error> {
        Self::with_connection(Connection::open_in_memory()?, character)
    }

    fn with_connection(conn: Connection, character: Option<&str>) -> Result<Self, Error> {
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
        conn.execute_batch("PRAGMA synchronous = NORMAL;")?;
        conn.execute_batch(schema::SCHEMA)?;
        conn.execute(
            "INSERT OR REPLACE INTO ledger_meta (key, value) VALUES ('version', ?)",
            [schema::LEDGER_VERSION],
        )?;
        Ok(Self {
            conn,
            character: character.map(str::to_owned),
            live: Live::default(),
        })
    }

    /// The database, for reports and tests. Read it; do not write the
    /// ledger's tables behind its back.
    #[must_use]
    pub const fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Record one chunk's facts, stamped `at`.
    ///
    /// # Errors
    ///
    /// Any `SQLite` failure. The whole chunk is rolled back, on disk and in
    /// memory; the ledger is usable afterwards.
    pub fn record(&mut self, chunk: &LootChunk, at: f64) -> Result<(), Error> {
        if chunk.facts.is_empty() {
            return Ok(());
        }
        let before = self.live.clone();
        let tx = self.conn.transaction()?;
        let mut writer = Writer {
            tx: &tx,
            live: &mut self.live,
            character: self.character.as_deref(),
            room: chunk.room.as_deref(),
            at,
        };
        let result = writer.record(chunk).and_then(|()| tx.commit());
        if result.is_err() {
            self.live = before;
        }
        result
    }

    /// Release the database.
    ///
    /// # Errors
    ///
    /// Any `SQLite` failure closing the file.
    pub fn close(self) -> Result<(), Error> {
        self.conn.close().map_err(|(_, e)| e)
    }
}
