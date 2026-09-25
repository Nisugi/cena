//! The ledger's reports, read back from rows the ledger wrote.
//!
//! One short day: a search that yields silver, a gem and a coffer; the gem
//! sold at the gem shop; a deposit; a bounty. Then each report over it.

use cena_model::GameState;
use cena_protocol::Parser;
use cena_session::ledger::Ledger;
use cena_session::ledger::report::{Period, Reader};

fn prompt(at: u32) -> String {
    format!("<prompt time=\"{at}\">&gt;</prompt>\n")
}

/// Feed wire through the pipeline and record every loot chunk.
fn record(ledger: &mut Ledger, state: &mut GameState, wire: &str) -> rusqlite::Result<()> {
    let mut parser = Parser::new();
    for frame in parser.push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
    for chunk in state.take_loot() {
        ledger.record(&chunk, f64::from(chunk.at.unwrap_or(0)))?;
    }
    Ok(())
}

fn temp_db(name: &str) -> std::io::Result<std::path::PathBuf> {
    let dir = std::env::temp_dir().join(format!("cena-report-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("ledger.db"))
}

/// The day, written to a file of its own per test (they run in parallel);
/// the reader over it.
fn a_day(name: &str) -> rusqlite::Result<Reader> {
    let path = temp_db(name).map_err(|e| rusqlite::Error::InvalidPath(e.to_string().into()))?;
    let mut ledger = Ledger::open(&path, Some("Tester"))?;
    let mut state = GameState::default();
    let wire = "<nav rm='8801'/>\n".to_owned()
        + "You search the <pushBold/><a exist=\"407446374\" noun=\"shield-maiden\">gigas shield-maiden</a><popBold/>.\n"
        + "<pushBold/><a exist=\"407446374\" noun=\"shield-maiden\">She</a><popBold/> had 596 silvers on <pushBold/><a exist=\"407446374\" noun=\"shield-maiden\">her</a><popBold/>.\n"
        + "<pushBold/><a exist=\"407446374\" noun=\"shield-maiden\">She</a><popBold/> had a <a exist=\"5002\" noun=\"emerald\">uncut emerald</a> on her.\n"
        + "<pushBold/><a exist=\"407446374\" noun=\"shield-maiden\">She</a><popBold/> carried an <a exist=\"5001\" noun=\"coffer\">enruned steel coffer</a> on her.\n"
        + &prompt(1_000)
        + "The <pushBold/><a exist=\"-480255\" noun=\"Krosane\">jeweler Krosane</a><popBold/> takes the <a exist=\"5002\" noun=\"emerald\">uncut emerald</a>, gives it a careful examination and hands you 1,250 silver for it.\n"
        + &prompt(2_000)
        + "You deposit 1,500 silvers into your account.  The teller carefully records the transaction.\n"
        + &prompt(2_100)
        + "[You have earned 925 bounty points, 800 experience points, and 9250 silver.]\n"
        + &prompt(2_200);
    record(&mut ledger, &mut state, &wire)?;
    ledger.close()?;
    Reader::open(&path)
}

const DAY: Period = Period {
    since: 0.0,
    until: 10_000.0,
};

#[test]
fn the_summary_adds_the_day_up() {
    let reader = a_day("the_summary_adds_the_day_up").expect("the day");
    let s = reader.summary(DAY).expect("summary");
    assert_eq!((s.searches, s.silvers_search, s.items), (1, 596, 2));
    assert_eq!(s.sales, [("gemshop".to_owned(), 1250)]);
    assert_eq!(
        (s.deposits, s.bounty_silver, s.bounty_points),
        (1500, 9250, 925)
    );
    // A period before any of it is empty, not an error.
    let none = reader
        .summary(Period {
            since: 0.0,
            until: 1.0,
        })
        .expect("empty");
    assert_eq!(none.searches, 0);
    assert!(none.sales.is_empty());
}

#[test]
fn recent_filters_by_the_game_object_table() {
    let reader = a_day("recent_filters_by_the_game_object_table").expect("the day");
    let all = reader.recent(10, None).expect("recent");
    assert_eq!(all.len(), 2);
    let gems = reader.recent(10, Some("gem")).expect("gems");
    assert_eq!(gems.len(), 1);
    assert_eq!(gems[0].name, "uncut emerald");
    assert_eq!(
        (gems[0].sold, gems[0].sold_to.as_deref()),
        (Some(1250), Some("gemshop"))
    );
    assert!(reader.recent(10, Some("skin")).expect("skins").is_empty());
}

#[test]
fn boxes_know_their_corpse_and_their_fate() {
    let reader = a_day("boxes_know_their_corpse_and_their_fate").expect("the day");
    let boxes = reader.boxes(10).expect("boxes");
    assert_eq!(boxes.len(), 1, "the emerald is not a box");
    let b = &boxes[0];
    assert_eq!(b.name, "enruned steel coffer");
    assert_eq!(b.source.as_deref(), Some("gigas shield-maiden"));
    assert_eq!(
        (b.silvers, b.pool_dropped_at, b.returned_at),
        (None, None, None)
    );
}

#[test]
fn creatures_are_ranked_by_what_they_yielded() {
    let reader = a_day("creatures_are_ranked_by_what_they_yielded").expect("the day");
    let rows = reader.creatures(DAY, 10).expect("creatures");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (
            rows[0].name.as_str(),
            rows[0].searches,
            rows[0].silvers,
            rows[0].items
        ),
        ("gigas shield-maiden", 1, 596, 2)
    );
}

#[test]
fn the_cap_counts_boxes_apart_and_values_items_by_type() {
    let reader = a_day("the_cap_counts_boxes_apart_and_values_items_by_type").expect("the day");
    let cap = reader.cap(DAY).expect("cap");
    assert_eq!(
        (
            cap.searches,
            cap.boxes,
            cap.silvers_loose,
            cap.bounty_silver
        ),
        (1, 1, 596, 9250)
    );
    assert_eq!(
        cap.items,
        [("gem".to_owned(), 1, 1250, 1250)],
        "the coffer is counted as a box, not an item"
    );
    assert_eq!(cap.too_valuable, 0);
}
