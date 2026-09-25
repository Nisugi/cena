//! Loot facts leave a running session for the ledger.
//!
//! The bytes go through a real `SessionActor` over a `ReplaySource`, so what
//! is tested is the wiring -- `close_chunk` queuing, `actor/combat.rs`'s
//! `publish_loot`, `ledger/worker.rs` -- and not the parts, which have their
//! own files (`ledger.rs`, and `cena-model/tests/loot_facts.rs`).

use cena_platform::ReplaySource;
use cena_session::Session;
use cena_session::ledger::{Ledger, worker};

fn temp_db(name: &str) -> std::io::Result<std::path::PathBuf> {
    let dir = std::env::temp_dir().join(format!("cena-ledger-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("gs3_tester_combat.db"))
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_session_records_the_searches_in_a_real_hunt() {
    let path = temp_db("hunt").expect("temp dir");
    let ledger = Ledger::open(&path, Some("Tester")).expect("opens");
    let (handle, flush) = worker::spawn(ledger).expect("spawns");
    let wire = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../cena-behavior/tests/fixtures/arch_kill.xml"
    ))
    .expect("the hunt fixture");
    let stats = handle.clone();
    let end = Session::new(ReplaySource::from_bytes(&wire))
        .with_ledger(handle)
        .into_actor()
        .run()
        .await;
    drop(end);
    drop(stats);
    flush.join().expect("the worker flushed");

    let conn = rusqlite::Connection::open(&path).expect("reopens");
    let rows: Vec<(String, i64, f64)> = {
        let mut stmt = conn
            .prepare("SELECT source_noun, silvers, at FROM loot_events ORDER BY id")
            .expect("prepare");
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .expect("query")
            .collect::<rusqlite::Result<_>>()
            .expect("rows")
    };
    assert_eq!(
        rows,
        [
            ("mastodon".to_owned(), 0, 1_790_045_830.0),
            ("shield-maiden".to_owned(), 596, 1_790_045_835.0)
        ],
        "the two searches, stamped with the prompts that closed them"
    );
    // The fixture's one `<nav rm=>` (line 342) comes AFTER both searches: the
    // cut begins mid-room. A room the wire has not named is NULL, not a guess.
    let unknown: i64 = conn
        .query_row(
            "SELECT count(*) FROM loot_events WHERE room_id IS NULL",
            [],
            |r| r.get(0),
        )
        .expect("q");
    assert_eq!(unknown, 2);
}
