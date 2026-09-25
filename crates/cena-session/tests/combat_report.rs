//! The combat recorder's reports, read back from rows the recorder wrote.
//!
//! Two hunts through the real recorder, on disk, then the reader over the
//! file: the list, one hunt, the aggregate, the abilities and the recent
//! attacks. The rows are the kit's hand-built events, the same shapes
//! `combat_recorder.rs` proves the recorder writes.

mod recorder_kit;

use cena_model::Actor;
use cena_session::combat_recorder::CombatRecorder;
use cena_session::combat_recorder::report::Scope;
use cena_session::ledger::report::Reader;
use recorder_kit::{attack, chunk, dead, facts, lizard};

fn temp_db(name: &str) -> std::io::Result<std::path::PathBuf> {
    let dir = std::env::temp_dir().join(format!("cena-cstats-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("combat.db"))
}

fn rat() -> Actor {
    Actor {
        id: Some(202),
        noun: Some("rat".to_owned()),
        name: "a giant rat".to_owned(),
    }
}

/// Two hunts: the first kills a lizard in two swings; the second swings at
/// a rat once and leaves it alive.
fn two_hunts(name: &str) -> rusqlite::Result<Reader> {
    let path = temp_db(name).map_err(|e| rusqlite::Error::InvalidPath(e.to_string().into()))?;
    let mut rec = CombatRecorder::open(&path, Some("Tester"), "spec", None)?;
    rec.start_session(1_000.0)?;
    rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 1_001.0)?;
    rec.record_chunk(&chunk(vec![attack(&lizard(), 60)]), 1_005.0)?;
    rec.record_chunk(&facts(vec![dead(&lizard())]), 1_006.0)?;
    rec.finish_session(1_060.0)?;
    rec.start_session(2_000.0)?;
    rec.record_chunk(&chunk(vec![attack(&rat(), 25)]), 2_010.0)?;
    rec.finish_session(2_100.0)?;
    rec.close(2_100.0)?;
    Reader::open(&path)
}

#[test]
fn the_hunts_list_newest_first() {
    let reader = two_hunts("list").expect("two hunts");
    let rows = reader.hunts(10).expect("hunts");
    assert_eq!(rows.len(), 2);
    assert_eq!(
        (rows[0].id, rows[0].attacks, rows[0].kills, rows[0].damage),
        (2, 1, 0, 25)
    );
    assert_eq!(
        (rows[1].id, rows[1].attacks, rows[1].kills, rows[1].damage),
        (1, 2, 1, 100)
    );
    assert_eq!(rows[1].ended_at, Some(1_060.0));
}

#[test]
fn one_hunt_counts_its_kill_and_groups_its_creatures() {
    let reader = two_hunts("one").expect("two hunts");
    let r = reader.hunt(&Scope::Hunt(1)).expect("q").expect("hunt 1");
    assert_eq!(r.character.as_deref(), Some("Tester"));
    assert_eq!((r.attacks, r.sequences, r.inbound), (2, 2, 0));
    assert_eq!((r.kills, r.assists, r.dealt, r.taken), (1, 0, 100, 0));
    assert!((r.hunt_time - 60.0).abs() < f64::EPSILON);
    assert_eq!(r.creatures.len(), 1);
    let k = &r.creatures[0];
    assert_eq!(
        (
            k.noun.as_str(),
            k.seen,
            k.kills,
            k.other_dead,
            k.alive,
            k.attacks,
            k.damage
        ),
        ("lizard", 1, 1, 0, 0, 2, 100)
    );
    // The latest is hunt 2, with the rat alive.
    let latest = reader.hunt(&Scope::Latest).expect("q").expect("latest");
    assert_eq!(latest.hunts[0].id, 2);
    assert_eq!((latest.kills, latest.creatures[0].alive), (0, 1));
    assert!(reader.hunt(&Scope::Hunt(9)).expect("q").is_none());
}

#[test]
fn the_aggregate_sums_hunts_and_pools_creatures() {
    let reader = two_hunts("all").expect("two hunts");
    let r = reader.hunt(&Scope::All).expect("q").expect("all");
    assert_eq!(r.hunts.len(), 2);
    assert_eq!((r.attacks, r.kills, r.dealt), (3, 1, 125));
    assert!((r.hunt_time - 160.0).abs() < f64::EPSILON);
    let nouns: Vec<&str> = r.creatures.iter().map(|k| k.noun.as_str()).collect();
    assert_eq!(nouns, ["lizard", "rat"], "most kills first");
    let last = reader.hunt(&Scope::Last(1)).expect("q").expect("last");
    assert_eq!(last.hunts.len(), 1);
    assert_eq!(last.hunts[0].id, 2);
}

#[test]
fn abilities_count_every_swing_and_its_hits() {
    let reader = two_hunts("abilities").expect("two hunts");
    let a = reader.abilities(&Scope::All).expect("q").expect("all");
    assert_eq!(a.abilities.len(), 1);
    let row = &a.abilities[0];
    assert_eq!(
        (
            row.name.as_str(),
            row.attempts,
            row.landed,
            row.hits,
            row.damage,
            row.fatal
        ),
        ("attack", 3, 3, 3, 125, 0)
    );
    assert!(a.flares.is_empty());
    assert!(a.inbound.is_empty());
}

#[test]
fn recent_attacks_are_the_latest_hunts_newest_first() {
    let reader = two_hunts("recent").expect("two hunts");
    let rows = reader.recent_attacks(15).expect("q");
    assert_eq!(rows.len(), 1, "only the latest hunt's");
    assert_eq!(
        (rows[0].damage, rows[0].target.as_deref()),
        (25, Some("a giant rat"))
    );
    assert!(!rows[0].echo);
}
