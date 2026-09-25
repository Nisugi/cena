//! The loot ledger: what one chunk's facts become as rows, and the linking
//! across chunks that loottracker's processors did.
//!
//! Every chunk is real wire through the real pipeline -- parser, chunk,
//! classifier, `GameState::take_loot` -- closed by a prompt whose time is the
//! row's `at`.

use cena_model::GameState;
use cena_model::state::ledger::LootChunk;
use cena_protocol::Parser;
use cena_session::ledger::Ledger;
use rusqlite::types::FromSql;

/// Feed wire (with its own prompts) and take every loot chunk it produced.
fn chunks(state: &mut GameState, wire: &str) -> Vec<LootChunk> {
    let mut parser = Parser::new();
    for frame in parser.push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
    state.take_loot()
}

/// Feed and record, in one go.
fn record(ledger: &mut Ledger, state: &mut GameState, wire: &str) -> rusqlite::Result<usize> {
    let taken = chunks(state, wire);
    for chunk in &taken {
        ledger.record(chunk, f64::from(chunk.at.unwrap_or(0)))?;
    }
    Ok(taken.len())
}

fn one<T: FromSql>(ledger: &Ledger, sql: &str) -> rusqlite::Result<T> {
    ledger.connection().query_row(sql, [], |r| r.get(0))
}

fn count(ledger: &Ledger, table: &str) -> rusqlite::Result<i64> {
    one(ledger, &format!("SELECT count(*) FROM {table}"))
}

fn prompt(at: u32) -> String {
    format!("<prompt time=\"{at}\">&gt;</prompt>\n")
}

fn room(id: &str) -> String {
    format!("<nav rm='{id}'/>\n")
}

const SEARCH: &str = concat!(
    "You search the <pushBold/><a exist=\"407446374\" noun=\"shield-maiden\">gigas shield-maiden</a><popBold/>.\n",
    "<pushBold/><a exist=\"407446374\" noun=\"shield-maiden\">She</a><popBold/> had 596 silvers on <pushBold/><a exist=\"407446374\" noun=\"shield-maiden\">her</a><popBold/>.\n",
    "<pushBold/><a exist=\"407446374\" noun=\"shield-maiden\">She</a><popBold/> carried a <a exist=\"5001\" noun=\"coffer\">enruned steel coffer</a> on her.\n",
);

#[test]
fn a_search_is_an_event_with_its_items_in_the_wires_room() {
    let mut ledger = Ledger::in_memory(Some("Tester")).expect("opens");
    let mut state = GameState::default();
    let wire = room("8801") + SEARCH + &prompt(1_000);
    assert_eq!(record(&mut ledger, &mut state, &wire).expect("records"), 1);

    let (kind, silvers, room_id, at): (String, i64, String, f64) = ledger
        .connection()
        .query_row(
            "SELECT kind, silvers, room_id, at FROM loot_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("one event");
    assert_eq!(
        (kind.as_str(), silvers, room_id.as_str()),
        ("search", 596, "8801")
    );
    assert!((at - 1_000.0).abs() < f64::EPSILON);
    let (exist, noun, kind, event): (i64, String, String, i64) = ledger
        .connection()
        .query_row(
            "SELECT exist_id, noun, kind, event_id FROM loot_items",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("one item");
    assert_eq!(
        (exist, noun.as_str(), kind.as_str(), event),
        (5001, "coffer", "item", 1)
    );
}

#[test]
fn an_offer_one_prompt_and_the_sale_the_next_are_one_item_sold() {
    let mut ledger = Ledger::in_memory(Some("Tester")).expect("opens");
    let mut state = GameState::default();
    let looted = room("8801") + SEARCH + &prompt(1_000);
    record(&mut ledger, &mut state, &looted).expect("records");
    let offer = "You offer to sell your <a exist=\"5001\" noun=\"coffer\">enruned steel coffer</a> to Bushybrow.\n".to_owned()
        + &room("2210")
        + &prompt(2_000);
    record(&mut ledger, &mut state, &offer).expect("records");
    let chit = "He scribbles out a <a exist=\"7\" noun=\"chit\">salt-stained kraken chit</a> for 25,000 silvers and hands it to you.\n".to_owned()
        + &prompt(2_003);
    record(&mut ledger, &mut state, &chit).expect("records");

    assert_eq!(
        count(&ledger, "loot_items").expect("q"),
        1,
        "no `seen` row: the offer named it"
    );
    let (value, to, note, room_id): (i64, String, i64, String) = ledger
        .connection()
        .query_row(
            "SELECT sold_value, sold_to, sold_note_exist, sold_room_id FROM loot_items",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("sold");
    assert_eq!(
        (value, to.as_str(), note, room_id.as_str()),
        (25_000, "pawn", 7, "2210")
    );
    let (category, amount, item): (String, i64, i64) = ledger
        .connection()
        .query_row(
            "SELECT category, amount, item_id FROM transactions",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .expect("transaction");
    assert_eq!((category.as_str(), amount, item), ("sale", 25_000, 1));
}

#[test]
fn an_offer_nobody_answers_in_the_window_claims_nothing() {
    let mut ledger = Ledger::in_memory(Some("Tester")).expect("opens");
    let mut state = GameState::default();
    let offer = "You offer to sell your <a exist=\"5001\" noun=\"coffer\">enruned steel coffer</a> to Bushybrow.\n".to_owned()
        + &prompt(2_000);
    record(&mut ledger, &mut state, &offer).expect("records");
    // Ten minutes later a chit for something else.
    let chit = "He scribbles out a <a exist=\"7\" noun=\"chit\">salt-stained kraken chit</a> for 25,000 silvers and hands it to you.\n".to_owned()
        + &prompt(2_600);
    record(&mut ledger, &mut state, &chit).expect("records");
    assert_eq!(
        count(&ledger, "loot_items").expect("q"),
        0,
        "the coffer was not sold"
    );
    let item: Option<i64> = one(&ledger, "SELECT item_id FROM transactions").expect("q");
    assert_eq!(item, None, "the sale is recorded, against no item");
}

#[test]
fn a_loresong_pairs_across_chunks() {
    let mut ledger = Ledger::in_memory(Some("Tester")).expect("opens");
    let mut state = GameState::default();
    let sung = "As you sing, you feel a faint resonating vibration from the <a exist=\"554177549\" noun=\"ivory\">age-darkened ivory</a> in your hand, and you learn something about it...\n".to_owned() + &prompt(100);
    let valued = "This is a small item, under a pound.  In your best estimation, it's worth about 1,400 silvers, and is of outstanding quality.\n".to_owned() + &prompt(104);
    record(&mut ledger, &mut state, &sung).expect("records");
    assert_eq!(
        count(&ledger, "loot_items").expect("q"),
        0,
        "nothing until the figure"
    );
    record(&mut ledger, &mut state, &valued).expect("records");
    let (exist, value, by, kind): (i64, i64, String, String) = ledger
        .connection()
        .query_row(
            "SELECT exist_id, appraised_value, appraised_by, kind FROM loot_items",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("appraised");
    assert_eq!(
        (exist, value, by.as_str(), kind.as_str()),
        (554_177_549, 1400, "loresong", "seen"),
        "never looted here, so a `seen` row carries the appraisal"
    );
}

/// The Red Forest case (`plan/34` §3): the box comes back from the pool under
/// a new `exist` id, in a room the map gives two uids. The ledger keys the
/// room by the wire's one id and finds the dropped box by noun and room.
#[test]
fn a_box_returned_under_a_new_id_links_to_the_box_dropped_in_this_room() {
    let mut ledger = Ledger::in_memory(Some("Tester")).expect("opens");
    let mut state = GameState::default();
    let looted = room("8801") + SEARCH + &prompt(1_000);
    record(&mut ledger, &mut state, &looted).expect("records");
    let locksmith = room("31337");
    let quote = locksmith.clone()
        + "You want a locksmith to open an <a exist=\"5001\" noun=\"coffer\">enruned steel coffer</a> for a tip of 219 silvers, or 3 percent of the box value.  There is also a fee of 2,716 silvers due up front.\n"
        + &prompt(3_000);
    record(&mut ledger, &mut state, &quote).expect("records");
    let dropped = "<pushBold/>The <a exist=\"-100\" noun=\"Wehnimer\">locksmith Wehnimer</a><popBold/> takes your coffer and says, \"Your tip of 219 silvers has been recorded, and the 2,716 silver fee has been collected.  We'll get someone on that right away.\"\n".to_owned() + &prompt(3_010);
    record(&mut ledger, &mut state, &dropped).expect("records");
    let (tip, fee, pool_room): (i64, i64, String) = ledger
        .connection()
        .query_row(
            "SELECT pool_tip, pool_fee, pool_room_id FROM loot_items WHERE exist_id = 5001",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .expect("dropped");
    assert_eq!((tip, fee, pool_room.as_str()), (219, 2716, "31337"));
    assert_eq!(
        count(&ledger, "transactions").expect("q"),
        2,
        "the fee and the tip"
    );

    // Back, later, as a new object, then opened.
    let returned = "<pushBold/>The <a exist=\"-100\" noun=\"Wehnimer\">locksmith Wehnimer</a><popBold/> says, \"Alright, here's your <a exist=\"6002\" noun=\"coffer\">enruned steel coffer</a> back.\"\n".to_owned() + &prompt(3_400);
    record(&mut ledger, &mut state, &returned).expect("records");
    let opened = concat!(
        "<container id='6002' title='Coffer' target='#6002' location='right'/>",
        "<inv id='6002'>In the <a exist=\"6002\" noun=\"coffer\">coffer</a>:</inv>\n",
        "<inv id='6002'> a <a exist=\"6003\" noun=\"scroll\">shimmering scroll</a></inv>\n",
        "You gather the remaining 1,576 <a exist=\"6004\" noun=\"coins\">coins</a> from inside your <a exist=\"6002\" noun=\"coffer\">enruned steel coffer</a>.\n",
    )
    .to_owned()
        + &prompt(3_405);
    record(&mut ledger, &mut state, &opened).expect("records");

    let (exist, returned_at, opened_event): (i64, f64, i64) = ledger
        .connection()
        .query_row(
            "SELECT exist_id, returned_at, opened_event_id FROM loot_items WHERE noun = 'coffer'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .expect("one coffer row, not two");
    assert_eq!(exist, 6002, "the row took the new id");
    assert!((returned_at - 3_400.0).abs() < f64::EPSILON);
    let (kind, silvers): (String, i64) = ledger
        .connection()
        .query_row(
            "SELECT kind, silvers FROM loot_events WHERE id = ?",
            [opened_event],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("opening");
    assert_eq!((kind.as_str(), silvers), ("box", 1576));
    let content: (i64, String) = ledger
        .connection()
        .query_row(
            "SELECT exist_id, noun FROM loot_items WHERE kind = 'content'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("the scroll, from the inventory model");
    assert_eq!(content, (6003, "scroll".to_owned()));
}

#[test]
fn the_bank_and_the_bounty_are_transactions() {
    let mut ledger = Ledger::in_memory(Some("Tester")).expect("opens");
    let mut state = GameState::default();
    let wire = "[You have earned 925 bounty points, 800 experience points, and 9250 silver.]\n"
        .to_owned()
        + &prompt(10)
        + "You deposit 9,250 silvers into your account.  The teller carefully records the transaction.\n"
        + &prompt(20);
    assert_eq!(record(&mut ledger, &mut state, &wire).expect("records"), 2);
    assert_eq!(count(&ledger, "bounty_rewards").expect("q"), 1);
    let categories: Vec<String> = {
        let mut stmt = ledger
            .connection()
            .prepare("SELECT category FROM transactions ORDER BY id")
            .expect("prepare");
        stmt.query_map([], |r| r.get(0))
            .expect("query")
            .collect::<rusqlite::Result<_>>()
            .expect("rows")
    };
    assert_eq!(categories, ["bounty", "deposit"]);
}
