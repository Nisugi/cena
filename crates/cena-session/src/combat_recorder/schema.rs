//! The seven tables, verbatim from `recorder.rb:110-243`.
//!
//! Verbatim on purpose: the author's `combat_stats.db` is the per-attack
//! answer key (`inventory/11` §4), and the `cstats` reports are SQL over
//! exactly these columns. A database written here opens in those reports,
//! and one written by Lich opens here. The comments are Lich's.
//!
//! No `ADDED_COLUMNS` migration map came with it: every column Lich added
//! after its first release is already in this `CREATE`, and there are no
//! older Cena databases to alter. [`USER_VERSION`] is stamped so the first
//! real migration has something to read.

/// `PRAGMA user_version` for this schema.
pub const USER_VERSION: i64 = 1;

/// The schema, idempotent.
pub const SCHEMA: &str = r"
CREATE TABLE IF NOT EXISTS sessions (
  id          INTEGER PRIMARY KEY,
  character   TEXT,
  source      TEXT,
  started_at  REAL NOT NULL,
  ended_at    REAL
);

CREATE TABLE IF NOT EXISTS creatures (
  id          INTEGER PRIMARY KEY,
  session_id  INTEGER NOT NULL REFERENCES sessions(id),
  exist_id    INTEGER,
  noun        TEXT,
  name        TEXT,
  first_seen  REAL,
  last_seen   REAL,
  killed_at   REAL,
  killed_by_attack_id INTEGER,
  kill_credit TEXT,                             -- how killed_by_attack_id was chosen: 'crit' (fatal crit, ground truth) | 'window' (room-feed death inside an attack window) | 'last_own_hit' (no window: the last damaging attack on it was ours) | 'last_hit' (no window: the last damaging attack on it was someone else's) | NULL (unknown / pre-migration)
  UNIQUE (session_id, exist_id)
);

CREATE TABLE IF NOT EXISTS attacks (
  id          INTEGER PRIMARY KEY,
  session_id  INTEGER NOT NULL REFERENCES sessions(id),
  seq         INTEGER NOT NULL,             -- monotonic within session
  occurred_at REAL NOT NULL,
  name        TEXT NOT NULL,                -- def name (:unknown = orphan sink)
  parent      TEXT,                         -- spawned-cast lineage (flare name)
  parent_weapon TEXT,                       -- the weapon whose flare spawned it
  root_attack_id   INTEGER REFERENCES attacks(id), -- initiating shot of this blob's spawn tree (self for a root)
  parent_attack_id INTEGER REFERENCES attacks(id), -- immediate spawner, ONLY when asserted (blink bracket); NULL when ambiguous
  parent_confidence TEXT,                   -- 'bracket' (game-declared) | NULL (unproven); reserved: 'count'
  via         TEXT,                         -- gesture wrapper (:cast)
  creature_id INTEGER REFERENCES creatures(id),  -- NULL: inbound/self/foreign/orphan
  target_kind TEXT NOT NULL,                -- creature|self|foreign|none
  attacker    TEXT,                         -- inbound: who attacked us
  attacker_exist_id INTEGER,
  weapon      TEXT,
  outcome     TEXT,                         -- first outcome (miss/evade/warded/...)
  outcomes_all TEXT,                        -- comma-joined when >1
  aimed       INTEGER NOT NULL DEFAULT 0,
  ambush      INTEGER NOT NULL DEFAULT 0,
  attack_kind TEXT,                         -- named maneuver: 'waylay'/'ambush' (from hiding, also sets ambush=1) or 'reverse_strike' (parry reaction, ambush=0); NULL when not an ambush, or recorded before this column existed
  inbound     INTEGER NOT NULL DEFAULT 0,
  orphan      INTEGER NOT NULL DEFAULT 0,
  foreign_caster INTEGER NOT NULL DEFAULT 0, -- a nearby player's attack (observed, not ours)
  unowned     INTEGER NOT NULL DEFAULT 0,    -- effect tick, no owning cast: applied to creature, not our deal
  ours        INTEGER NOT NULL DEFAULT 0,    -- decided at write time: our own OUTBOUND attack - not inbound, not a nearby player's, not on a foreign target, not an unowned tick, not an orphan sink
  chunk_seq   INTEGER,                       -- the recorder's own count of chunks seen (monotonic for its lifetime): orders chunks that share a whole-second occurred_at, where hits.line_seq restarts
  redirected_from TEXT                       -- guardian redirect: noun of the creature we struck AT; the row's creature is the guardian that took it
);
CREATE INDEX IF NOT EXISTS idx_attacks_session ON attacks(session_id, seq);
CREATE INDEX IF NOT EXISTS idx_attacks_creature ON attacks(creature_id);
CREATE INDEX IF NOT EXISTS idx_attacks_name ON attacks(session_id, name);
CREATE INDEX IF NOT EXISTS idx_attacks_root ON attacks(root_attack_id);

-- attacker_stat/defender_stat/modifier by type:
--   as_ds:  AS  / DS  / AvD      cs_td: CS  / TD  / CvA
--   uaf_udf:UAF / UDF / MM       fear:  FS  / FD  / FvP
--   smr/ssr/maneuver_roll/activation: NULLs + roll/bonus/penalty/result
CREATE TABLE IF NOT EXISTS resolutions (
  id          INTEGER PRIMARY KEY,
  attack_id   INTEGER NOT NULL REFERENCES attacks(id),
  flare_id    INTEGER REFERENCES flares(id),   -- NULL = the attack's own roll
  seq         INTEGER NOT NULL,                -- order within the attack
  type        TEXT NOT NULL,
  attacker_stat INTEGER,
  defender_stat INTEGER,
  modifier    INTEGER,
  roll        INTEGER,                         -- the die, kept separate:
  bonus       INTEGER,                         -- result - roll = margin
  penalty     INTEGER,
  result      INTEGER,
  total       REAL                             -- UCS pre-MM total (fractional)
);
CREATE INDEX IF NOT EXISTS idx_resolutions_attack ON resolutions(attack_id);

CREATE TABLE IF NOT EXISTS flares (
  id          INTEGER PRIMARY KEY,
  attack_id   INTEGER NOT NULL REFERENCES attacks(id),
  seq         INTEGER NOT NULL,
  name        TEXT NOT NULL,
  damaging    INTEGER NOT NULL DEFAULT 0,
  creature_id INTEGER REFERENCES creatures(id), -- AoE: may differ from swing
  weapon      TEXT,
  outcome     TEXT,
  ours        INTEGER NOT NULL DEFAULT 0     -- 2p flare (our item fired). On an INBOUND row ours=1 is our REACTIVE flare (shield spike, thorns); ours=0 there is the creature's own weapon proc striking us
);
CREATE INDEX IF NOT EXISTS idx_flares_attack ON flares(attack_id);
CREATE INDEX IF NOT EXISTS idx_flares_name ON flares(name);

CREATE TABLE IF NOT EXISTS hits (
  id          INTEGER PRIMARY KEY,
  attack_id   INTEGER NOT NULL REFERENCES attacks(id),
  flare_id    INTEGER REFERENCES flares(id),    -- NULL = attack's own damage
  session_id  INTEGER NOT NULL REFERENCES sessions(id),
  creature_id INTEGER REFERENCES creatures(id), -- NULL = inbound/self/orphan
  seq         INTEGER NOT NULL,
  damage      INTEGER NOT NULL,
  location    TEXT,                             -- CritRanks location
  body_part   TEXT,                             -- mapped injury-doll part
  crit_type   TEXT,
  crit_rank   INTEGER,                          -- CritRanks :rank (0-9): crit severity
  wound_rank  INTEGER,                          -- CritRanks :wound_rank (0-3): wound left on the creature
  fatal       INTEGER NOT NULL DEFAULT 0,
  amputated   INTEGER NOT NULL DEFAULT 0,
  secondary_location TEXT,
  secondary_rank     INTEGER,                   -- secondary wound severity (:wound_rank)
  line_seq    INTEGER                           -- feed position of the damage line within its chunk: COMBAT order. Row id is insertion order, which a released cast (emitted after its swing) breaks; attacks.occurred_at orders across chunks
);
CREATE INDEX IF NOT EXISTS idx_hits_attack ON hits(attack_id);
CREATE INDEX IF NOT EXISTS idx_hits_creature ON hits(session_id, creature_id);
CREATE INDEX IF NOT EXISTS idx_hits_location ON hits(location);
CREATE INDEX IF NOT EXISTS idx_hits_crit ON hits(crit_rank);
CREATE INDEX IF NOT EXISTS idx_hits_creature_only ON hits(creature_id);  -- per-creature damage subqueries (the composite needs session_id first)
CREATE INDEX IF NOT EXISTS idx_hits_flare ON hits(flare_id);

-- kind: status (action add/remove), stun (value=rounds),
--       roundtime (value=seconds), spell_loss (spell/spell_name/cause),
--       ucs (status=position/tierup/..., value=tier)
CREATE TABLE IF NOT EXISTS statuses (
  id          INTEGER PRIMARY KEY,
  session_id  INTEGER NOT NULL REFERENCES sessions(id),
  creature_id INTEGER REFERENCES creatures(id), -- NULL = self or unresolved
  subject     TEXT,                             -- name as printed ('self' for us)
  attack_id   INTEGER REFERENCES attacks(id),   -- window attribution
  flare_id    INTEGER REFERENCES flares(id),    -- the flare it rode on (glowbark blind), else NULL
  occurred_at REAL NOT NULL,
  kind        TEXT NOT NULL,
  status      TEXT,
  action      TEXT,
  value       INTEGER,
  spell       INTEGER,
  spell_name  TEXT,
  cause       TEXT,
  source      TEXT                              -- 'direct' | 'window'
);
CREATE INDEX IF NOT EXISTS idx_statuses_creature ON statuses(session_id, creature_id);
CREATE INDEX IF NOT EXISTS idx_statuses_attack ON statuses(attack_id);
";
