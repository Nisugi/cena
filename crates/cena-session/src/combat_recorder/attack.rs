//! One attack event: its row, and the resolutions, flares and hits under it
//! (`record_attack`, `recorder.rb:711-848`), plus the creature registry the
//! rows point at (`ensure_creature`, `recorder.rb:660-693`).

use std::collections::BTreeSet;

use cena_model::state::combat::AttackEvent;
use cena_model::state::combat::event::{Confidence, EventTarget, Hit};
use cena_model::{Actor, BodyPart, Resolution};
use rusqlite::params;

use super::{Error, OpenAttack, Writer, txt};

/// An event that is not ours: a nearby player's attack, a swing at a third
/// party, an effect tick nobody owns, an orphan sink. Recorded for context
/// inside a hunt, but it neither opens a session nor keeps one alive
/// (`foreign_event?`, `recorder.rb:469-473`; 2026-09-07: a bystander's cast
/// in town opened "session 2" of a hunt that had ended).
///
/// Not the negation of `is_ours`: an INBOUND attack is not ours to count as
/// damage dealt, and is very much part of our hunt.
pub(super) fn is_foreign(event: &AttackEvent) -> bool {
    event.foreign_caster || event.is_foreign_target() || event.unowned || event.orphan
}

/// A crit location as Lich's tables spell it: `left arm`, not `left_arm`.
fn spaced(text: &str) -> String {
    text.replace('_', " ")
}

impl Writer<'_> {
    /// The `creatures.id` for an actor, inserting or refreshing its row.
    /// `None` when the actor carries no `exist` id.
    ///
    /// Name and noun backfill through `COALESCE`: a status-first creature is
    /// inserted with no noun, and the later attack that names it must fill it
    /// in or it stays NULL forever (*1 kill / 0 kinds*).
    pub(super) fn ensure_creature(&mut self, actor: &Actor, at: f64) -> Result<Option<i64>, Error> {
        let (Some(exist_id), Some(session)) = (actor.id, self.live.session_id) else {
            return Ok(None);
        };
        let name = txt(&actor.name);
        let noun = actor.noun.as_deref().and_then(txt);
        if let Some(&row) = self.live.creature_cache.get(&exist_id) {
            self.tx.execute(
                "UPDATE creatures SET last_seen = ?, name = COALESCE(name, ?), \
                 noun = COALESCE(noun, ?) WHERE id = ?",
                params![at, name, noun, row],
            )?;
            return Ok(Some(row));
        }
        self.tx.execute(
            "INSERT INTO creatures (session_id, exist_id, noun, name, first_seen, last_seen) \
             VALUES (?, ?, ?, ?, ?, ?) \
             ON CONFLICT (session_id, exist_id) DO UPDATE SET last_seen = excluded.last_seen",
            params![session, exist_id, noun, name, at, at],
        )?;
        let row: i64 = self.tx.query_row(
            "SELECT id FROM creatures WHERE session_id = ? AND exist_id = ?",
            params![session, exist_id],
            |r| r.get(0),
        )?;
        self.live.creature_cache.insert(exist_id, row);
        Ok(Some(row))
    }

    /// Write one event and everything under it, then open its window.
    pub(super) fn record_attack(
        &mut self,
        index: usize,
        event: &AttackEvent,
        at: f64,
    ) -> Result<(), Error> {
        let creature_row = match event.target.creature() {
            Some(actor) => self.ensure_creature(actor, at)?,
            None => None,
        };
        let attack_id = self.insert_attack(index, event, creature_row, at)?;
        if let Some(slot) = self.chunk_rows.get_mut(index) {
            *slot = Some(attack_id);
        }

        let mut touched: BTreeSet<i64> = event.target.id().into_iter().collect();
        let (mut hit_seq, mut res_seq) = (0i64, 0i64);
        for r in &event.resolutions {
            res_seq += 1;
            self.insert_resolution(attack_id, None, res_seq, r)?;
        }
        for hit in &event.hits {
            hit_seq += 1;
            self.insert_hit((attack_id, None), creature_row, hit_seq, hit, at)?;
        }
        let mut flare_ids = Vec::with_capacity(event.flares.len());
        for (i, flare) in event.flares.iter().enumerate() {
            let flare_creature = match &flare.target {
                Some(actor) => self.ensure_creature(actor, at)?,
                None => None,
            };
            touched.extend(flare.target.as_ref().and_then(|a| a.id));
            // A 2p flare is our item firing -- on an inbound row that is our
            // REACTIVE flare; a 3p flare names the creature's or a nearby
            // player's item. Decided here, once.
            let ours = flare.is_ours() && !event.foreign_caster;
            self.tx.execute(
                "INSERT INTO flares (attack_id, seq, name, damaging, creature_id, weapon, outcome, ours) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    attack_id,
                    i64::try_from(i + 1).unwrap_or(i64::MAX),
                    flare.name,
                    flare.damaging,
                    flare_creature,
                    flare.weapon.as_ref().and_then(|w| txt(&w.name)),
                    flare.outcomes.first().map(|o| o.as_str()),
                    ours,
                ],
            )?;
            let flare_id = self.tx.last_insert_rowid();
            flare_ids.push(flare_id);
            for r in &flare.resolutions {
                res_seq += 1;
                self.insert_resolution(attack_id, Some(flare_id), res_seq, r)?;
            }
            for hit in &flare.hits {
                hit_seq += 1;
                let on = flare_creature.or(creature_row);
                self.insert_hit((attack_id, Some(flare_id)), on, hit_seq, hit, at)?;
            }
        }
        if let Some(slot) = self.chunk_flares.get_mut(index) {
            slot.clone_from(&flare_ids);
        }
        self.live.open_attack = Some(OpenAttack {
            id: attack_id,
            touched,
            inbound: event.inbound,
            flare_ids,
        });
        Ok(())
    }

    /// The `attacks` row. Spawn-tree links resolve through this chunk's rows:
    /// a root or parent always emits BEFORE its children, so it is already
    /// written; an unresolvable root is the event itself.
    fn insert_attack(
        &mut self,
        index: usize,
        event: &AttackEvent,
        creature_row: Option<i64>,
        at: f64,
    ) -> Result<i64, Error> {
        let row_of = |i: Option<usize>| i.and_then(|i| self.chunk_rows.get(i).copied().flatten());
        let root_row = row_of(event.root.filter(|&r| r != index));
        let parent_row = row_of(event.parent);
        let target_kind = if creature_row.is_some() {
            "creature"
        } else if event.inbound {
            "self"
        } else if matches!(event.target, EventTarget::Foreign(_)) {
            "foreign"
        } else {
            "none"
        };
        let parent = event.parent_flare.as_ref();
        // flare-spawned but the spawner row could not be asserted: say so in
        // the column instead of leaving readers to infer it
        let confidence = match event.parent_confidence {
            Some(Confidence::Bracket) => Some("bracket"),
            Some(Confidence::Count) => Some("count"),
            None if parent.is_some() && parent_row.is_none() => Some("unbound"),
            None => None,
        };
        let outcomes: Vec<&str> = event.outcomes.iter().map(|o| o.as_str()).collect();
        // only an HONORED redirect changes who the row is about
        let redirected_from = event
            .redirect
            .as_ref()
            .filter(|r| r.honored)
            .map(|r| r.intended.clone());
        self.live.seq += 1;
        self.tx.execute(
            "INSERT INTO attacks (session_id, seq, occurred_at, name, parent, parent_weapon, \
                root_attack_id, parent_attack_id, parent_confidence, via, creature_id, \
                target_kind, attacker, attacker_exist_id, weapon, outcome, outcomes_all, \
                aimed, ambush, attack_kind, inbound, orphan, foreign_caster, unowned, \
                redirected_from, ours, chunk_seq) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                self.live.session_id,
                self.live.seq,
                at,
                event.name,
                parent.map(|p| p.flare.as_str()),
                parent.and_then(|p| p.weapon.as_ref()).and_then(|w| txt(&w.name)),
                root_row,
                parent_row,
                confidence,
                event.via_cast.then_some("cast"),
                creature_row,
                target_kind,
                event.attacker.as_ref().and_then(|a| txt(&a.name)),
                event.attacker.as_ref().and_then(|a| a.id),
                event.weapon.as_deref().and_then(txt),
                outcomes.first(),
                (outcomes.len() > 1).then(|| outcomes.join(",")),
                event.aimed,
                event.ambush,
                event.attack_kind.map(cena_model::AmbushKind::as_str),
                event.inbound,
                event.orphan,
                event.foreign_caster,
                event.unowned,
                redirected_from,
                event.is_ours(),
                self.live.chunk_seq,
            ],
        )?;
        let id = self.tx.last_insert_rowid();
        if root_row.is_none() {
            self.tx.execute(
                "UPDATE attacks SET root_attack_id = ? WHERE id = ?",
                params![id, id],
            )?;
        }
        Ok(id)
    }

    fn insert_resolution(
        &mut self,
        attack_id: i64,
        flare_id: Option<i64>,
        seq: i64,
        r: &Resolution,
    ) -> Result<(), Error> {
        self.tx.execute(
            "INSERT INTO resolutions (attack_id, flare_id, seq, type, attacker_stat, \
                defender_stat, modifier, roll, bonus, penalty, result, total) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                attack_id,
                flare_id,
                seq,
                r.kind.as_str(),
                r.attacker_stat,
                r.defender_stat,
                r.modifier,
                r.roll,
                r.bonus,
                r.penalty,
                r.result,
                r.total,
            ],
        )?;
        Ok(())
    }

    /// One damage fact, its crit denormalised onto the row. `crit_rank` is
    /// the table's severity 0-9 and `wound_rank` the wound left, 0-3: Lich
    /// once conflated them and flattened every crit-rank analytic.
    fn insert_hit(
        &mut self,
        (attack_id, flare_id): (i64, Option<i64>),
        creature_row: Option<i64>,
        seq: i64,
        hit: &Hit,
        at: f64,
    ) -> Result<(), Error> {
        let crit = hit.crit.as_ref();
        // Lich's spellings, which the answer-key database holds: locations
        // spaced, damage types hyphenated, the coup a type of its own.
        let crit_type = crit.map(|c| {
            c.damage_type.map_or_else(
                || "coup_de_grace".to_owned(),
                |t| t.as_str().replace('_', "-"),
            )
        });
        let secondary = crit.and_then(|c| c.secondary_wound.as_ref());
        let fatal = crit.is_some_and(|c| c.fatal);
        self.tx.execute(
            "INSERT INTO hits (attack_id, flare_id, session_id, creature_id, seq, damage, \
                location, body_part, crit_type, crit_rank, wound_rank, fatal, amputated, \
                secondary_location, secondary_rank, line_seq) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                attack_id,
                flare_id,
                self.live.session_id,
                creature_row,
                seq,
                hit.damage,
                crit.map(|c| spaced(c.location.as_str())),
                crit.and_then(|c| BodyPart::from_location(c.location))
                    .map(BodyPart::as_str),
                crit_type,
                crit.and_then(|c| c.rank),
                crit.and_then(|c| c.wound_rank),
                fatal,
                crit.is_some_and(|c| c.amputated),
                secondary.map(|s| spaced(s.location.as_str())),
                secondary.map(|s| s.wound_rank),
                i64::try_from(hit.line).unwrap_or(i64::MAX),
            ],
        )?;
        // A fatal crit is ground truth for the killing blow: it takes the
        // credit even when a room-feed death already stamped `killed_at`,
        // but never moves an earlier `killed_at`.
        if let (true, Some(row)) = (fatal, creature_row) {
            self.tx.execute(
                "UPDATE creatures SET killed_at = COALESCE(killed_at, ?), \
                 killed_by_attack_id = ?, kill_credit = 'crit' WHERE id = ?",
                params![at, attack_id, row],
            )?;
        }
        Ok(())
    }
}
