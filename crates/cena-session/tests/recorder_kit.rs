//! Shared scaffolding for the combat recorder tests. Not a test file itself.
//!
//! Two ways to make a chunk's facts, deliberately:
//!
//! - [`feed`] sends real feed text through the parser, the chunk and the
//!   state machine, which is how the recorder will be fed in production.
//! - [`attack`] and friends build an event by hand, for the shapes Lich's
//!   `recorder_spec.rb` builds by hand too: a nearby player's cast, a blink
//!   child, a death that arrives with no attack beside it.
//!
//! Nothing here panics: it is a plain module with no `#[test]` in it, so
//! `clippy.toml`'s `allow-panic-in-tests` does not reach it.

// Each test binary uses its own subset of these.
#![allow(dead_code)]

use std::sync::Arc;

use cena_model::crit::CritTables;
use cena_model::state::combat::event::{EventTarget, Hit, Subject};
use cena_model::state::combat::{AttackEvent, ChunkFacts, Fact};
use cena_model::{Actor, GameState, StatusAction, StatusName};
use cena_protocol::Parser;
use cena_session::combat_recorder::CombatRecorder;
use rusqlite::types::FromSql;

/// A bolded creature link, as the feed prints one.
#[must_use]
pub fn bolded(id: i64, noun: &str, name: &str) -> String {
    format!(r#"<pushBold/><a exist="{id}" noun="{noun}">{name}</a><popBold/>"#)
}

/// The creature most tests fight.
#[must_use]
pub fn lizard() -> Actor {
    Actor {
        id: Some(101),
        noun: Some("lizard".to_owned()),
        name: "a cave lizard".to_owned(),
    }
}

/// Feed one chunk through the real pipeline and close it with a prompt.
pub fn feed(state: &mut GameState, lines: &[&str]) -> ChunkFacts {
    if state.combat().crit_tables().is_none()
        && let Ok(tables) = CritTables::load()
    {
        state.combat_mut().set_crit_tables(Arc::new(tables));
    }
    let mut wire: String = lines.iter().flat_map(|l| [l, "\n"]).collect();
    wire.push_str("<prompt time=\"1\">&gt;</prompt>\n");
    let mut parser = Parser::new();
    for frame in parser.push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
    state.combat_mut().take_facts().pop().unwrap_or_default()
}

/// Our own swing at `target` for `damage`.
#[must_use]
pub fn attack(target: &Actor, damage: u32) -> AttackEvent {
    let mut e = AttackEvent::default();
    "attack".clone_into(&mut e.name);
    e.target = EventTarget::Creature(target.clone());
    e.root = Some(0);
    if damage > 0 {
        e.hits.push(Hit {
            damage,
            crit: None,
            line: 1,
        });
    }
    e
}

/// A chunk holding just these events.
#[must_use]
pub fn chunk(events: Vec<AttackEvent>) -> ChunkFacts {
    ChunkFacts {
        events,
        facts: Vec::new(),
    }
}

/// A chunk holding just these facts.
#[must_use]
pub fn facts(facts: Vec<Fact>) -> ChunkFacts {
    ChunkFacts {
        events: Vec::new(),
        facts,
    }
}

/// A message status on a creature, riding no event.
#[must_use]
pub fn status(who: &Actor, status: StatusName, action: StatusAction) -> Fact {
    Fact::Status {
        subject: Subject::Creature(who.clone()),
        status,
        action,
        event: None,
        flare_seq: None,
        line: 0,
    }
}

/// The room feed confirming a death.
#[must_use]
pub fn dead(who: &Actor) -> Fact {
    Fact::Dead {
        creature: who.clone(),
    }
}

/// Every value of a one-column query.
///
/// # Errors
///
/// Any `SQLite` failure.
pub fn column<T: FromSql>(rec: &CombatRecorder, sql: &str) -> rusqlite::Result<Vec<T>> {
    let mut stmt = rec.connection().prepare(sql)?;
    let rows = stmt.query_map([], |r| r.get(0))?;
    rows.collect()
}

/// The single value of a one-row, one-column query.
///
/// # Errors
///
/// Any `SQLite` failure, including no row.
pub fn one<T: FromSql>(rec: &CombatRecorder, sql: &str) -> rusqlite::Result<T> {
    rec.connection().query_row(sql, [], |r| r.get(0))
}

/// `SELECT count(*)` over a table.
///
/// # Errors
///
/// Any `SQLite` failure.
pub fn count(rec: &CombatRecorder, table: &str) -> rusqlite::Result<i64> {
    one(rec, &format!("SELECT count(*) FROM {table}"))
}
