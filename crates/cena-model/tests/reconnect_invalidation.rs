//! `plan/12` §5.2: what a reconnect forgets, and what it keeps.
//!
//! The fixtures are the shapes the live server actually sent, from Cena's own
//! captures of 2026-09-18 — the indicator burst, a real roundtime, a real
//! effect id, and the vitals the login burst carries.
//!
//! # Every test here sets the fact before clearing it
//!
//! A test that asserted `roundtime_ends == None` on a `GameState::default()`
//! would pass without `invalidate_for_reconnect` existing at all. So each
//! assertion is preceded by an assertion that the fact was **known**, which is
//! what makes the clearing the thing under test rather than the default.

use cena_model::{GameState, Room};
use cena_protocol::Frame;

/// Drive a burst through the real parser, as a live session would.
fn state_from(wire: &str) -> GameState {
    let mut parser = cena_protocol::Parser::new();
    let mut state = GameState::default();
    for frame in parser.push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
    state
}

/// A session that knows everything, so that clearing it proves something.
fn a_fully_known_session() -> GameState {
    let mut state = state_from(concat!(
        "<nav rm='7503251'/>",
        "<compass><dir value=\"n\"/><dir value=\"e\"/></compass>",
        "<roundTime value='1789775824'/>",
        r#"<indicator id="IconSTANDING" visible="y"/>"#,
        r#"<indicator id="IconSTUNNED" visible="n"/>"#,
        "<left>a brass lantern</left><right>a steel broadsword</right>",
        "<progressBar id='health' value='97'/>",
        "<progressBar id='mana' value='42'/>",
        "<prompt time=\"1789775821\">R&gt;</prompt>\n",
    ));
    // An effect, in the shape the incant-515 capture carried.
    state.effects.insert(
        "515".to_owned(),
        cena_model::Effect {
            category: "Buffs".to_owned(),
            text: "Rapid Fire".to_owned(),
            ends_at: Some(1_789_776_000),
            percent: 100,
        },
    );
    // An unmodelled tag, criterion 8's evidence.
    state.apply(&Frame::UnknownTag {
        name: "somethingNew".to_owned(),
        raw: "<somethingNew/>".to_owned(),
    });
    state
}

/// The facts the login burst does **not** re-send all return to `Unknown`.
///
/// MEASURED 2026-09-18, 7/7 logins: the burst carries no `nav rm`, no
/// `compass`, no `prompt`, no hands, no `roundTime`, no `indicator` and none of
/// the four effect dialogs (`plan/15` §2b). A reconnected session genuinely has
/// not been told them, so `Unknown` is the only honest value.
#[test]
fn facts_the_burst_omits_return_to_unknown() {
    let mut state = a_fully_known_session();

    // GUARD: without this, the assertions below pass on a default state and
    // the method under test could be empty.
    assert_eq!(state.room.id.as_deref(), Some("7503251"));
    assert!(
        state.room.exits.as_ref().is_some_and(|e| !e.is_empty()),
        "exits were observed"
    );
    assert!(state.prompt.is_some(), "a prompt was observed");
    assert!(state.left_hand.is_some() && state.right_hand.is_some());
    assert_eq!(state.roundtime_ends, Some(1_789_775_824));
    assert!(state.status.standing(), "IconSTANDING was reported y");
    assert!(
        state.status.is_known("stunned"),
        "IconSTUNNED was reported n"
    );
    assert_eq!(state.effects.len(), 1);
    assert!(state.game_time_now().is_some(), "the clock was calibrated");

    state.invalidate_for_reconnect();

    assert_eq!(state.room, Room::default(), "the room goes whole");
    assert_eq!(state.prompt, None);
    assert_eq!(state.left_hand, None);
    assert_eq!(state.right_hand, None);
    // **RETAINED**, and this assertion was flipped deliberately. It read
    // `None`, encoding §5.2's "roundtime after reconnect is Unknown, never 0"
    // literally -- but clearing it produced the opposite of Unknown: a review
    // reproduced `in_roundtime() == Some(false)` with 20 real seconds left,
    // because `clock.rs`'s `is_some_and` maps a cleared end to "not in
    // roundtime" the moment a prompt restores the clock.
    //
    // `roundtime_ends` is an ABSOLUTE SERVER EPOCH, so unlike the room or the
    // hands it cannot have gone stale while the socket was down. Keeping it is
    // keeping a true fact; what §5.2 forbids is asserting "not in roundtime" on
    // no evidence, which the clock's own invalidation below still prevents.
    assert_eq!(
        state.roundtime_ends,
        Some(1_789_775_824),
        "an absolute server epoch survives a reconnect -- clearing it made          `in_roundtime` report `Some(false)` during a live roundtime"
    );
    assert!(state.effects.is_empty());
}

/// **Unknown is not `false`**, and `StatusInfo` is where that distinction is
/// easiest to get wrong.
///
/// `get` collapses "reported inactive" with "never reported"; only `is_known`
/// separates them. So asserting `!state.status.standing()` would pass on an
/// implementation that wrote `false` into every key rather than removing it —
/// which is a confident belief about a connection that has ended, exactly what
/// `plan/12` §5.2 forbids.
#[test]
fn cleared_indicators_are_unknown_rather_than_reported_false() {
    let mut state = a_fully_known_session();
    assert!(state.status.is_known("standing"), "guard: it was reported");

    state.invalidate_for_reconnect();

    assert!(!state.status.standing(), "it no longer reads true");
    assert!(
        !state.status.is_known("standing"),
        "...and it must be UNKNOWN, not known-false. A new generation has been \
         told nothing; writing `false` would be a belief nobody reported."
    );
}

/// The clock is per-connection, and **both halves** of it go.
///
/// This is the subtle one. `game_time_now` extrapolates from the local instant
/// the reading arrived, so a clock carried across a reconnect reports a server
/// time as far in the future as the outage was long — and `in_roundtime`
/// compares against it. A stale clock is worse than no clock.
#[test]
fn the_clock_does_not_survive_a_reconnect() {
    let mut state = a_fully_known_session();
    assert!(state.game_time_now().is_some(), "guard: it was calibrated");

    state.invalidate_for_reconnect();

    assert_eq!(state.game_time_now(), None, "the server clock is unknown");
    assert_eq!(
        state.in_roundtime(),
        None,
        "with no clock this is Unknown, NOT false -- a caller that wants to \
         treat unknown as 'go ahead' has to write that decision down"
    );
    assert_eq!(state.roundtime_remaining(), None);
}

/// Vitals survive, because the burst re-sends them unprompted.
///
/// MEASURED: ten `progressBar`s in every login burst. Clearing them would open
/// a window where health reads `Unknown` for no reason — the burst is about to
/// confirm them anyway.
#[test]
fn vitals_survive_because_the_burst_re_sends_them() {
    let mut state = a_fully_known_session();
    assert_eq!(state.vitals.get("health"), Some(&97));

    state.invalidate_for_reconnect();

    assert_eq!(
        state.vitals.get("health"),
        Some(&97),
        "the login burst carries ten progressBars (MEASURED, 7/7 logins), so \
         these are refreshed rather than unobserved"
    );
    assert_eq!(state.vitals.get("mana"), Some(&42));
}

/// Unknown tags survive: they are a fact about the protocol, not the socket.
///
/// Criterion 8's evidence, and it is most useful *across* a reconnect — a tag
/// that appeared before a drop is exactly what a reader needs when working out
/// why the session dropped.
#[test]
fn unknown_tags_survive_because_they_belong_to_the_session() {
    let mut state = a_fully_known_session();
    assert_eq!(state.unknown_tags.len(), 1, "guard: one was recorded");

    state.invalidate_for_reconnect();

    assert_eq!(
        state.unknown_tags.len(),
        1,
        "criterion 8's evidence belongs to the SESSION, not to one connection"
    );
    assert_eq!(state.unknown_tags[0].name, "somethingNew");
}

/// Invalidating twice is the same as once, and invalidating a fresh state is
/// harmless.
///
/// The supervisor calls this between generations; a reconnect that fails and
/// retries calls it again. It must not depend on having something to clear.
#[test]
fn invalidation_is_idempotent() {
    let mut once = a_fully_known_session();
    once.invalidate_for_reconnect();
    let mut twice = a_fully_known_session();
    twice.invalidate_for_reconnect();
    twice.invalidate_for_reconnect();
    assert_eq!(once, twice);

    let mut fresh = GameState::default();
    fresh.invalidate_for_reconnect();
    assert_eq!(
        fresh,
        GameState::default(),
        "clearing nothing changes nothing"
    );
}
