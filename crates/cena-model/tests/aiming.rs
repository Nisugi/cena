//! Where the character aims, as the game last said it (the author,
//! 2026-09-30, with the game's own lines: *"aim right eye"*), kept by the
//! model for the Aim widget.

use cena_model::state::GameState;
use cena_protocol::Parser;

fn read(markup: &str, state: &mut GameState) {
    for frame in Parser::new().push_bytes(markup.as_bytes()) {
        state.apply(&frame);
    }
}

#[test]
fn the_place_aimed_at_is_kept_until_the_game_says_another_or_none() {
    let mut state = GameState::default();
    assert_eq!(state.aiming, None, "not said");
    read(
        "You're now aiming at the right eye of your target when using a ranged weapon, or while ambushing.\n\
         <prompt time=\"1000\">&gt;</prompt>\n",
        &mut state,
    );
    assert_eq!(state.aiming.as_deref(), Some("right eye"));
    read(
        "You're now aiming at the left eye of your target when using a ranged weapon, or while ambushing.\n\
         <prompt time=\"1001\">&gt;</prompt>\n",
        &mut state,
    );
    assert_eq!(state.aiming.as_deref(), Some("left eye"));
    read(
        "You're now no longer aiming at anything in particular.\n<prompt time=\"1002\">&gt;</prompt>\n",
        &mut state,
    );
    assert_eq!(state.aiming, None);
}
