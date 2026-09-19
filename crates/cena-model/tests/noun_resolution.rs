//! **M2 step 5: resolving a noun to a thing.**
//!
//! `plan/18` §2e: *"every interactable carries a stable id... it is what makes
//! 'the doublet' resolvable to a thing. Cheap, and every later behavior depends
//! on it."*
//!
//! The **capture** of `exist=`/`noun=` happened in steps 2 and 4. This is the
//! lookup, and the two decisions it encodes are the whole of the step:
//!
//! 1. **Not a separate registry.** A second copy of every item, kept in sync,
//!    would be a cache with two writers: when a creature dies the game re-sends
//!    `room objs` without it, and an accumulating registry would keep offering it
//!    forever. Reading through to the live collections means a thing resolves for
//!    exactly as long as the game still says it is there.
//!
//! 2. **Every match, not the best one.** `noun=` is what a command targets and
//!    the game disambiguates with ordinals, so silently taking the first match
//!    would send `attack artificer` at the wrong artificer and report success.

use cena_model::{GameState, Where};
use cena_protocol::Parser;

fn fold(wire: &[u8]) -> GameState {
    let mut state = GameState::default();
    let mut parser = Parser::new();
    for frame in parser.push_bytes(wire) {
        state.apply(&frame);
    }
    state
}

/// Two creatures with the SAME noun, plus a floor object -- the real ambiguity.
const ROOM: &[u8] = b"<component id='room objs'>  You also see<b> <pushBold/>a \
<a exist=\"240632\" noun=\"artificer\">gaunt masked artificer</a><popBold/></b> and<b> <pushBold/>a \
<a exist=\"240633\" noun=\"artificer\">hooded artificer</a><popBold/></b>, a \
<a exist=\"18122889\" noun=\"arrow\">wooden arrow</a>.</component>\n\
<component id='room players'>Also here: <a exist=\"-10070682\" noun=\"Dicate\">Dicate</a></component>\n\
<container id='stow' title=\"My Cloak\" target='#64863904'/>\
<inv id='stow'> an <a exist=\"64863936\" noun=\"arrow\">ornate arrow</a></inv>\n";

#[test]
fn an_unambiguous_noun_resolves_to_one_thing() {
    let state = fold(ROOM);
    let found = state.resolve_noun("Dicate");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].item.text, "Dicate");
    assert_eq!(found[0].found_in, Where::Player);
}

#[test]
fn an_ambiguous_noun_returns_every_match_rather_than_guessing() {
    // TWO creatures named `artificer`. A consumer that took the first would
    // attack the wrong one and be told it worked.
    let state = fold(ROOM);
    let found = state.resolve_noun("artificer");

    assert_eq!(
        found.len(),
        2,
        "an ambiguous noun was silently resolved to one thing"
    );
    assert_eq!(found[0].item.id, "240632");
    assert_eq!(found[1].item.id, "240633");
}

#[test]
fn a_noun_that_spans_the_room_and_a_container_reports_both_places() {
    // `arrow` is on the floor AND in the cloak. Where it is changes what a
    // command can do with it -- one can be picked up, one is already held.
    let state = fold(ROOM);
    let found = state.resolve_noun("arrow");

    assert_eq!(found.len(), 2);
    assert_eq!(found[0].found_in, Where::RoomObject);
    assert_eq!(found[1].found_in, Where::Container("stow"));
}

#[test]
fn creatures_are_searched_before_floor_objects() {
    // The order is defined, and this is the one ordering decision with a reason
    // rather than a convention: a noun matching both a creature and an item is
    // overwhelmingly meant as the creature.
    let state = fold(
        b"<component id='room objs'>  You also see<b> <pushBold/>a \
<a exist=\"1\" noun=\"bear\">angry bear</a><popBold/></b>, a \
<a exist=\"2\" noun=\"bear\">bear pelt</a>.</component>\n",
    );
    let found = state.resolve_noun("bear");

    assert_eq!(found.len(), 2);
    assert_eq!(
        found[0].found_in,
        Where::Creature,
        "the floor item was offered before the creature"
    );
}

#[test]
fn an_unknown_noun_resolves_to_nothing() {
    let state = fold(ROOM);
    assert!(state.resolve_noun("dragon").is_empty());
}

#[test]
fn an_exist_id_resolves_unambiguously() {
    // Ids are unique where nouns are not. This is what a consumer uses after
    // CHOOSING among the noun's matches, to check the thing is still there.
    let state = fold(ROOM);
    let found = state.find_by_id("240633").expect("the second artificer");
    assert_eq!(found.item.text, "hooded artificer");
    assert_eq!(found.found_in, Where::Creature);
}

#[test]
fn a_thing_that_leaves_the_room_stops_resolving() {
    // **The staleness bug a separate registry would have.** The game re-sends
    // `room objs` WITHOUT the dead creature; an accumulating registry would keep
    // offering it forever, and a behavior would attack a corpse that is not
    // there.
    let mut wire = ROOM.to_vec();
    wire.extend_from_slice(
        b"<component id='room objs'>  You also see a <a exist=\"18122889\" noun=\"arrow\">wooden arrow</a>.</component>\n",
    );
    let state = fold(&wire);

    assert!(
        state.resolve_noun("artificer").is_empty(),
        "a creature that left the room still resolved"
    );
    assert!(
        state.find_by_id("240632").is_none(),
        "and it was still findable by id"
    );
    assert_eq!(
        state.resolve_noun("arrow").len(),
        2,
        "...while everything still present must still resolve"
    );
}

#[test]
fn a_reconnect_leaves_nothing_resolvable() {
    let mut state = fold(ROOM);
    assert!(!state.resolve_noun("artificer").is_empty());

    state.invalidate_for_reconnect();

    assert!(state.resolve_noun("artificer").is_empty());
    assert!(state.resolve_noun("arrow").is_empty());
}
