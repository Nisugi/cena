//! The `Spells` stream, typed: which spells the game lists for a character.
//!
//! `spells_known.xml` is a real login's list, cut 2026-09-21 from a live log
//! (`FIXTURES.md` has the provenance). The counts asserted here were MEASURED
//! on the fixture before the test was written:
//!
//! ```sh
//! grep -c 'noun="[0-9]' spells_known.xml   -> 53
//! ```

use cena_model::GameState;
use cena_model::state::known_spells::{self, Row};
use cena_protocol::Parser;

const LIST: &str = include_str!("../../cena-protocol/tests/fixtures/spells_known.xml");

fn fed(wire: &str) -> GameState {
    let mut state = GameState::default();
    for frame in Parser::new().push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
    state
}

#[test]
fn a_real_login_teaches_every_listed_spell_by_number() {
    let state = fed(LIST);
    let spells = &state.known_spells;

    assert_eq!(spells.len(), 53, "every spell row, and no circle link");
    assert_eq!(spells.knows(101), Some(true));
    assert_eq!(spells.knows(1109), Some(true), "the last row of the list");
    assert_eq!(
        spells
            .knows(102)
            .zip(spells.get(102).map(|s| s.name.as_str())),
        Some((true, "Spirit Barrier"))
    );
    assert_eq!(spells.knows(1700), Some(false), "stated, and not on it");
}

#[test]
fn a_spell_is_filed_under_the_circle_header_before_it() {
    // 215 is the ONLY spell under `Major Spiritual:` and sits between two
    // other circles, so a consumer that never advanced its circle -- or
    // advanced it one row late -- files it wrongly.
    let state = fed(LIST);
    let circle = |n| {
        state
            .known_spells
            .get(n)
            .and_then(|s| s.circle.clone())
            .expect("listed, under a header")
    };
    assert_eq!(circle(140), "Minor Spiritual");
    assert_eq!(circle(215), "Major Spiritual");
    assert_eq!(circle(506), "Major Elemental");
    assert_eq!(circle(601), "Ranger Base");
}

#[test]
fn a_circle_link_is_not_a_spell() {
    // The list opens with circle LINKS -- `noun=""` -- which are menu entries.
    // Their `exist` id is the same as every spell's, so only the noun tells
    // them apart.
    let line = fed(
        "<stream id=\"Spells\"><a exist=\"-1\" coord=\"2524,1885\" noun=\"\">Minor Spiritual</a></stream>\n",
    );
    assert!(line.known_spells.is_empty());
    assert_eq!(
        known_spells::classify(&line.stream("Spells")[0]),
        None,
        "a circle link is neither a header nor a spell"
    );
}

#[test]
fn not_told_is_not_the_same_as_knows_none() {
    // A character with no spells gets the clear and nothing after it. That is
    // a STATED empty list; a session that has heard nothing is not.
    assert_eq!(GameState::default().known_spells.knows(101), None);

    let squire = fed("<clearStream id=\"Spells\"/>\n");
    assert_eq!(squire.known_spells.knows(101), Some(false));
    assert!(squire.known_spells.is_stated());
}

#[test]
fn a_second_list_replaces_the_first_rather_than_adding_to_it() {
    // The game sends the list WHOLE after a clear. A spell that is no longer
    // on it -- unlearned, a fixskills -- must not survive from the last one.
    let mut wire = LIST.to_owned();
    wire.push_str(
        "<clearStream id=\"Spells\"/>\n\
          <stream id=\"Spells\">Minor Spiritual:</stream>\n\
          <stream id=\"Spells\">  <a exist=\"-1\" coord=\"2524,1865\" noun=\"101\">Spirit Warding I</a></stream>\n",
    );
    let state = fed(&wire);
    assert_eq!(state.known_spells.len(), 1);
    assert_eq!(state.known_spells.knows(102), Some(false));
}

#[test]
fn a_reconnect_forgets_the_list_until_the_burst_resends_it() {
    let mut state = fed(LIST);
    state.invalidate_for_reconnect();
    assert_eq!(state.known_spells.knows(101), None);
}

#[test]
fn the_three_row_shapes_classify_as_themselves() {
    let state = fed(LIST);
    let rows: Vec<_> = state
        .stream("Spells")
        .iter()
        .filter_map(known_spells::classify)
        .collect();
    let circles = rows.iter().filter(|r| matches!(r, Row::Circle(_))).count();
    let spells = rows
        .iter()
        .filter(|r| matches!(r, Row::Spell { .. }))
        .count();
    assert_eq!((circles, spells), (5, 53));
}
