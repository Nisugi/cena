//! Reading the combat recorder: hunts, one hunt, the aggregate, the
//! abilities rollup and the recent attacks, as typed rows.
//!
//! The first of `combat_stats.lic`'s reports (`reference/scripts/scripts/
//! combat_stats.lic`, `list_sessions`, `session_report`, `aggregate_report`,
//! `recount`, `recent_attacks`), over the same [`Reader`] the loot ledger's
//! reports use, since both sets of tables live in one file per character
//! (`plan/34` Stage 4). The rest of that script -- the creature tree, the
//! flare, defense and HP analytics, `blind`/`aim`/`ttk`/`compare` -- are
//! later reports on the same rows; nothing here forecloses them.
//!
//! # Ownership is the recorder's, not the reader's
//!
//! The script's predicates are kept as they are: `attacks.ours` and
//! `flares.ours` were decided at write time, and a hit is ours when its
//! attack is, or when it rode a flare of ours on an inbound row (a shield
//! spike). A kill is ours when the crediting attack is ours by the same
//! rule. The reader only names the columns.

use rusqlite::{OptionalExtension, params};

use crate::ledger::report::{Error, Reader};

/// Our own outbound attack (`OWN_ATTACK`).
const OWN_ATTACK: &str = "a.ours = 1";
/// A flare of ours (`OWN_FLARE`).
const OWN_FLARE: &str = "f.ours = 1";
/// A hit we dealt: its attack is ours, or it rode a flare of ours (`OWN_HIT`).
const OWN_HIT: &str =
    "(a.ours = 1 OR EXISTS (SELECT 1 FROM flares f WHERE f.id = h.flare_id AND f.ours = 1))";
/// The kill is ours: the crediting attack `a` on creature `c` (`OUR_KILL`).
const OUR_KILL: &str = "(a.ours = 1 OR EXISTS (SELECT 1 FROM hits h JOIN flares f ON f.id = h.flare_id \
     WHERE h.attack_id = a.id AND h.creature_id = c.id AND f.ours = 1))";

/// Which hunts a report covers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    /// The most recent hunt.
    Latest,
    /// One hunt by its id.
    Hunt(i64),
    /// Every hunt.
    All,
    /// The last `n` hunts.
    Last(usize),
}

/// One hunt in the list.
#[derive(Clone, Debug, PartialEq)]
pub struct HuntRow {
    /// `sessions.id`.
    pub id: i64,
    /// When it opened.
    pub started_at: f64,
    /// When it closed; `None` while open.
    pub ended_at: Option<f64>,
    /// The last attack's time.
    pub last_event: Option<f64>,
    /// Our own attacks.
    pub attacks: i64,
    /// Our kills.
    pub kills: i64,
    /// Damage we dealt to creatures.
    pub damage: i64,
}

/// One kind of creature over a hunt or hunts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KindRow {
    /// The noun, or `(unknown)`.
    pub noun: String,
    /// Instances seen.
    pub seen: i64,
    /// Killed by us.
    pub kills: i64,
    /// Dead, but not by us.
    pub other_dead: i64,
    /// Not known dead.
    pub alive: i64,
    /// Our attacks on them.
    pub attacks: i64,
    /// Our damage to them.
    pub damage: i64,
}

/// One hunt, or several summed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HuntReport {
    /// The hunts covered, in id order.
    pub hunts: Vec<HuntRow>,
    /// Whose.
    pub character: Option<String>,
    /// Hunt time in seconds, an open hunt counted to its last event.
    pub hunt_time: f64,
    /// Our own attacks.
    pub attacks: i64,
    /// Distinct attack sequences (spawn trees).
    pub sequences: i64,
    /// Attacks against us.
    pub inbound: i64,
    /// Our kills.
    pub kills: i64,
    /// Creatures we damaged that someone else killed.
    pub assists: i64,
    /// Damage we dealt to creatures.
    pub dealt: i64,
    /// Damage we took.
    pub taken: i64,
    /// Creature kinds, most kills first.
    pub creatures: Vec<KindRow>,
}

/// One ability's swings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AbilityRow {
    /// The attack's name.
    pub name: String,
    /// The flare that spawned it, if a cast rode one.
    pub parent: Option<String>,
    /// Swings, misses included.
    pub attempts: i64,
    /// Swings with at least one hit on a creature.
    pub landed: i64,
    /// Damage components.
    pub hits: i64,
    /// Damage in all.
    pub damage: i64,
    /// Hits that critted.
    pub crits: i64,
    /// Fatal hits.
    pub fatal: i64,
}

/// One flare of ours.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlareRow {
    /// The flare's name.
    pub name: String,
    /// Times it fired.
    pub procs: i64,
    /// Hits it dealt.
    pub hits: i64,
    /// Damage in all.
    pub damage: i64,
    /// Fatal hits.
    pub fatal: i64,
}

/// One kind of attack against us.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InboundRow {
    /// The attack's name.
    pub name: String,
    /// Who made it.
    pub attacker: Option<String>,
    /// How many times.
    pub count: i64,
    /// The outcomes seen, comma-joined.
    pub outcomes: String,
    /// Damage to us.
    pub damage: i64,
}

/// The abilities rollup (`recount`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Abilities {
    /// Our abilities, most damage first.
    pub abilities: Vec<AbilityRow>,
    /// Our flares, most damage first.
    pub flares: Vec<FlareRow>,
    /// Attacks against us, most frequent first.
    pub inbound: Vec<InboundRow>,
}

/// One recent attack of ours.
#[derive(Clone, Debug, PartialEq)]
pub struct AttackRow {
    /// `attacks.id`.
    pub id: i64,
    /// The attack's name.
    pub name: String,
    /// When.
    pub at: f64,
    /// The first outcome, if not a plain hit.
    pub outcome: Option<String>,
    /// The creature struck.
    pub target: Option<String>,
    /// Damage in all.
    pub damage: i64,
    /// Flares that fired.
    pub flares: i64,
    /// A released cast echoing its spawner.
    pub echo: bool,
}

impl Reader {
    /// The hunt ids a scope names, newest last; `None` when there are none.
    fn hunt_ids(&self, scope: &Scope) -> Result<Option<Vec<i64>>, Error> {
        let mut ids: Vec<i64> = match scope {
            Scope::Latest => self
                .conn
                .query_row("SELECT MAX(id) FROM sessions", [], |r| {
                    r.get::<_, Option<i64>>(0)
                })?
                .into_iter()
                .collect(),
            Scope::Hunt(id) => self
                .conn
                .query_row("SELECT id FROM sessions WHERE id = ?", [id], |r| r.get(0))
                .optional()?
                .into_iter()
                .collect(),
            Scope::All => {
                let mut stmt = self.conn.prepare("SELECT id FROM sessions ORDER BY id")?;
                let rows = stmt.query_map([], |r| r.get(0))?;
                rows.collect::<Result<_, _>>()?
            }
            Scope::Last(n) => {
                let n = i64::try_from(*n).unwrap_or(i64::MAX).max(1);
                let mut stmt = self
                    .conn
                    .prepare("SELECT id FROM sessions ORDER BY id DESC LIMIT ?")?;
                let rows = stmt.query_map([n], |r| r.get(0))?;
                rows.collect::<Result<_, _>>()?
            }
        };
        ids.sort_unstable();
        Ok((!ids.is_empty()).then_some(ids))
    }

    /// `col IN (1,2,3)`: the ids are the database's own integers.
    fn in_scope(col: &str, ids: &[i64]) -> String {
        let list: Vec<String> = ids.iter().map(i64::to_string).collect();
        format!("{col} IN ({})", list.join(","))
    }

    fn hunt_rows(&self, ids: &[i64]) -> Result<Vec<HuntRow>, Error> {
        let sql = format!(
            "SELECT s.id, s.started_at, s.ended_at, \
                    (SELECT MAX(occurred_at) FROM attacks WHERE session_id = s.id), \
                    (SELECT COUNT(*) FROM attacks a WHERE a.session_id = s.id AND {OWN_ATTACK}), \
                    (SELECT COUNT(*) FROM creatures c JOIN attacks a ON a.id = c.killed_by_attack_id \
                        WHERE c.session_id = s.id AND c.killed_at IS NOT NULL AND {OUR_KILL}), \
                    (SELECT COALESCE(SUM(h.damage), 0) FROM hits h JOIN attacks a ON a.id = h.attack_id \
                        WHERE h.session_id = s.id AND h.creature_id IS NOT NULL AND {OWN_HIT}) \
             FROM sessions s WHERE {} ORDER BY s.id",
            Self::in_scope("s.id", ids)
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |r| {
            Ok(HuntRow {
                id: r.get(0)?,
                started_at: r.get(1)?,
                ended_at: r.get(2)?,
                last_event: r.get(3)?,
                attacks: r.get(4)?,
                kills: r.get(5)?,
                damage: r.get(6)?,
            })
        })?;
        rows.collect()
    }

    /// The last `n` hunts, newest first (`list_sessions`).
    ///
    /// # Errors
    ///
    /// Any `SQLite` failure.
    pub fn hunts(&self, n: usize) -> Result<Vec<HuntRow>, Error> {
        let Some(ids) = self.hunt_ids(&Scope::Last(n))? else {
            return Ok(Vec::new());
        };
        let mut rows = self.hunt_rows(&ids)?;
        rows.reverse();
        Ok(rows)
    }

    /// One hunt, or the scope's hunts summed (`session_report`,
    /// `aggregate_report`). `None` when the scope names no hunt.
    ///
    /// # Errors
    ///
    /// Any `SQLite` failure.
    pub fn hunt(&self, scope: &Scope) -> Result<Option<HuntReport>, Error> {
        let Some(ids) = self.hunt_ids(scope)? else {
            return Ok(None);
        };
        let hunts = self.hunt_rows(&ids)?;
        let hunt_time = hunts
            .iter()
            .map(|h| h.ended_at.or(h.last_event).unwrap_or(h.started_at) - h.started_at)
            .sum();
        let a = Self::in_scope("a.session_id", &ids);
        let h = Self::in_scope("h.session_id", &ids);
        let c = Self::in_scope("c.session_id", &ids);
        let one = |sql: &str| -> Result<i64, Error> {
            self.conn
                .query_row(sql, [], |r| r.get::<_, Option<i64>>(0))
                .map(Option::unwrap_or_default)
        };
        let (attacks, sequences) = self.conn.query_row(
            &format!(
                "SELECT COUNT(*), COUNT(DISTINCT COALESCE(a.root_attack_id, a.id)) \
                 FROM attacks a WHERE {a} AND {OWN_ATTACK}"
            ),
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let inbound = one(&format!(
            "SELECT COUNT(*) FROM attacks a WHERE {a} AND a.inbound = 1"
        ))?;
        let kills = one(&format!(
            "SELECT COUNT(*) FROM creatures c JOIN attacks a ON a.id = c.killed_by_attack_id \
             WHERE {c} AND c.killed_at IS NOT NULL AND {OUR_KILL}"
        ))?;
        let assists = one(&format!(
            "SELECT COUNT(*) FROM creatures c WHERE {c} AND c.killed_at IS NOT NULL \
             AND EXISTS (SELECT 1 FROM hits h JOIN attacks a ON a.id = h.attack_id \
                 WHERE h.creature_id = c.id AND {OWN_HIT}) \
             AND (c.killed_by_attack_id IS NULL OR NOT EXISTS \
                 (SELECT 1 FROM attacks a WHERE a.id = c.killed_by_attack_id AND {OUR_KILL}))"
        ))?;
        let dealt = one(&format!(
            "SELECT COALESCE(SUM(h.damage), 0) FROM hits h JOIN attacks a ON a.id = h.attack_id \
             WHERE {h} AND h.creature_id IS NOT NULL AND {OWN_HIT}"
        ))?;
        let taken = one(&format!(
            "SELECT COALESCE(SUM(h.damage), 0) FROM hits h JOIN attacks a ON a.id = h.attack_id \
             WHERE {h} AND h.creature_id IS NULL AND a.inbound = 1"
        ))?;
        let character = self
            .conn
            .query_row(
                &format!(
                    "SELECT character FROM sessions s WHERE {} ORDER BY s.id DESC LIMIT 1",
                    Self::in_scope("s.id", &ids)
                ),
                [],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        Ok(Some(HuntReport {
            hunts,
            character,
            hunt_time,
            attacks,
            sequences,
            inbound,
            kills,
            assists,
            dealt,
            taken,
            creatures: self.kinds(&c)?,
        }))
    }

    /// Creature kinds over the scoped creatures, most kills first.
    fn kinds(&self, scope: &str) -> Result<Vec<KindRow>, Error> {
        let sql = format!(
            "SELECT COALESCE(c.noun, '(unknown)'), COUNT(*), \
                    SUM(c.killed_at IS NOT NULL), \
                    SUM((SELECT COALESCE({OUR_KILL}, 0) FROM attacks a WHERE a.id = c.killed_by_attack_id)), \
                    SUM((SELECT COUNT(*) FROM attacks a WHERE a.creature_id = c.id AND {OWN_ATTACK})), \
                    SUM((SELECT COALESCE(SUM(h.damage), 0) FROM hits h JOIN attacks a ON a.id = h.attack_id \
                        WHERE h.creature_id = c.id AND {OWN_HIT})) \
             FROM creatures c WHERE {scope} GROUP BY 1 ORDER BY 4 DESC, 2 DESC"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |r| {
            let (seen, dead, kills): (i64, i64, i64) = (
                r.get(1)?,
                r.get::<_, Option<i64>>(2)?.unwrap_or_default(),
                r.get::<_, Option<i64>>(3)?.unwrap_or_default(),
            );
            Ok(KindRow {
                noun: r.get(0)?,
                seen,
                kills,
                other_dead: dead - kills,
                alive: seen - dead,
                attacks: r.get::<_, Option<i64>>(4)?.unwrap_or_default(),
                damage: r.get::<_, Option<i64>>(5)?.unwrap_or_default(),
            })
        })?;
        rows.collect()
    }

    /// Damage by ability, our flares, and the attacks against us
    /// (`recount`). `None` when the scope names no hunt.
    ///
    /// # Errors
    ///
    /// Any `SQLite` failure.
    pub fn abilities(&self, scope: &Scope) -> Result<Option<Abilities>, Error> {
        let Some(ids) = self.hunt_ids(scope)? else {
            return Ok(None);
        };
        let a = Self::in_scope("a.session_id", &ids);
        let own_hit = "h.attack_id = a.id AND h.flare_id IS NULL AND h.creature_id IS NOT NULL";
        let mut stmt = self.conn.prepare(&format!(
            "SELECT a.name, a.parent, COUNT(*), \
                    SUM(EXISTS (SELECT 1 FROM hits h WHERE {own_hit})), \
                    SUM((SELECT COUNT(*) FROM hits h WHERE {own_hit})), \
                    SUM((SELECT COALESCE(SUM(h.damage), 0) FROM hits h WHERE {own_hit})), \
                    SUM((SELECT COUNT(*) FROM hits h WHERE {own_hit} AND h.crit_rank IS NOT NULL)), \
                    SUM((SELECT COUNT(*) FROM hits h WHERE {own_hit} AND h.fatal = 1)) \
             FROM attacks a WHERE {a} AND {OWN_ATTACK} GROUP BY a.name, a.parent ORDER BY 6 DESC"
        ))?;
        let abilities = stmt
            .query_map([], |r| {
                Ok(AbilityRow {
                    name: r.get(0)?,
                    parent: r.get(1)?,
                    attempts: r.get(2)?,
                    landed: r.get::<_, Option<i64>>(3)?.unwrap_or_default(),
                    hits: r.get::<_, Option<i64>>(4)?.unwrap_or_default(),
                    damage: r.get::<_, Option<i64>>(5)?.unwrap_or_default(),
                    crits: r.get::<_, Option<i64>>(6)?.unwrap_or_default(),
                    fatal: r.get::<_, Option<i64>>(7)?.unwrap_or_default(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut stmt = self.conn.prepare(&format!(
            "SELECT f.name, COUNT(DISTINCT f.id), COUNT(h.id), COALESCE(SUM(h.damage), 0), \
                    COALESCE(SUM(h.fatal), 0) \
             FROM flares f JOIN attacks a ON a.id = f.attack_id LEFT JOIN hits h ON h.flare_id = f.id \
             WHERE {a} AND {OWN_FLARE} GROUP BY f.name ORDER BY 4 DESC, 2 DESC"
        ))?;
        let flares = stmt
            .query_map([], |r| {
                Ok(FlareRow {
                    name: r.get(0)?,
                    procs: r.get(1)?,
                    hits: r.get(2)?,
                    damage: r.get(3)?,
                    fatal: r.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut stmt = self.conn.prepare(&format!(
            "SELECT a.name, a.attacker, COUNT(*), COALESCE(GROUP_CONCAT(DISTINCT a.outcome), ''), \
                    SUM((SELECT COALESCE(SUM(h.damage), 0) FROM hits h \
                        WHERE h.attack_id = a.id AND h.creature_id IS NULL)) \
             FROM attacks a WHERE {a} AND a.inbound = 1 GROUP BY a.name, a.attacker ORDER BY 3 DESC"
        ))?;
        let inbound = stmt
            .query_map([], |r| {
                Ok(InboundRow {
                    name: r.get(0)?,
                    attacker: r.get(1)?,
                    count: r.get(2)?,
                    outcomes: r.get(3)?,
                    damage: r.get::<_, Option<i64>>(4)?.unwrap_or_default(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(Abilities {
            abilities,
            flares,
            inbound,
        }))
    }

    /// The last `n` attacks of ours in the latest hunt, newest first
    /// (`recent_attacks`).
    ///
    /// # Errors
    ///
    /// Any `SQLite` failure.
    pub fn recent_attacks(&self, n: usize) -> Result<Vec<AttackRow>, Error> {
        let Some(ids) = self.hunt_ids(&Scope::Latest)? else {
            return Ok(Vec::new());
        };
        let n = i64::try_from(n).unwrap_or(i64::MAX).max(1);
        let mut stmt = self.conn.prepare(&format!(
            "SELECT a.id, a.name, a.occurred_at, a.outcome, c.name, \
                    (SELECT COALESCE(SUM(h.damage), 0) FROM hits h WHERE h.attack_id = a.id), \
                    (SELECT COUNT(*) FROM flares f WHERE f.attack_id = a.id), \
                    (a.parent_confidence = 'unbound') \
             FROM attacks a LEFT JOIN creatures c ON c.id = a.creature_id \
             WHERE a.session_id = ?1 AND {OWN_ATTACK} ORDER BY a.seq DESC LIMIT ?2"
        ))?;
        let rows = stmt.query_map(params![ids[0], n], |r| {
            Ok(AttackRow {
                id: r.get(0)?,
                name: r.get(1)?,
                at: r.get(2)?,
                outcome: r.get(3)?,
                target: r.get(4)?,
                damage: r.get(5)?,
                flares: r.get(6)?,
                echo: r.get::<_, Option<i64>>(7)? == Some(1),
            })
        })?;
        rows.collect()
    }
}
