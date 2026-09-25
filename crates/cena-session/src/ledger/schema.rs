//! The ledger's tables: loottracker's six, reshaped as `plan/34` §4 records.
//!
//! **Fresh, not verbatim.** Unlike the combat recorder, whose schema is
//! Lich's column for column because the author's `combat_stats.db` is the
//! per-attack answer key, there is no history to keep here (author,
//! 2026-09-24: *"no history, all new"*). So the eight migrations that grew
//! `loot_items` are one `CREATE`, the `year/month/day/hour` columns are gone
//! (derive them from `at`), and the room column holds the wire's `<nav rm=>`
//! id rather than a map room's list of uids (§3's Red Forest bug).
//!
//! # Keys
//!
//! An item's `exist` id is the game's and **repeats across sessions**, so it
//! is a column, not a key: rows are keyed by row id and linked by row id, and
//! the ledger finds "the item with this exist id" as *the most recent row
//! with it that is still open* (`write.rs`). loottracker's unique index on
//! `(item_id, item_noun)` guarded against double-recording within a session
//! and broke across them.
//!
//! These tables live in the same file as the combat recorder's seven
//! (`worker::open_live`), so a report that wants kills against loot per
//! creature is one query; both writers use WAL and a busy timeout.

/// `PRAGMA user_version` is the combat schema's; the ledger's own version
/// lives in its own table so the two can migrate independently.
pub const LEDGER_VERSION: i64 = 1;

/// The schema, idempotent.
pub const SCHEMA: &str = r"
CREATE TABLE IF NOT EXISTS ledger_meta (
  key         TEXT PRIMARY KEY,
  value       INTEGER NOT NULL
);

-- A search, a box opened, or a wand duplicated: something that produced loot.
CREATE TABLE IF NOT EXISTS loot_events (
  id            INTEGER PRIMARY KEY,
  character     TEXT,
  kind          TEXT NOT NULL,              -- search | box | wand_dupe
  source_exist  INTEGER,                    -- the corpse, the box, the donor wand
  source_noun   TEXT,
  source_name   TEXT,
  silvers       INTEGER NOT NULL DEFAULT 0,
  room_id       TEXT,                       -- the wire's <nav rm=>
  at            REAL NOT NULL
);

-- Every item that entered the character's hands, and what became of it.
CREATE TABLE IF NOT EXISTS loot_items (
  id            INTEGER PRIMARY KEY,
  character     TEXT,
  exist_id      INTEGER,
  noun          TEXT,
  name          TEXT,
  kind          TEXT NOT NULL,              -- item | find | content | skin | seen
  event_id      INTEGER REFERENCES loot_events(id), -- NULL for an item first seen at a shop
  first_seen_at REAL NOT NULL,
  first_room_id TEXT,
  -- a box
  opened_event_id INTEGER REFERENCES loot_events(id),
  pool_tip      INTEGER,
  pool_fee      INTEGER,
  pool_room_id  TEXT,
  pool_dropped_at REAL,
  returned_at   REAL,
  -- valued
  appraised_value INTEGER,
  appraised_by  TEXT,                       -- gem | skin | loresong | shop
  appraised_at  REAL,
  -- gone
  sold_value    INTEGER,
  sold_to       TEXT,                       -- pawn | gemshop | furrier | chronomage
  sold_note_exist INTEGER,
  sold_at       REAL,
  sold_room_id  TEXT,
  refused       TEXT,                       -- worthless | too_valuable
  shattered_at  REAL
);
CREATE INDEX IF NOT EXISTS loot_items_exist ON loot_items (exist_id, id);

CREATE TABLE IF NOT EXISTS skin_events (
  id              INTEGER PRIMARY KEY,
  character       TEXT,
  creature_exist  INTEGER,
  creature_noun   TEXT,
  creature_name   TEXT,
  skin_item_id    INTEGER REFERENCES loot_items(id),
  room_id         TEXT,
  at              REAL NOT NULL
);

CREATE TABLE IF NOT EXISTS bundle_events (
  id              INTEGER PRIMARY KEY,
  character       TEXT,
  skin_exist      INTEGER,
  bundle_exist    INTEGER,
  bundle_noun     TEXT,
  bundle_name     TEXT,
  container_exist INTEGER,
  created         INTEGER NOT NULL,
  at              REAL NOT NULL
);

-- Silver in and out.
CREATE TABLE IF NOT EXISTS transactions (
  id            INTEGER PRIMARY KEY,
  character     TEXT,
  category      TEXT NOT NULL,              -- sale | credit | deposit | withdrawal | note_deposit | pool_fee | pool_tip | bounty
  amount        INTEGER NOT NULL,           -- the figure, always positive; category says which way
  item_id       INTEGER REFERENCES loot_items(id),
  counterparty  TEXT,                       -- pawn | gemshop | furrier | chronomage | bank | locksmith | bounty
  room_id       TEXT,
  at            REAL NOT NULL
);

CREATE TABLE IF NOT EXISTS bounty_rewards (
  id            INTEGER PRIMARY KEY,
  character     TEXT,
  points        INTEGER NOT NULL,
  experience    INTEGER NOT NULL,
  silvers       INTEGER NOT NULL,
  at            REAL NOT NULL
);
";
