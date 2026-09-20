//! Combat facts leave a running session: to observers, and to the recorder.
//!
//! The bytes go through a real `SessionActor` over a `ReplaySource`, so what
//! is tested is the wiring -- `actor/combat.rs` and
//! `combat_recorder/worker.rs` -- and not the parts, which have their own
//! files.

mod recorder_kit;

use std::sync::Arc;

use cena_model::state::combat::ChunkFacts;
use cena_platform::ReplaySource;
use cena_session::combat_recorder::{CombatRecorder, worker};
use cena_session::{CritTables, Event, Frame, Session};
use recorder_kit::{bolded, count, one};

/// One swing that crits, closed by a prompt at `time`.
fn swing(time: u32) -> String {
    let liz = bolded(101, "lizard", "a cave lizard");
    format!(
        "You swing a broadsword at {liz}!\n\
         \x20 AS: +300 vs DS: +100 with AvD: +30 + d100 roll: +50 = +280\n\
         \x20  ... and hit for 40 points of damage!\n\
         \x20  Hit on the leg chars the skin and eats into the underlying muscles.\n\
         <prompt time=\"{time}\">&gt;</prompt>\n"
    )
}

fn quiet_prompt(time: u32) -> String {
    format!("<prompt time=\"{time}\">&gt;</prompt>\n")
}

fn temp_db(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("cena-wiring-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_closed_chunk_is_one_combat_event_after_its_prompt() {
    let tables = Arc::new(CritTables::load().expect("crit tables"));
    let session =
        Session::new(ReplaySource::from_bytes(swing(1_000).as_bytes())).with_crit_tables(tables);
    let (_snapshot, mut events) = session.subscribe();
    let end = session.into_actor().run().await;

    let mut seen = Vec::new();
    while let Ok(event) = events.try_recv() {
        match event {
            Event::Frame(f) if matches!(*f, Frame::Prompt { .. }) => seen.push("prompt"),
            Event::Combat(_) => seen.push("combat"),
            _ => {}
        }
    }
    assert_eq!(seen, ["prompt", "combat"], "one event, after the prompt");
    // the model applied the chunk before anyone was told
    assert_eq!(
        end.state
            .creatures()
            .get(101)
            .map(cena_model::CreatureInstance::damage_taken),
        Some(40)
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_combat_event_carries_the_chunk_whole_with_its_crit_and_its_time() {
    let tables = Arc::new(CritTables::load().expect("crit tables"));
    let session =
        Session::new(ReplaySource::from_bytes(swing(1_000).as_bytes())).with_crit_tables(tables);
    let (_snapshot, mut events) = session.subscribe();
    let _ = session.into_actor().run().await;

    let mut chunks: Vec<Arc<ChunkFacts>> = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let Event::Combat(facts) = event {
            chunks.push(facts);
        }
    }
    assert_eq!(chunks.len(), 1);
    let facts = &chunks[0];
    assert_eq!(facts.at, Some(1_000));
    assert_eq!(facts.events.len(), 1);
    assert_eq!(facts.events[0].total_damage(), 40);
    assert!(
        facts.events[0].hits[0].crit.is_some(),
        "with_crit_tables reached the tracker"
    );
    assert!(!facts.facts.is_empty(), "the crit's own facts ride along");
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn without_crit_tables_a_hit_has_no_crit() {
    let session = Session::new(ReplaySource::from_bytes(swing(1_000).as_bytes()));
    let (_snapshot, mut events) = session.subscribe();
    let _ = session.into_actor().run().await;
    let mut crits = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let Event::Combat(facts) = event {
            crits.push(facts.events[0].hits[0].crit.is_some());
        }
    }
    assert_eq!(crits, [false], "guard: the builder is what supplies them");
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_quiet_prompt_publishes_nothing() {
    let session = Session::new(ReplaySource::from_bytes(quiet_prompt(5).as_bytes()));
    let (_snapshot, mut events) = session.subscribe();
    let _ = session.into_actor().run().await;
    let mut combat = 0;
    while let Ok(event) = events.try_recv() {
        combat += usize::from(matches!(event, Event::Combat(_)));
    }
    assert_eq!(combat, 0);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_session_records_its_combat_and_a_quiet_prompt_closes_the_idle_hunt() {
    let dir = temp_db("records");
    let (handle, flush, path) = worker::open_live(&dir, "GS3", "Tester").expect("opens");
    assert!(path.ends_with("GS3_Tester_combat.db"), "{}", path.display());

    // a swing, a quiet prompt inside the gap, and one 301s after the swing
    let wire = swing(1_000) + &quiet_prompt(1_200) + &quiet_prompt(1_301);
    let tables = Arc::new(CritTables::load().expect("crit tables"));
    // a second handle, as a supervisor holds across connections
    let stats = handle.clone();
    let end = Session::new(ReplaySource::from_bytes(wire.as_bytes()))
        .with_crit_tables(tables)
        .with_combat_recorder(handle)
        .into_actor()
        .run()
        .await;
    drop(end);

    // The worker is STILL RUNNING -- `stats` holds its queue open -- so a
    // closed hunt here was closed by the quiet prompt's tick. Shutdown closes
    // a hunt at its last event too, and would hide a tick that never came.
    let reader = rusqlite::Connection::open(&path).expect("a second reader");
    let mut closed_by_tick = None;
    for _ in 0..500 {
        closed_by_tick = reader
            .query_row("SELECT CAST(ended_at AS INTEGER) FROM sessions", [], |r| {
                r.get::<_, Option<i64>>(0)
            })
            .ok()
            .flatten();
        if closed_by_tick.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(
        closed_by_tick,
        Some(1_000),
        "closed at the LAST EVENT, by the tick"
    );
    assert_eq!(stats.stats().recorded(), 1);
    assert_eq!(stats.stats().failed(), 0);
    drop(reader);

    // every handle gone: the worker drains, closes and exits
    drop(stats);
    flush.join().expect("the worker exits cleanly");

    let rec = CombatRecorder::open(&path, None, "check", None).expect("reopens");
    assert_eq!(count(&rec, "attacks").expect("q"), 1);
    assert_eq!(count(&rec, "hits").expect("q"), 1);
    assert_eq!(
        one::<String>(&rec, "SELECT character FROM sessions").expect("q"),
        "Tester"
    );
    // stamped with the prompt's server time
    assert_eq!(
        one::<i64>(&rec, "SELECT CAST(occurred_at AS INTEGER) FROM attacks").expect("q"),
        1_000
    );
    assert_eq!(
        one::<i64>(&rec, "SELECT CAST(ended_at AS INTEGER) FROM sessions").expect("q"),
        1_000
    );
    drop(rec);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_chunk_with_no_server_time_is_refused_and_counted() {
    let rec = CombatRecorder::in_memory(None, "spec", Some(300.0)).expect("opens");
    let (handle, flush) = worker::spawn(rec).expect("spawns");
    let untimed = ChunkFacts {
        events: vec![recorder_kit::attack(&recorder_kit::lizard(), 1)],
        ..ChunkFacts::default()
    };
    assert!(!handle.offer(Arc::new(untimed)));
    assert_eq!(handle.stats().untimed(), 1);
    assert_eq!(handle.stats().dropped(), 0);
    drop(handle);
    flush.join().expect("the worker exits cleanly");
}

#[test]
fn a_database_named_after_nobody_is_refused() {
    let dir = temp_db("nobody");
    assert!(worker::open_live(&dir, "GS3", "!!!").is_err());
    assert!(worker::open_live(&dir, "", "Tester").is_err());
    assert!(!dir.exists(), "and nothing was created for it");
}

/// Serves prepared connections in order, then refuses: `reconnect.rs`'s
/// connector, without its call counter.
struct TwoConnections(std::collections::VecDeque<Vec<u8>>);

impl cena_session::Connector for TwoConnections {
    type Source = ReplaySource;

    async fn connect(
        &mut self,
        _generation: cena_session::Generation,
    ) -> Result<ReplaySource, cena_session::ConnectError> {
        self.0
            .pop_front()
            .map(|bytes| ReplaySource::from_bytes(&bytes))
            .ok_or_else(|| cena_session::ConnectError::fatal("scripted", "no more connections"))
    }
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn one_hunt_spans_a_reconnect_with_its_tables_and_its_recorder() {
    let dir = temp_db("reconnect");
    let (handle, flush, path) = worker::open_live(&dir, "GS3", "Tester").expect("opens");
    let connector = TwoConnections(
        [swing(1_000).into_bytes(), swing(1_100).into_bytes()]
            .into_iter()
            .collect(),
    );
    let (session, _commands) = cena_session::SupervisedSession::new(connector);
    let session = session
        .with_crit_tables(Arc::new(CritTables::load().expect("crit tables")))
        .with_combat_recorder(handle);
    let (_snapshot, mut events) = session.subscribe();
    let end = session.run().await;
    assert!(end.generations.0 >= 1, "guard: it did reconnect");
    drop(end);

    let mut crits = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let Event::Combat(facts) = event {
            crits.push(facts.events[0].hits[0].crit.is_some());
        }
    }
    assert_eq!(crits, [true, true], "the tables rode the carried state");

    flush.join().expect("the worker exits cleanly");
    let rec = CombatRecorder::open(&path, None, "check", None).expect("reopens");
    assert_eq!(count(&rec, "attacks").expect("q"), 2);
    assert_eq!(
        count(&rec, "sessions").expect("q"),
        1,
        "one hunt, two connections"
    );
    assert_eq!(count(&rec, "creatures").expect("q"), 1);
    drop(rec);
    let _ = std::fs::remove_dir_all(&dir);
}
