//! Maneuver cooldowns: the one PSM refusal that is a fact, not a reply.
//!
//! Found by reading Lich's PSM sources rather than a log, then checking the
//! author's combat logs for what the game actually says. MEASURED over
//! `GST-Nisugi` (390 files): 15 cooldown lines naming five distinct maneuvers
//! -- Volley, Barrage, Whirlwind, Guardant Thrusts, Pulverize -- and verified
//! on the raw wire, so it is game text and not a client artifact.
//!
//! Everything else in `psms.rb` is about SENDING: the eleven
//! `FAILURES_REGEXES` and the per-command `results_regex` exist so a script can
//! recognise the reply to its own `cman bullrush`. That needs the authority
//! token and a roundtime, so it is M6.

use cena_model::GameState;
use cena_model::state::maneuvers::{cooldown_refusal, ready_line};
use cena_protocol::Parser;

/// Verbatim from `2025-09-04_05-28-18.xml`, including the prompt that closes
/// the chunk and carries the server clock.
const BARRAGE: &str = concat!(
    "Barrage is still in cooldown.\n",
    "<prompt time=\"1756984481\">&gt;</prompt>\n",
);

fn fed(wire: &str) -> GameState {
    let mut state = GameState::default();
    for frame in Parser::new().push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
    state
}

#[test]
fn a_real_refusal_reaches_the_model_with_the_server_second() {
    let state = fed(BARRAGE);
    assert_eq!(
        state.maneuvers.said_at("Barrage"),
        Some(Some(1_756_984_481)),
        "the prompt's clock is when we were told"
    );
}

#[test]
fn never_said_is_not_said_with_no_clock() {
    // Two distinct `None`s, and the nesting is the point: `None` is "never
    // said", `Some(None)` is "said, and no clock was known" -- which is a real
    // state before the first prompt of a connection.
    let state = fed(BARRAGE);
    assert_eq!(state.maneuvers.said_at("Volley"), None, "never said");

    let mut clockless = GameState::default();
    assert!(clockless.maneuvers.note_cooling("Volley", None));
    assert_eq!(clockless.maneuvers.said_at("Volley"), Some(None));
}

#[test]
fn every_maneuver_the_author_was_refused_classifies() {
    // The five from the logs, in the game's own spelling -- including the
    // two-word one, which a classifier that took a single token would lose.
    for name in [
        "Volley",
        "Barrage",
        "Whirlwind",
        "Guardant Thrusts",
        "Pulverize",
    ] {
        assert_eq!(
            cooldown_refusal(&format!("{name} is still in cooldown.")),
            Some(name)
        );
    }
}

#[test]
fn a_client_timestamp_prefix_is_not_part_of_the_name() {
    // MEASURED: the author's logs carry both forms, the prefixed one from a
    // client with per-line timestamps on. It never reaches the model through
    // `GameState`, but the classifier should not be brittle about it.
    assert_eq!(
        cooldown_refusal("23:53:12: Volley is still in cooldown."),
        Some("Volley")
    );
}

#[test]
fn something_that_is_not_a_refusal_is_not_one() {
    assert_eq!(cooldown_refusal("You see nothing unusual."), None);
    assert_eq!(cooldown_refusal(""), None);
    // A nameless refusal: `trim` leaves exactly the suffix, so there is
    // nothing before it to strip and this is not a refusal at all. MEASURED --
    // an empty-name guard in `cooldown_refusal` was unreachable and was
    // deleted rather than kept untestable.
    assert_eq!(cooldown_refusal(" is still in cooldown."), None);
    assert_eq!(cooldown_refusal("is still in cooldown."), None);
    // A near miss: the game's OTHER refusals are not this one, and each means
    // something different (`psms.rb:208`).
    assert_eq!(
        cooldown_refusal("You lack the momentum to attempt another skill."),
        None
    );
}

#[test]
fn a_second_refusal_updates_when_we_were_told() {
    let mut state = GameState::default();
    assert!(state.maneuvers.note_cooling("Volley", Some(100)));
    assert!(
        !state.maneuvers.note_cooling("Volley", Some(100)),
        "the same reading is not a change"
    );
    assert!(state.maneuvers.note_cooling("Volley", Some(160)));
    assert_eq!(state.maneuvers.said_at("Volley"), Some(Some(160)));
}

#[test]
fn several_maneuvers_are_tracked_independently() {
    let wire = concat!(
        "Volley is still in cooldown.\n",
        "Barrage is still in cooldown.\n",
        "<prompt time=\"500\">&gt;</prompt>\n",
    );
    let state = fed(wire);
    assert_eq!(state.maneuvers.len(), 2);
    let names: Vec<&str> = state.maneuvers.cooling().map(|(n, _)| n).collect();
    assert_eq!(
        names,
        ["Barrage", "Volley"],
        "name order, so a diff is stable"
    );
}

#[test]
fn a_reconnect_forgets_them_because_a_cooldown_cannot_be_aged() {
    // Unlike `roundtime_ends`, which the server enforces across the gap and so
    // is KEPT: a cooldown carries no duration, so a stale "was cooling" can
    // never expire and is worse than no reading.
    let mut state = fed(BARRAGE);
    assert!(!state.maneuvers.is_empty(), "guard");
    state.invalidate_for_reconnect();
    assert!(state.maneuvers.is_empty());
    assert_eq!(state.maneuvers.said_at("Barrage"), None);
}

/// `Volley is ready for use.` (`cena-behavior/tests/fixtures/smithy_engage.xml:271`)
/// ends the refusal the model held (`inventory/12` §1.3).
#[test]
fn the_ready_line_ends_a_held_refusal() {
    assert_eq!(ready_line("Volley is ready for use."), Some("Volley"));
    assert_eq!(
        ready_line("Guardant Thrusts is ready for use."),
        Some("Guardant Thrusts")
    );
    assert_eq!(ready_line("Your spell is ready."), None);
    assert_eq!(ready_line(" is ready for use."), None);
    let state = fed(concat!(
        "Volley is still in cooldown.\n",
        "<prompt time=\"1000\">&gt;</prompt>\n",
        "Volley is ready for use.\n",
        "<prompt time=\"1030\">&gt;</prompt>\n",
    ));
    assert_eq!(state.maneuvers.said_at("Volley"), None);
}
