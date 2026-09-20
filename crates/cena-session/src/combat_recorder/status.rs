//! The status stream: statuses, stuns, roundtimes, spell losses, UCS facts
//! and confirmed deaths (`record_status`, `recorder.rb:935-1057`).
//!
//! # Attribution
//!
//! A fact is filed under an attack three ways, best first:
//!
//! | `source` | When |
//! |---|---|
//! | `event` | the state machine named the event the fact rode. Authoritative over the window: *the last attack of a chunk may be a creature's cast that interrupted ours* |
//! | `window` | no event named, but the last attack recorded touched this creature -- or the fact is about us and that attack was inbound |
//! | `direct` | neither: the fact stands alone |
//!
//! Crit-derived facts always carry their event, so they are `event` here
//! where Lich, emitting them from inside `persist_event`, could only say
//! `window`. The column is a display tag (`combat_stats.lic:1057`).

use cena_model::StatusAction;
use cena_model::state::combat::Fact;
use cena_model::state::combat::event::{LossCause, Subject, UcsKind};
use rusqlite::{OptionalExtension, params};

use super::{Error, Writer, txt};

/// Seconds within which a crit-table stun and its messaging twin are the
/// same swing's (`STUN_PAIR_WINDOW`).
const STUN_PAIR_WINDOW: f64 = 2.0;

/// One `statuses` row, before attribution.
#[derive(Default)]
struct Row<'a> {
    kind: &'static str,
    exist_id: Option<i64>,
    /// As printed; `self` for us.
    name: &'a str,
    noun: Option<&'a str>,
    status: Option<&'a str>,
    action: Option<&'static str>,
    value: Option<i64>,
    spell: Option<i64>,
    spell_name: Option<&'a str>,
    cause: Option<&'static str>,
    event: Option<usize>,
    flare_seq: Option<usize>,
}

/// Where a row was filed.
struct Filed {
    attack_id: Option<i64>,
    flare_id: Option<i64>,
    source: &'static str,
}

/// The `exist` id a fact is keyed to, which is what decides whether a bare
/// fact is ours (`known_creature?`). A fact about us has none.
pub(super) fn exist_id(fact: &Fact) -> Option<i64> {
    match fact {
        Fact::Status {
            subject: Subject::Creature(a),
            ..
        } => a.id,
        Fact::Status {
            subject: Subject::Us,
            ..
        } => None,
        Fact::SpellLoss { subject, .. } => subject.id,
        Fact::Ucs { creature, .. }
        | Fact::Stun { creature, .. }
        | Fact::Roundtime { creature, .. }
        | Fact::Dead { creature } => creature.id,
    }
}

const fn action_str(action: StatusAction) -> &'static str {
    match action {
        StatusAction::Add => "add",
        StatusAction::Remove => "remove",
    }
}

/// A UCS fact's row: the kind is the status, a tier is its ordinal.
fn ucs_row<'a>(creature: &'a cena_model::Actor, kind: &UcsKind) -> Row<'a> {
    let (status, value, followup) = match kind {
        UcsKind::Position(t) => ("position", Some(i64::from(t.ordinal())), None),
        UcsKind::PositionInbound(t) => ("position_inbound", Some(i64::from(t.ordinal())), None),
        // see the module doc of `combat_recorder`: the followup's name has
        // no integer to be, so it rides `spell_name`
        UcsKind::Tierup(attack) => ("tierup", None, Some(attack.as_str())),
        UcsKind::SmiteOn => ("smite_on", None, None),
        UcsKind::SmiteOff => ("smite_off", None, None),
    };
    Row {
        kind: "ucs",
        exist_id: creature.id,
        name: &creature.name,
        noun: creature.noun.as_deref(),
        status: Some(status),
        value,
        spell_name: followup,
        ..Row::default()
    }
}

impl Writer<'_> {
    /// Write one fact as one `statuses` row.
    pub(super) fn record_fact(&mut self, fact: &Fact, at: f64) -> Result<(), Error> {
        let row = match fact {
            Fact::Status {
                subject,
                status,
                action,
                event,
                flare_seq,
                ..
            } => {
                let (exist_id, name, noun) = match subject {
                    Subject::Creature(a) => (a.id, a.name.as_str(), a.noun.as_deref()),
                    Subject::Us => (None, "self", None),
                };
                Row {
                    kind: "status",
                    exist_id,
                    name,
                    noun,
                    status: Some(status.as_str()),
                    action: Some(action_str(*action)),
                    event: *event,
                    flare_seq: *flare_seq,
                    ..Row::default()
                }
            }
            Fact::Stun {
                creature,
                rounds,
                event,
                flare_seq,
            } => Row {
                kind: "stun",
                exist_id: creature.id,
                name: &creature.name,
                noun: creature.noun.as_deref(),
                status: Some("stunned"),
                action: Some("add"),
                value: Some(i64::from(*rounds)),
                event: Some(*event),
                flare_seq: *flare_seq,
                ..Row::default()
            },
            Fact::Roundtime {
                creature,
                seconds,
                event,
                flare_seq,
            } => Row {
                kind: "roundtime",
                exist_id: creature.id,
                name: &creature.name,
                noun: creature.noun.as_deref(),
                status: Some("roundtime"),
                action: Some("add"),
                value: Some(i64::from(*seconds)),
                event: Some(*event),
                flare_seq: *flare_seq,
                ..Row::default()
            },
            Fact::SpellLoss {
                subject,
                spell,
                spell_name,
                cause,
            } => Row {
                kind: "spell_loss",
                exist_id: subject.id,
                name: &subject.name,
                noun: subject.noun.as_deref(),
                action: Some("remove"),
                spell: spell.map(i64::from),
                spell_name: Some(spell_name),
                cause: cause.map(|c| match c {
                    LossCause::Dispel => "dispel",
                    LossCause::Death => "death",
                }),
                ..Row::default()
            },
            Fact::Ucs { creature, kind } => ucs_row(creature, kind),
            Fact::Dead { creature } => Row {
                kind: "status",
                exist_id: creature.id,
                name: &creature.name,
                noun: creature.noun.as_deref(),
                status: Some("dead"),
                action: Some("add"),
                ..Row::default()
            },
        };
        self.record_status(&row, at)
    }

    /// Which attack, and which of its flares, a row belongs under.
    fn file(&self, row: &Row<'_>) -> Filed {
        let flare_at = |ids: &[i64]| {
            row.flare_seq
                .and_then(|seq| seq.checked_sub(1))
                .and_then(|i| ids.get(i).copied())
        };
        let by_event = row
            .event
            .and_then(|i| Some((self.chunk_rows.get(i).copied().flatten()?, i)));
        if let Some((attack_id, i)) = by_event {
            return Filed {
                attack_id: Some(attack_id),
                flare_id: self.chunk_flares.get(i).and_then(|ids| flare_at(ids)),
                source: "event",
            };
        }
        if let Some(open) = &self.live.open_attack {
            if row.exist_id.is_some_and(|id| open.touched.contains(&id)) {
                return Filed {
                    attack_id: Some(open.id),
                    flare_id: flare_at(&open.flare_ids),
                    source: "window",
                };
            }
            if row.name == "self" && open.inbound {
                return Filed {
                    attack_id: Some(open.id),
                    flare_id: None,
                    source: "window",
                };
            }
        }
        Filed {
            attack_id: None,
            flare_id: None,
            source: "direct",
        }
    }

    fn record_status(&mut self, row: &Row<'_>, at: f64) -> Result<(), Error> {
        let creature_row = match row.exist_id {
            Some(id) => {
                let actor = cena_model::Actor {
                    id: Some(id),
                    noun: row.noun.map(str::to_owned),
                    name: row.name.to_owned(),
                };
                self.ensure_creature(&actor, at)?
            }
            None => None,
        };
        let filed = self.file(row);
        if !self.merge_stun_twin(row, creature_row, &filed, at)? {
            self.tx.execute(
                "INSERT INTO statuses (session_id, creature_id, subject, attack_id, flare_id, \
                    occurred_at, kind, status, action, value, spell, spell_name, cause, source) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    self.live.session_id,
                    creature_row,
                    txt(row.name),
                    filed.attack_id,
                    filed.flare_id,
                    at,
                    row.kind,
                    row.status.and_then(txt),
                    row.action,
                    row.value,
                    row.spell,
                    row.spell_name.and_then(txt),
                    row.cause,
                    filed.source,
                ],
            )?;
        }
        if let (Some("dead"), Some("add"), Some(creature)) = (row.status, row.action, creature_row)
        {
            self.credit_kill(creature, filed.attack_id, at)?;
        }
        Ok(())
    }

    /// One swing's stun arrives twice: the crit table's `stun` row, with the
    /// round count, and the messaging *"X is stunned!"* `status` row. Readers
    /// folded them inconsistently, so they merge into ONE row -- kind `stun`,
    /// the count kept, the better attack link kept -- whichever came first.
    ///
    /// The twin must be the SAME swing's: the same attack when both know
    /// theirs, no stun removal in between, and a row that already absorbed a
    /// twin is not reused. Returns whether the row was merged away.
    fn merge_stun_twin(
        &mut self,
        row: &Row<'_>,
        creature_row: Option<i64>,
        filed: &Filed,
        at: f64,
    ) -> Result<bool, Error> {
        let is_stun = row.kind == "stun";
        let stun_add = is_stun
            || (row.kind == "status" && row.status == Some("stunned") && row.action == Some("add"));
        let (true, Some(creature)) = (stun_add, creature_row) else {
            return Ok(false);
        };
        let session = self.live.session_id;
        let twin: Option<i64> = self
            .tx
            .query_row(
                "SELECT id FROM statuses \
                 WHERE session_id = ?1 AND creature_id = ?2 AND kind = ?3 \
                   AND status = 'stunned' AND action = 'add' AND occurred_at >= ?4 \
                   AND (attack_id IS NULL OR ?5 IS NULL OR attack_id = ?5) \
                   AND id > COALESCE((SELECT MAX(id) FROM statuses \
                        WHERE session_id = ?1 AND creature_id = ?2 \
                          AND status = 'stunned' AND action = 'remove'), 0) \
                 ORDER BY id DESC LIMIT 1",
                params![
                    session,
                    creature,
                    if is_stun { "status" } else { "stun" },
                    at - STUN_PAIR_WINDOW,
                    filed.attack_id,
                ],
                |r| r.get(0),
            )
            .optional()?;
        let Some(twin) = twin.filter(|id| !self.live.merged_stun_rows.contains(id)) else {
            return Ok(false);
        };
        self.live.merged_stun_rows.insert(twin);
        self.tx.execute(
            "UPDATE statuses SET kind = 'stun', value = COALESCE(?1, value), \
                attack_id = COALESCE(attack_id, ?2), flare_id = COALESCE(flare_id, ?3), \
                source = CASE WHEN attack_id IS NULL THEN ?4 ELSE source END \
             WHERE id = ?5",
            params![
                row.value.filter(|_| is_stun),
                filed.attack_id,
                filed.flare_id,
                filed.source,
                twin,
            ],
        )?;
        Ok(true)
    }

    /// A death the room feed confirmed, rather than a fatal crit: stamp the
    /// kill and say how the credit was chosen.
    ///
    /// With an attack in hand the credit is `window`. With none -- the death
    /// lagged past the swing -- it goes to the LAST damaging attack on the
    /// creature from ANY source, then says whether that was ours
    /// (`last_own_hit`: an own swing, or our reactive flare on an inbound
    /// row) or someone else's (`last_hit`). Filtering to our hits first would
    /// credit our earlier hit over another player's later one.
    ///
    /// The ordering is the chunk's time, then which chunk (prompt times are
    /// whole seconds), then the damage line's position in it, then insertion:
    /// row id alone credited a released spell over the swing that landed
    /// after it. A fatal crit's credit is never overwritten: a crit always
    /// stamps `killed_at`, and this `UPDATE` only touches rows where it is
    /// NULL. The `COALESCE`s are Lich's and are belt over those braces --
    /// removing one changes nothing a test can see.
    fn credit_kill(&mut self, creature: i64, attack_id: Option<i64>, at: f64) -> Result<(), Error> {
        let mut credit = attack_id.map(|id| (id, "window"));
        if credit.is_none() {
            credit = self
                .tx
                .query_row(
                    "SELECT a.id, (a.ours = 1 OR COALESCE(f.ours, 0) = 1) FROM hits h \
                     JOIN attacks a ON a.id = h.attack_id \
                     LEFT JOIN flares f ON f.id = h.flare_id \
                     WHERE h.creature_id = ? AND a.session_id = ? AND h.damage > 0 \
                     ORDER BY a.occurred_at DESC, COALESCE(a.chunk_seq, -1) DESC, \
                              COALESCE(h.line_seq, -1) DESC, h.id DESC LIMIT 1",
                    params![creature, self.live.session_id],
                    |r| Ok((r.get::<_, i64>(0)?, r.get::<_, bool>(1)?)),
                )
                .optional()?
                .map(|(id, ours)| (id, if ours { "last_own_hit" } else { "last_hit" }));
        }
        self.tx.execute(
            "UPDATE creatures SET killed_at = ?, \
                killed_by_attack_id = COALESCE(killed_by_attack_id, ?), \
                kill_credit = COALESCE(kill_credit, ?) \
             WHERE id = ? AND killed_at IS NULL",
            params![at, credit.map(|c| c.0), credit.map(|c| c.1), creature],
        )?;
        Ok(())
    }
}
