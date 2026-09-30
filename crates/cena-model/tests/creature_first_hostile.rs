//! A creature's hostility is what the game first said of it, kept (the
//! author, 2026-09-30: *"If first time they are seen, they are recorded as
//! hostile, then that's it, hostile can't change"*): sympathy makes a
//! hostile creature read not hostile, and it is still a target.

use cena_model::state::GameState;
use cena_protocol::Parser;

fn read(markup: &str, state: &mut GameState) {
    for frame in Parser::new().push_bytes(markup.as_bytes()) {
        state.apply(&frame);
    }
}

#[test]
fn hostile_when_first_seen_stays_hostile() {
    let mut state = GameState::default();
    read(
        "<component id='room objs'>You also see <pushBold/>a <a exist=\"71\" noun=\"warg\">niveous giant warg</a><popBold/> and <pushBold/>a <a exist=\"72\" noun=\"cat\">sleek white cat</a><popBold/>.</component>\n\
         <crtrStatus exist=\"71\" hostile=\"1\"/><crtrStatus exist=\"72\"/>\n\
         <prompt time=\"1000\">&gt;</prompt>\n",
        &mut state,
    );
    let first = |state: &GameState, id| {
        state
            .creatures()
            .get(id)
            .and_then(cena_model::CreatureInstance::hostile_when_first_seen)
    };
    assert_eq!(first(&state, 71), Some(true), "a target");
    assert_eq!(first(&state, 72), Some(false), "an NPC");
    read(
        "<crtrStatus exist=\"71\" sympathetic=\"1\"/><crtrStatus exist=\"72\" hostile=\"1\"/>\n\
         <prompt time=\"1002\">&gt;</prompt>\n",
        &mut state,
    );
    assert_eq!(first(&state, 71), Some(true), "sympathied, still a target");
    assert_eq!(
        first(&state, 72),
        Some(false),
        "first seen not hostile, stays an NPC"
    );
}
