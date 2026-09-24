//! **MO-1: the local clock leaks into compared state through `Effect::ends_at`.**
//!
//! `GameState`'s `PartialEq` destructures `game_time_received` to `_` on
//! purpose: it is an `Instant` read from the local machine, so including it
//! would make a replayed state unequal to the live one it replays. That
//! exclusion is correct and it is not sufficient.
//!
//! `ends_at` is built from `game_time_now()`, which is
//! `game_time + game_time_received.elapsed()`. So the excluded value flows
//! straight back into the comparison through `effects`, which IS compared.
//!
//! Live: the character idles, a Buffs refill arrives some seconds after the
//! prompt that set the clock, and `ends_at = base + elapsed + secs`.
//! Replayed: the same bytes arrive back-to-back, `elapsed` is ~0, and
//! `ends_at = base + secs`. Same input, unequal output.
//!
//! `cena-session/tests/replay_determinism.rs` names "no `Instant::now()` in
//! session code" as its handling for wall-clock nondeterminism. `apply` calls
//! it on every prompt, and it is `std::time::Instant`, which
//! `start_paused = true` does not touch.

use cena_model::GameState;
use cena_protocol::frame::{Frame, ProgressBar};

/// The prompt that teaches the server clock.
fn prompt_at(server_time: u32) -> Frame {
    Frame::Prompt {
        time: server_time.to_string(),
        text: ">".to_owned(),
    }
}

/// A Buffs row carrying a duration, as the wire sends it.
fn buff_with_remaining(id: &str, secs: u32) -> Frame {
    Frame::ProgressBar(ProgressBar {
        id: id.to_owned(),
        dialog: Some("Buffs".to_owned()),
        text: "Spirit Warding I".to_owned(),
        percent: 50,
        amount: None,
        time_remaining_secs: Some(secs),
        attrs: Vec::new(),
    })
}

/// **Two runs of the same frames must agree, whatever the wall clock did.**
///
/// The delay stands in for the gap between a prompt and a later refill, which
/// on a live connection is however long the character idled. The replay of
/// that same recording has no such gap.
///
/// A real sleep is used rather than a mocked clock because the value under
/// test is a `std::time::Instant`: there is no seam to inject, which is
/// precisely the defect. It is one second, which is the smallest delay
/// `game_time_now`'s `as_secs()` can observe.
#[test]
fn an_effects_end_time_does_not_depend_on_how_long_the_client_waited() {
    let frames = [prompt_at(1_789_775_821), buff_with_remaining("101", 120)];

    // The "replay": frames applied back to back, as `ReplaySource` delivers
    // them.
    let mut replayed = GameState::default();
    for frame in &frames {
        replayed.apply(frame);
    }

    // The "live" run: the same frames, with the character idling in between.
    let mut live = GameState::default();
    live.apply(&frames[0]);
    std::thread::sleep(std::time::Duration::from_millis(1_100));
    live.apply(&frames[1]);

    assert_eq!(
        live, replayed,
        "the same frames produced two different states because the client \
         waited between them. `game_time_received` is excluded from PartialEq \
         to prevent exactly this, but the same Instant reading flows back in \
         through `Effect::ends_at`, which is compared. A replay of a \
         recording cannot reproduce the session it recorded."
    );
}

/// The end time is what the SERVER said, not what the client observed.
///
/// Stated separately from the equality test because it is the property that
/// makes the equality hold, and it is the one a caller depends on: an effect
/// ending at server-second N must read as ending at N however long ago the
/// client learned the clock.
#[test]
fn an_end_time_is_the_server_clock_plus_the_duration() {
    let base = 1_789_775_821;
    let mut state = GameState::default();
    state.apply(&prompt_at(base));
    std::thread::sleep(std::time::Duration::from_millis(1_100));
    state.apply(&buff_with_remaining("101", 120));

    let ends_at = state
        .effects
        .get("101")
        .and_then(|e| e.ends_at)
        .unwrap_or_else(|| panic!("the buff must be recorded: {:?}", state.effects));

    assert_eq!(
        ends_at,
        base + 120,
        "the effect ends 120 server-seconds after the clock the SERVER sent. \
         A second of local waiting between the prompt and the refill must not \
         move it: the game did not say the spell lasts longer because the \
         client was slow to read the next frame."
    );
}
