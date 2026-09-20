//! The 71 replay fixtures, through the state machine.
//!
//! Lich's `replay_spec.rb` drives each blob through `parse_events` and
//! asserts the events reproduce the `# expect:` header under the floor
//! contract: *"numeric: exact. set fields: every expected fact must be
//! present (extras tolerated -- the header is a floor)."* This is that test
//! against [`CombatTracker`], with the same `summarize` reduction:
//!
//! - `attacks`: every event's name, plus `cast` for a `via_cast` event and
//!   `ambush` for an ambush-flagged one
//! - `res`, `flares`, `outcomes`: across events AND their flares
//! - `dmg`: the count of hits, events and flares together -- **exact**
//! - `statuses`: `status/action` for every status fact, creature or self
//!
//! `combat_replay_lines.rs` is the floor beneath this one: it proves each
//! fact is classified from SOME line. This proves the machine attributes it.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use cena_model::GameState;
use cena_model::crit::CritTables;
use cena_model::state::combat::event::{ChunkFacts, Fact};
use cena_model::state::combat::tracker::CombatTracker;
use cena_protocol::Parser;

struct Blob {
    file: String,
    header: String,
    expect: Vec<(String, String)>,
    dmg: usize,
    lines: Vec<String>,
}

fn parse_expect(line: &str) -> (Vec<(String, String)>, usize) {
    let mut pairs = Vec::new();
    let mut dmg = 0;
    for part in line.trim_start_matches("# expect: ").split(" | ") {
        let Some((k, v)) = part.split_once('=') else {
            continue;
        };
        if k == "dmg" {
            dmg = v.trim().parse().unwrap_or(0);
            continue;
        }
        for x in v.split(',').filter(|x| !x.is_empty() && *x != "none") {
            pairs.push((k.to_owned(), x.to_owned()));
        }
    }
    (pairs, dmg)
}

fn load_blobs() -> Vec<Blob> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/combat");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    paths.sort();
    let mut blobs = Vec::new();
    for path in paths {
        let file = path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default();
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let mut header: Option<String> = None;
        let mut expect: Option<(Vec<(String, String)>, usize)> = None;
        let mut lines: Vec<String> = Vec::new();
        let mut flush = |header: &mut Option<String>,
                         expect: &mut Option<(Vec<(String, String)>, usize)>,
                         lines: &mut Vec<String>| {
            if let (Some(h), Some((e, dmg))) = (header.take(), expect.take())
                && !lines.is_empty()
            {
                blobs.push(Blob {
                    file: file.clone(),
                    header: h,
                    expect: e,
                    dmg,
                    lines: std::mem::take(lines),
                });
            }
            lines.clear();
        };
        for raw in text.lines() {
            if let Some(h) = raw.strip_prefix("##### ") {
                flush(&mut header, &mut expect, &mut lines);
                header = Some(h.to_owned());
            } else if raw.starts_with("# expect:") {
                expect = Some(parse_expect(raw));
            } else if raw.trim().is_empty() {
                flush(&mut header, &mut expect, &mut lines);
            } else if header.is_some() {
                lines.push(raw.to_owned());
            }
        }
        flush(&mut header, &mut expect, &mut lines);
    }
    blobs
}

/// A blob through the real parser and a fresh tracker with crit tables.
fn run(lines: &[String], tables: &Arc<CritTables>) -> ChunkFacts {
    let mut parser = Parser::new();
    let mut state = GameState::default();
    let wire = lines.join("\n") + "\n";
    for frame in parser.push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
    let mut tracker = CombatTracker::default();
    tracker.set_crit_tables(Arc::clone(tables));
    tracker.consume_chunk(state.open_chunk(), None);
    tracker.take_facts().into_iter().next().unwrap_or_default()
}

/// `replay_spec.rb`'s `summarize`, as `(key, value)` pairs plus the hit count.
fn summarize(facts: &ChunkFacts) -> (BTreeSet<(String, String)>, usize) {
    let mut got = BTreeSet::new();
    let mut dmg = 0;
    for e in &facts.events {
        got.insert(("attacks".to_owned(), e.name.clone()));
        if e.via_cast {
            got.insert(("attacks".to_owned(), "cast".to_owned()));
        }
        if e.ambush {
            got.insert(("attacks".to_owned(), "ambush".to_owned()));
        }
        for r in &e.resolutions {
            got.insert(("res".to_owned(), r.kind.as_str().to_owned()));
        }
        for o in &e.outcomes {
            got.insert(("outcomes".to_owned(), o.as_str().to_owned()));
        }
        dmg += e.hits.len();
        for f in &e.flares {
            got.insert(("flares".to_owned(), f.name.clone()));
            for r in &f.resolutions {
                got.insert(("res".to_owned(), r.kind.as_str().to_owned()));
            }
            for o in &f.outcomes {
                got.insert(("outcomes".to_owned(), o.as_str().to_owned()));
            }
            dmg += f.hits.len();
        }
    }
    for f in &facts.facts {
        if let Fact::Status { status, action, .. } = f {
            let action = match action {
                cena_model::StatusAction::Add => "add",
                cena_model::StatusAction::Remove => "remove",
            };
            got.insert((
                "statuses".to_owned(),
                format!("{}/{action}", status.as_str()),
            ));
        }
    }
    (got, dmg)
}

/// **Every blob's header is reproduced by the state machine.**
#[test]
fn every_replay_blob_reproduces_its_header() {
    let tables = Arc::new(CritTables::load().expect("crit tables load"));
    let blobs = load_blobs();
    assert!(blobs.len() >= 50, "replay_spec.rb asserts >= 50 blobs");
    let mut report = Vec::new();
    let started = std::time::Instant::now();
    let mut total_lines = 0usize;
    for blob in &blobs {
        total_lines += blob.lines.len();
        let facts = run(&blob.lines, &tables);
        let (got, dmg) = summarize(&facts);
        let mut problems = Vec::new();
        if dmg != blob.dmg {
            problems.push(format!("dmg: want {} got {dmg}", blob.dmg));
        }
        let missing: Vec<String> = blob
            .expect
            .iter()
            .filter(|pair| !got.contains(pair))
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        if !missing.is_empty() {
            problems.push(format!("missing {}", missing.join(", ")));
        }
        if !problems.is_empty() {
            let names: Vec<String> = facts
                .events
                .iter()
                .map(|e| {
                    format!(
                        "{}[{}h {}f {}r {}o]",
                        e.name,
                        e.hits.len(),
                        e.flares.len(),
                        e.resolutions.len(),
                        e.outcomes.len()
                    )
                })
                .collect();
            report.push(format!(
                "  {} / {}: {} (events: {})",
                blob.file,
                blob.header,
                problems.join("; "),
                names.join(" ")
            ));
        }
    }
    // MEASURED, printed under `--nocapture`: the parser, the chunk and the
    // state machine together, per real feed line. `plan/06` §1.9.
    let elapsed = started.elapsed();
    println!(
        "{} blobs, {total_lines} lines through parser + FSM in {elapsed:?}: {:.0}µs/line",
        blobs.len(),
        elapsed.as_secs_f64() * 1e6 / total_lines as f64
    );
    assert!(
        report.is_empty(),
        "{} of {} blobs fail:\n{}",
        report.len(),
        blobs.len(),
        report.join("\n")
    );
}
