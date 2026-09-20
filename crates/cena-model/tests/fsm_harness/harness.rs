//! Shared harness for the state-machine spec ports.
//!
//! A chunk is fed as wire text through the real parser into a `GameState`,
//! closed by a prompt exactly as the game closes it, and the tracker's facts
//! are drained. Lich's specs pass an array of raw lines to `parse_events`;
//! the difference here is that the lines go through `Parser` first, which is
//! the whole point of the port -- the classifiers read runs, not tags.

use std::path::PathBuf;
use std::sync::Arc;

use cena_model::crit::CritTables;
use cena_model::state::combat::{AttackEvent, ChunkFacts};
use cena_model::{GameState, OutcomeKind};
use cena_protocol::Parser;

/// A bolded creature link, as Lich's specs build one.
pub fn bolded(id: i64, noun: &str, name: &str) -> String {
    format!(r#"<pushBold/><a exist="{id}" noun="{noun}">{name}</a><popBold/>"#)
}

/// Feed one chunk and close it with a prompt; return what it yielded.
pub fn run(state: &mut GameState, lines: &[&str]) -> ChunkFacts {
    // A load failure shows up as `crit: None` on every hit, which the crit
    // assertions catch; the harness itself does not panic.
    if state.combat().crit_tables().is_none()
        && let Ok(tables) = CritTables::load()
    {
        state.combat_mut().set_crit_tables(Arc::new(tables));
    }
    let mut parser = Parser::new();
    let mut wire = String::new();
    for l in lines.iter().filter(|l| !l.starts_with("<prompt")) {
        wire.push_str(l);
        wire.push('\n');
    }
    wire.push_str("<prompt time=\"1\">&gt;</prompt>\n");
    for frame in parser.push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
    state.combat_mut().take_facts().pop().unwrap_or_default()
}

/// One chunk through a fresh state.
pub fn parse(lines: &[&str]) -> ChunkFacts {
    run(&mut GameState::default(), lines)
}

/// A raw spec fixture's lines (`tests/fixtures/combat_raw/<name>.txt`).
pub fn raw_fixture(name: &str) -> Vec<&'static str> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/combat_raw")
        .join(format!("{name}.txt"));
    // A missing fixture reads as no lines, and the test's own assertions
    // report it; the harness does not panic.
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    Box::leak(text.into_boxed_str()).lines().collect()
}

pub fn names(f: &ChunkFacts) -> Vec<&str> {
    f.events.iter().map(|e| e.name.as_str()).collect()
}

pub fn tid(e: &AttackEvent) -> Option<i64> {
    e.target.id()
}

pub fn dmg(e: &AttackEvent) -> Vec<u32> {
    e.hits.iter().map(|h| h.damage).collect()
}

pub fn results(e: &AttackEvent) -> Vec<Option<i64>> {
    e.resolutions.iter().map(|r| r.result).collect()
}

pub fn outs(e: &AttackEvent) -> Vec<OutcomeKind> {
    e.outcomes.clone()
}

pub fn flares(e: &AttackEvent) -> Vec<&str> {
    e.flares.iter().map(|x| x.name.as_str()).collect()
}
