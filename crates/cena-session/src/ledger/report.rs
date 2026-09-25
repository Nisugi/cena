//! Reading the ledger: the first reports, as typed rows.
//!
//! loottracker's `Reports` module (`loottracker.lic:3419-3878`), over the
//! tables `schema.rs` lays out. Pure queries: a [`Reader`] holds a read-only
//! connection, each report takes a period in server seconds and returns rows,
//! and what the player sees is the caller's to format (`cena/src/loot.rs`).
//! Time boundaries -- midnight, the first of the month, in Eastern time --
//! are the caller's too: the rows are stamped with the prompt's clock, and a
//! report asks for a range of it.
//!
//! # What changed from the script
//!
//! - **One type table.** loottracker typed an item at loot time by its own
//!   noun list and stored the type; `recent gems` and `boxes` here classify
//!   by `gameobj` at read time, the table eloot's `loot_types` uses, so the
//!   ledger and the planner cannot disagree about an item.
//! - **`cap` reports, and does not calculate bonuses.** The author, 2026-09-24:
//!   the trading formula is a sale bonus and lootcap has made it moot. So the
//!   cap report is the month's silver and items with their appraised and
//!   realised values, and stops there.
//! - **No proxy, no `game` column.** One database is one character.

use std::path::Path;

use cena_model::state::gameobj;
use rusqlite::{Connection, OpenFlags, params};

/// A reading failure.
pub type Error = rusqlite::Error;

/// A read-only view of one character's ledger.
#[derive(Debug)]
pub struct Reader {
    conn: Connection,
}

/// A span of server time, `[since, until)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Period {
    /// Inclusive start, server seconds.
    pub since: f64,
    /// Exclusive end, server seconds.
    pub until: f64,
}

/// What a period brought in and sent out.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Summary {
    /// Corpses searched.
    pub searches: i64,
    /// Boxes opened.
    pub boxes_opened: i64,
    /// Skins taken.
    pub skins: i64,
    /// Silver found on corpses.
    pub silvers_search: i64,
    /// Silver gathered from boxes.
    pub silvers_boxes: i64,
    /// Items that entered, finds included.
    pub items: i64,
    /// Silver by counterparty of sale or credit: `(counterparty, total)`.
    pub sales: Vec<(String, i64)>,
    /// Locksmith fees paid.
    pub pool_fees: i64,
    /// Locksmith tips paid.
    pub pool_tips: i64,
    /// Deposited at the bank, notes included.
    pub deposits: i64,
    /// Withdrawn from the bank.
    pub withdrawals: i64,
    /// Bounty silver.
    pub bounty_silver: i64,
    /// Bounty points.
    pub bounty_points: i64,
    /// Bounty experience.
    pub bounty_experience: i64,
}

/// One item, as the ledger last knew it.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    /// The display text.
    pub name: String,
    /// The noun.
    pub noun: String,
    /// `item`, `find`, `content`, `skin` or `seen`.
    pub kind: String,
    /// When it was first mentioned.
    pub at: f64,
    /// A value put on it, if one was.
    pub appraised: Option<i64>,
    /// What it sold for, if it sold.
    pub sold: Option<i64>,
    /// Who bought it.
    pub sold_to: Option<String>,
    /// `worthless` or `too_valuable`, if refused.
    pub refused: Option<String>,
}

/// One box: where it came from and what became of it.
#[derive(Clone, Debug, PartialEq)]
pub struct BoxRow {
    /// The display text.
    pub name: String,
    /// The corpse it came off, when the ledger saw the search.
    pub source: Option<String>,
    /// When it was looted.
    pub at: f64,
    /// Dropped in the pool, and when.
    pub pool_dropped_at: Option<f64>,
    /// Handed back, and when.
    pub returned_at: Option<f64>,
    /// Coins gathered from it, once opened.
    pub silvers: Option<i64>,
    /// Items listed inside it, once opened.
    pub contents: i64,
}

/// One kind of creature over a period.
#[derive(Clone, Debug, PartialEq)]
pub struct CreatureRow {
    /// The name as the search line gave it.
    pub name: String,
    /// Corpses searched.
    pub searches: i64,
    /// Silver found on them.
    pub silvers: i64,
    /// Items they carried.
    pub items: i64,
}

/// The month against the loot cap.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cap {
    /// Corpses searched.
    pub searches: i64,
    /// Boxes looted (by type, at read time).
    pub boxes: i64,
    /// Skins taken.
    pub skins: i64,
    /// Silver found on corpses.
    pub silvers_loose: i64,
    /// Silver gathered from boxes.
    pub silvers_boxes: i64,
    /// Bounty silver.
    pub bounty_silver: i64,
    /// Items by `gameobj` type: `(type, count, estimated, realised)`, where
    /// estimated is the appraisal or, failing one, the sale, and realised is
    /// the sale.
    pub items: Vec<(String, i64, i64, i64)>,
    /// Items the gem shop refused as too valuable and not yet sold.
    pub too_valuable: i64,
}

impl Reader {
    /// Open the database at `path` read-only.
    ///
    /// # Errors
    ///
    /// The file is missing or not a database.
    pub fn open(path: &Path) -> Result<Self, Error> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(Self { conn })
    }

    /// A reader over a connection the caller holds: tests, over the ledger's own.
    #[must_use]
    pub const fn over(conn: Connection) -> Self {
        Self { conn }
    }

    fn sum(&self, sql: &str, period: Period) -> Result<i64, Error> {
        self.conn
            .query_row(sql, params![period.since, period.until], |r| {
                r.get::<_, Option<i64>>(0)
            })
            .map(Option::unwrap_or_default)
    }

    /// Everything in and out over `period` (`summary_for_range`).
    ///
    /// # Errors
    ///
    /// Any `SQLite` failure.
    pub fn summary(&self, period: Period) -> Result<Summary, Error> {
        let events = |kind: &str, what: &str| {
            self.sum(
                &format!(
                    "SELECT {what} FROM loot_events WHERE kind = '{kind}' AND at >= ? AND at < ?"
                ),
                period,
            )
        };
        let money = |category: &str| {
            self.sum(
                &format!(
                    "SELECT sum(amount) FROM transactions WHERE category = '{category}' \
                     AND at >= ? AND at < ?"
                ),
                period,
            )
        };
        let mut sales = Vec::new();
        {
            let mut stmt = self.conn.prepare(
                "SELECT counterparty, sum(amount) FROM transactions \
                 WHERE category IN ('sale', 'credit') AND at >= ? AND at < ? \
                 GROUP BY counterparty ORDER BY sum(amount) DESC",
            )?;
            let rows = stmt.query_map(params![period.since, period.until], |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    r.get(1)?,
                ))
            })?;
            for row in rows {
                sales.push(row?);
            }
        }
        Ok(Summary {
            searches: events("search", "count(*)")?,
            boxes_opened: events("box", "count(*)")?,
            skins: self.sum(
                "SELECT count(*) FROM skin_events WHERE at >= ? AND at < ?",
                period,
            )?,
            silvers_search: events("search", "sum(silvers)")?,
            silvers_boxes: events("box", "sum(silvers)")?,
            items: self.sum(
                "SELECT count(*) FROM loot_items WHERE kind != 'seen' \
                 AND first_seen_at >= ? AND first_seen_at < ?",
                period,
            )?,
            sales,
            pool_fees: money("pool_fee")?,
            pool_tips: money("pool_tip")?,
            deposits: money("deposit")? + money("note_deposit")?,
            withdrawals: money("withdrawal")?,
            bounty_silver: money("bounty")?,
            bounty_points: self.sum(
                "SELECT sum(points) FROM bounty_rewards WHERE at >= ? AND at < ?",
                period,
            )?,
            bounty_experience: self.sum(
                "SELECT sum(experience) FROM bounty_rewards WHERE at >= ? AND at < ?",
                period,
            )?,
        })
    }

    /// The last `limit` items, newest first, of one `gameobj` type when
    /// `kind` is given (`recent_items`).
    ///
    /// # Errors
    ///
    /// Any `SQLite` failure.
    pub fn recent(&self, limit: usize, kind: Option<&str>) -> Result<Vec<Item>, Error> {
        let mut stmt = self.conn.prepare(
            "SELECT name, noun, kind, first_seen_at, appraised_value, sold_value, sold_to, refused \
             FROM loot_items WHERE kind != 'seen' ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Item {
                name: r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                noun: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                kind: r.get(2)?,
                at: r.get(3)?,
                appraised: r.get(4)?,
                sold: r.get(5)?,
                sold_to: r.get(6)?,
                refused: r.get(7)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            let item = row?;
            if kind.is_some_and(|k| !gameobj::classify(&item.noun, &item.name).is(k)) {
                continue;
            }
            out.push(item);
            if out.len() >= limit {
                break;
            }
        }
        Ok(out)
    }

    /// The last `limit` boxes looted, newest first, with what became of each
    /// (`recent_boxes_with_details`).
    ///
    /// # Errors
    ///
    /// Any `SQLite` failure.
    pub fn boxes(&self, limit: usize) -> Result<Vec<BoxRow>, Error> {
        let mut stmt = self.conn.prepare(
            "SELECT i.name, i.noun, i.first_seen_at, i.pool_dropped_at, i.returned_at, \
                    e.source_name, o.silvers, \
                    (SELECT count(*) FROM loot_items c WHERE c.event_id = i.opened_event_id \
                        AND i.opened_event_id IS NOT NULL) \
             FROM loot_items i \
             LEFT JOIN loot_events e ON e.id = i.event_id \
             LEFT JOIN loot_events o ON o.id = i.opened_event_id \
             WHERE i.kind IN ('item', 'content') ORDER BY i.id DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            let noun: String = r.get::<_, Option<String>>(1)?.unwrap_or_default();
            Ok((
                noun,
                BoxRow {
                    name: r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    at: r.get(2)?,
                    pool_dropped_at: r.get(3)?,
                    returned_at: r.get(4)?,
                    source: r.get(5)?,
                    silvers: r.get(6)?,
                    contents: r.get(7)?,
                },
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (noun, item) = row?;
            if !gameobj::classify(&noun, &item.name).is("box") {
                continue;
            }
            out.push(item);
            if out.len() >= limit {
                break;
            }
        }
        Ok(out)
    }

    /// Creatures by what their corpses yielded over `period`, richest first
    /// (`creature_stats`).
    ///
    /// # Errors
    ///
    /// Any `SQLite` failure.
    pub fn creatures(&self, period: Period, limit: usize) -> Result<Vec<CreatureRow>, Error> {
        let mut stmt = self.conn.prepare(
            "SELECT e.source_name, count(*), sum(e.silvers), \
                    (SELECT count(*) FROM loot_items i WHERE i.event_id IN \
                        (SELECT id FROM loot_events s WHERE s.kind = 'search' \
                            AND s.source_name IS e.source_name AND s.at >= ?1 AND s.at < ?2)) \
             FROM loot_events e WHERE e.kind = 'search' AND e.at >= ?1 AND e.at < ?2 \
             GROUP BY e.source_name ORDER BY sum(e.silvers) DESC, count(*) DESC LIMIT ?3",
        )?;
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let rows = stmt.query_map(params![period.since, period.until, limit], |r| {
            Ok(CreatureRow {
                name: r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                searches: r.get(1)?,
                silvers: r.get::<_, Option<i64>>(2)?.unwrap_or_default(),
                items: r.get(3)?,
            })
        })?;
        rows.collect()
    }

    /// The period against the loot cap (`loot_cap_summary`, without the
    /// bonus arithmetic).
    ///
    /// # Errors
    ///
    /// Any `SQLite` failure.
    pub fn cap(&self, period: Period) -> Result<Cap, Error> {
        let mut cap = Cap {
            searches: self.sum(
                "SELECT count(*) FROM loot_events WHERE kind = 'search' AND at >= ? AND at < ?",
                period,
            )?,
            skins: self.sum(
                "SELECT count(*) FROM skin_events WHERE at >= ? AND at < ?",
                period,
            )?,
            silvers_loose: self.sum(
                "SELECT sum(silvers) FROM loot_events WHERE kind = 'search' AND at >= ? AND at < ?",
                period,
            )?,
            silvers_boxes: self.sum(
                "SELECT sum(silvers) FROM loot_events WHERE kind = 'box' AND at >= ? AND at < ?",
                period,
            )?,
            bounty_silver: self.sum(
                "SELECT sum(amount) FROM transactions WHERE category = 'bounty' \
                 AND at >= ? AND at < ?",
                period,
            )?,
            too_valuable: self.sum(
                "SELECT count(*) FROM loot_items WHERE refused = 'too_valuable' \
                 AND sold_at IS NULL AND first_seen_at >= ? AND first_seen_at < ?",
                period,
            )?,
            ..Cap::default()
        };
        // Items by type, typed at read time. An item of several types counts
        // under its first, in the table's order, so the totals add up.
        let mut stmt = self.conn.prepare(
            "SELECT name, noun, appraised_value, sold_value FROM loot_items \
             WHERE kind IN ('item', 'content', 'skin', 'find') \
             AND first_seen_at >= ? AND first_seen_at < ?",
        )?;
        let rows = stmt.query_map(params![period.since, period.until], |r| {
            Ok((
                r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, Option<i64>>(3)?,
            ))
        })?;
        let mut by_type: std::collections::BTreeMap<String, (i64, i64, i64)> =
            std::collections::BTreeMap::new();
        for row in rows {
            let (name, noun, appraised, sold) = row?;
            let types = gameobj::classify(&noun, &name);
            if types.is("box") {
                cap.boxes += 1;
                continue;
            }
            let kind = types
                .types
                .iter()
                .next()
                .cloned()
                .unwrap_or_else(|| "other".to_owned());
            let slot = by_type.entry(kind).or_default();
            slot.0 += 1;
            slot.1 += appraised.or(sold).unwrap_or_default();
            slot.2 += sold.unwrap_or_default();
        }
        cap.items = by_type
            .into_iter()
            .map(|(kind, (count, estimated, realised))| (kind, count, estimated, realised))
            .collect();
        Ok(cap)
    }
}
