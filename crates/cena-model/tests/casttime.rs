//! A cast roundtime reads as a roundtime does (`plan/30` §3): from the wire's
//! end instant, against the server clock, unknown only when the clock is.

use cena_model::GameState;
use cena_protocol::Parser;

fn fed(wire: &[u8]) -> GameState {
    let mut state = GameState::default();
    let mut parser = Parser::new();
    for frame in parser.push_bytes(wire) {
        state.apply(&frame);
    }
    state
}

#[test]
fn a_cast_roundtime_runs_to_its_end_instant() {
    let state =
        fed(b"<castTime value=\"1789775824\"/><prompt time=\"1789775821\">C&gt;</prompt>\n");
    assert_eq!(state.cast_time_ends, Some(1_789_775_824));
    assert_eq!(state.in_casttime_after(0), Some(true));
    assert_eq!(state.casttime_remaining_after(0), Some(3));
    assert_eq!(
        state.in_casttime_after(3),
        Some(false),
        "at its value, it is over"
    );
    assert_eq!(state.casttime_remaining_after(5), Some(0), "never negative");
    assert_eq!(
        state.in_roundtime_after(0),
        Some(false),
        "a cast roundtime is not a roundtime"
    );
}

#[test]
fn unknown_only_when_the_clock_is() {
    assert_eq!(GameState::default().in_casttime_after(0), None);
    let state = fed(b"<prompt time=\"1789775821\">&gt;</prompt>\n");
    assert_eq!(
        state.in_casttime_after(0),
        Some(false),
        "one never reported is over, as Lich's 0 start says"
    );
}
