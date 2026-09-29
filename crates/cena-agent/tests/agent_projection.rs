//! The agent's projection of a character, issue #19's point 1: a list the
//! game has not stated is `null`, and `changed` says what changed and not
//! the clock. Moved out of `agent.rs` at its cap.

use cena_agent::happenings::Happening;
use cena_agent::projection::project;
use cena_session::{GameState, Generation, SessionId, Snapshot, State};

fn snapshot(state: GameState, cursor: u64) -> Snapshot {
    Snapshot {
        session: SessionId(1),
        state,
        lifecycle: State::Ready,
        generation: Generation(0),
        cursor,
        retry: None,
        triggers: std::sync::Arc::default(),
        stopped: None,
    }
}

/// Issue #19, point 1: a list the game has not stated is `null`, never
/// empty, so an agent cannot read "nobody said" as "nobody is here".
#[test]
fn an_unstated_list_is_null_and_a_stated_empty_one_is_empty() {
    let mut state = GameState::default();
    let unstated = project("Nisugi", &snapshot(state.clone(), 1));
    assert!(unstated.room.players.is_none(), "never said who is here");
    assert!(unstated.room.creatures.is_none() && unstated.room.objects.is_none());
    for id in ["room players", "room objs"] {
        state.apply(&cena_session::Frame::Component {
            id: id.into(),
            body: cena_session::Runs { runs: Vec::new() },
        });
    }
    let stated = project("Nisugi", &snapshot(state, 2));
    assert_eq!(stated.room.players, Some(Vec::new()), "said: nobody");
    assert_eq!(stated.room.creatures, Some(Vec::new()));
}

/// Issue #19, point 1: `changed` carries each field that changed with its
/// new value, and leaves out the ones that only count the clock.
#[test]
fn changed_carries_what_changed_and_not_the_clock() {
    let mut before = GameState::default();
    before.status.set("stunned", false);
    let mut after = before.clone();
    after.status.set("stunned", true);
    let (was, mut now) = (
        project("Nisugi", &snapshot(before, 1)),
        project("Nisugi", &snapshot(after, 2)),
    );
    now.roundtime.seconds_left = Some(3);
    now.captured_unix_ms += 1_000;
    let Some(Happening::Changed { fields }) = cena_agent::happenings::changed(&was, &now) else {
        panic!("a status changed");
    };
    assert_eq!(
        fields.keys().collect::<Vec<_>>(),
        ["statuses"],
        "{fields:?}"
    );
    assert_eq!(fields["statuses"]["stunned"], true);
    assert!(
        cena_agent::happenings::changed(&was, &was).is_none(),
        "nothing changed, nothing said"
    );
}
