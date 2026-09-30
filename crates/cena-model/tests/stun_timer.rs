//! The character's own stun as a timer (`state/stun.rs`): its rounds read
//! from the game's line, five seconds a round from the prompt that closed
//! it, lengthened and never shortened by a second stun, and ended by the
//! `IconSTUNNED` indicator going dark.

use cena_model::state::GameState;
use cena_protocol::Parser;

fn read(markup: &str, state: &mut GameState) {
    for frame in Parser::new().push_bytes(markup.as_bytes()) {
        state.apply(&frame);
    }
}

#[test]
fn a_stun_counts_down_from_its_rounds_until_the_indicator_goes_dark() {
    let mut state = GameState::default();
    read("<prompt time=\"990\">&gt;</prompt>\n", &mut state);
    assert_eq!(state.stun_remaining_after(0), Some(0), "none told");

    read(
        "<indicator id='IconSTUNNED' visible='y'/>   You are stunned for 2 rounds!\n\
         <prompt time=\"1000\">&gt;</prompt>\n",
        &mut state,
    );
    assert_eq!(state.stun_ends, Some(1010));
    assert_eq!(state.stun_remaining_after(3), Some(7));

    // A crit's own way of saying it, shorter: what is left stays.
    read(
        "You are stunned 1 rounds!\n<prompt time=\"1002\">&gt;</prompt>\n",
        &mut state,
    );
    assert_eq!(state.stun_ends, Some(1010), "never shortened");
    read(
        "You are stunned for 3 rounds!\n<prompt time=\"1004\">&gt;</prompt>\n",
        &mut state,
    );
    assert_eq!(state.stun_ends, Some(1019), "lengthened");

    read(
        "<indicator id='IconSTUNNED' visible='n'/>\n<prompt time=\"1006\">&gt;</prompt>\n",
        &mut state,
    );
    assert_eq!(state.stun_ends, None, "the indicator ends it");
    assert_eq!(state.stun_remaining_after(0), Some(0));
}
