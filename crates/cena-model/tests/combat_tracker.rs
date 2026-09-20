//! The tracker's own contract: the pending queue's bound, and what a
//! reconnect forgets.

mod fsm_harness;

use cena_model::GameState;
use cena_model::state::combat::tracker::MAX_PENDING_CHUNKS;
use fsm_harness::{bolded, run};

/// **Guard before clear**, twice: a held cast is known, then a reconnect
/// forgets it; a held pre-flare is known, then a reconnect forgets it. Two
/// states, because a held cast re-opened as the current event CLAIMS the
/// next chunk's pre-flare (as Lich's does), so one state cannot hold both.
#[test]
fn a_reconnect_forgets_a_held_cast_and_a_held_pre_flare() {
    let zerk = bolded(121_654_846, "berserker", "a tattooed gigas berserker");
    let mut state = GameState::default();
    run(
        &mut state,
        &[
            &format!("You gesture at {zerk}."),
            "Cast Roundtime 1 Second.",
        ],
    );
    assert!(
        state.combat().holds_a_cast(),
        "guard: the cast is held before the clear"
    );
    state.invalidate_for_reconnect();
    assert!(!state.combat().holds_a_cast());

    let mut state = GameState::default();
    run(
        &mut state,
        &[
            &format!(
                " ** Your <a exist=\"125479289\" noun=\"bow\">glowbark long bow</a> glows brightly for a moment, consuming the magical energies around the {}! **",
                bolded(123_956_079, "mastodon", "armored battle mastodon")
            ),
            "   ... 20 points of damage!",
        ],
    );
    assert_eq!(
        state.combat().held_pre_flare_count(),
        1,
        "guard: the pre-flare is held before the clear"
    );
    state.invalidate_for_reconnect();
    assert_eq!(state.combat().held_pre_flare_count(), 0);
    assert!(state.combat().active_assault().is_none());
}

/// A session with no consumer draining facts does not grow without limit:
/// the oldest chunk's facts are dropped, and counted (Rule 2.2).
#[test]
fn undrained_facts_are_bounded_and_the_drops_are_counted() {
    let orc = bolded(4242, "orc", "a greater orc");
    let mut state = GameState::default();
    let mut parser = cena_protocol::Parser::new();
    for _ in 0..(MAX_PENDING_CHUNKS + 3) {
        let wire = format!(
            "You swing a slim short sword at {orc}!\n   ... and hits for 30 points of damage!\n<prompt time=\"1\">&gt;</prompt>\n"
        );
        for frame in parser.push_bytes(wire.as_bytes()) {
            state.apply(&frame);
        }
    }
    assert_eq!(state.combat().dropped(), 3);
    assert_eq!(
        state.combat().chunks_seen(),
        (MAX_PENDING_CHUNKS + 3) as u64
    );
    assert_eq!(state.combat_mut().take_facts().len(), MAX_PENDING_CHUNKS);
    assert_eq!(state.combat().last_facts(), None);
}
