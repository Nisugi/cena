//! Creatures the game will not let the hunt target, remembered by name
//! (`hunt/untargetable.rs`).

mod engine_support;

use cena_behavior::hunt::Hunt;
use cena_session::GameState;
use engine_support::*;

/// The author, 2026-10-01: a creature the game will not let the hunt target
/// is remembered by name, as bigshot's `CharSettings['untargetable']`: the
/// next one by that name is not chosen, and the name is handed out to keep.
#[test]
fn a_creature_that_cannot_be_targeted_is_left_by_name() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let mut state = state(1_000, "10");
    creature(&mut state, 42, "golem", &[]);
    let at = here(10, NO_EXITS);
    assert_eq!(
        hunt.tick(&state, at, Some(1_000)),
        send("target #42", Some(42))
    );
    hunt.replied(["You can't target that."], Some(1_000));
    creature(&mut state, 43, "golem", &[]);
    creature(&mut state, 44, "warg", &[]);
    assert_eq!(
        hunt.tick(&state, at, Some(1_001)),
        send("target #44", Some(44)),
        "another golem is not chosen"
    );
    assert_eq!(hunt.take_untargetable(), ["golem"]);
    assert!(hunt.take_untargetable().is_empty(), "handed out once");

    // A later hunt starts knowing it.
    let mut later = Hunt::new(profile().unwrap(), 1).with_untargetable(["golem".to_owned()].into());
    let mut golems = state_with_golem();
    creature(&mut golems, 45, "warg", &[]);
    assert_eq!(
        later.tick(&golems, at, Some(1_000)),
        send("target #45", Some(45))
    );
}

fn state_with_golem() -> GameState {
    let mut state = state(1_000, "10");
    creature(&mut state, 50, "golem", &[]);
    state
}
