//! Status indicators and the server clock, against the shapes the live server
//! actually sent.
//!
//! The fixtures here are taken from Cena's own capture of 2026-09-18
//! (`plan/15` §2a.4a), not invented: the indicator burst is the real one, the
//! roundtime figures are three real roundtimes, and the quoting is what the
//! wire used.

use cena_model::{GameState, StatusInfo};
use cena_protocol::Frame;

/// Ids normalise, so a caller may pass any of the three forms the codebase
/// mixes: the wire's `IconSTUNNED`, a bare `stunned`, or a shout.
#[test]
fn ids_normalise_across_the_forms_callers_use() {
    let mut s = StatusInfo::default();
    s.set("IconSTUNNED", true);

    assert!(s.get("IconSTUNNED"));
    assert!(s.get("stunned"));
    assert!(s.get("STUNNED"));
    assert!(s.stunned(), "the typed accessor reads the same entry");
}

/// **`get` collapses two different facts; `is_known` separates them.**
///
/// `plan/12` §5.2: "`Unknown` is a first-class value, not a default." After a
/// reconnect nothing has been reported, and a confident `false` would be a
/// stale belief about a connection that has ended.
#[test]
fn never_reported_is_distinguishable_from_reported_false() {
    let mut s = StatusInfo::default();
    s.set("IconSTUNNED", false);

    assert!(!s.get("stunned"), "reported inactive reads false");
    assert!(s.is_known("stunned"), "...and is KNOWN to be inactive");

    assert!(!s.get("webbed"), "never reported also reads false");
    assert!(
        !s.is_known("webbed"),
        "...but must NOT be known. Collapsing these is the stale-belief bug \
         plan/12 §5.2 exists to prevent."
    );
}

/// `set` reports whether anything changed, so a caller can skip a no-op.
///
/// This is not cosmetic: the login burst sets ten indicators at once
/// (MEASURED, `plan/15` §2a.4a) and nine of them are already what they were.
#[test]
fn set_reports_whether_the_value_actually_changed() {
    let mut s = StatusInfo::default();
    assert!(s.set("IconSTANDING", true), "first report is a change");
    assert!(!s.set("IconSTANDING", true), "same value is not a change");
    assert!(s.set("IconSTANDING", false), "flipping is a change");
}

/// A reconnect forgets everything, rather than carrying beliefs across
/// generations.
#[test]
fn clearing_returns_every_indicator_to_unknown() {
    let mut s = StatusInfo::default();
    s.set("IconSTANDING", true);
    s.clear();
    assert!(
        !s.is_known("standing"),
        "a new generation has been told nothing"
    );
    assert!(!s.standing());
}

/// The real login burst: all ten ids at once, `IconSTANDING` the only `y`.
///
/// Byte-for-byte the shape from the capture, **including the double quotes**
/// -- `<roundTime>` uses single quotes and `<indicator>` uses double, and a
/// matcher that assumes one form silently finds nothing (`plan/15` §2a.4a,
/// finding 4).
#[test]
fn the_login_indicator_burst_lands_in_state() {
    let burst = concat!(
        r#"<indicator id="IconKNEELING" visible="n"/>"#,
        r#"<indicator id="IconPRONE" visible="n"/>"#,
        r#"<indicator id="IconSITTING" visible="n"/>"#,
        r#"<indicator id="IconSTANDING" visible="y"/>"#,
        r#"<indicator id="IconSTUNNED" visible="n"/>"#,
        r#"<indicator id="IconHIDDEN" visible="n"/>"#,
        r#"<indicator id="IconINVISIBLE" visible="n"/>"#,
        r#"<indicator id="IconDEAD" visible="n"/>"#,
        r#"<indicator id="IconWEBBED" visible="n"/>"#,
        r#"<indicator id="IconJOINED" visible="n"/>"#,
        "\n",
    );

    let mut parser = cena_protocol::Parser::new();
    let mut state = GameState::default();
    for frame in parser.push_bytes(burst.as_bytes()) {
        state.apply(&frame);
    }

    assert!(state.status.standing(), "IconSTANDING was the only y");
    for off in [
        "kneeling",
        "prone",
        "sitting",
        "stunned",
        "hidden",
        "invisible",
        "dead",
        "webbed",
        "joined",
    ] {
        assert!(!state.status.get(off), "{off} was reported n");
        assert!(
            state.status.is_known(off),
            "{off} must be KNOWN-inactive, not merely unreported: the burst \
             reported it"
        );
    }
}

/// An id the wiki does not list still works, because the map is filled by id
/// rather than by field.
///
/// `IconPOISONED` and `IconDISEASED` are on the wire and absent from the
/// wiki's ten (`plan/15` §2, note 1).
#[test]
fn an_id_the_wiki_omits_is_stored_anyway() {
    let mut state = GameState::default();
    state.apply(&Frame::StatusIndicator {
        id: "IconPOISONED".to_owned(),
        active: true,
    });
    assert!(state.status.poisoned());
    assert!(state.status.is_known("IconPOISONED"));
}

/// Nothing is known before the first prompt, and `in_roundtime` says so rather
/// than guessing.
#[test]
fn the_clock_is_unknown_until_a_prompt_arrives() {
    let state = GameState::default();
    assert_eq!(state.game_time_now(), None);
    assert_eq!(
        state.in_roundtime(),
        None,
        "with no clock this must be Unknown, NOT false -- a caller that wants \
         to treat unknown as 'go ahead' has to write that down"
    );
    assert_eq!(state.roundtime_remaining(), None);
}

/// The real numbers from the capture: a 3-second roundtime, with the prompt
/// that reported it.
///
/// `<roundTime value='1789775824'/>` against `<prompt time="1789775821">R&gt;`
/// -- and the first plain `>` prompt arrived at exactly 1789775824.
#[test]
fn a_real_roundtime_is_in_effect_and_then_is_not() {
    let mut parser = cena_protocol::Parser::new();
    let mut state = GameState::default();
    let burst = "<roundTime value='1789775824'/>You don't find anything of interest here.\n\
                 Roundtime: 3 sec.\n\
                 <prompt time=\"1789775821\">R&gt;</prompt>\n";
    for frame in parser.push_bytes(burst.as_bytes()) {
        state.apply(&frame);
    }

    // `_after(0)`, NOT the wall-clock forms. This asserts the state as of the
    // instant the prompt landed, and that instant has to be stated: the
    // wall-clock form failed on CI's Windows runner because the suite crossed
    // a second boundary, reading 1789775822. See `clock.rs::game_time_after`.
    assert_eq!(state.roundtime_ends, Some(1_789_775_824));
    assert_eq!(state.game_time_after(0), Some(1_789_775_821));
    assert_eq!(state.in_roundtime_after(0), Some(true));
    assert_eq!(
        state.roundtime_remaining_after(0),
        Some(3),
        "the server said 'Roundtime: 3 sec.' and the arithmetic must agree"
    );

    // The prompt at the END second reads plain `>`, and the roundtime is over.
    // MEASURED: the first plain prompt landed precisely on `value`, which is
    // why this is `false` rather than a boundary argument.
    for frame in parser.push_bytes(b"<prompt time=\"1789775824\">&gt;</prompt>\n") {
        state.apply(&frame);
    }
    assert_eq!(state.game_time_after(0), Some(1_789_775_824));
    assert_eq!(
        state.in_roundtime_after(0),
        Some(false),
        "value IS the end instant (plan/15 §2a.4a): at value, roundtime is over"
    );
    assert_eq!(state.roundtime_remaining_after(0), Some(0));
}

/// The clock keeps counting **without** a new prompt.
///
/// This is the whole reason `game_time_now` extrapolates instead of returning
/// the last reading. A prompt is only sent when something happens, so an idle
/// client gets none -- and a roundtime that could only end on a prompt would
/// never end in a quiet room.
#[test]
fn the_clock_advances_between_prompts() {
    let mut parser = cena_protocol::Parser::new();
    let mut state = GameState::default();
    for frame in parser.push_bytes(b"<prompt time=\"1789775821\">R&gt;</prompt>\n") {
        state.apply(&frame);
    }
    let first = state.game_time_now().expect("a prompt arrived");

    std::thread::sleep(std::time::Duration::from_millis(1100));

    let later = state.game_time_now().expect("still known");
    assert!(
        later > first,
        "server time must advance with the local monotonic clock, with no new \
         prompt: {first} -> {later}"
    );
}

/// Two states that saw the same frames are equal, even though the local
/// instants differ.
///
/// **This is criterion 7's requirement, pinned at the type.** Deriving
/// `PartialEq` on `GameState` made `replay_determinism.rs` go red: two replays
/// of one recording produced `Instant`s microseconds apart, and a whole-state
/// comparison saw two different states. `Recorder`'s docs already warned that
/// "a wall clock is the single easiest way to make a replay non-deterministic";
/// this is that rule enforced one layer up.
///
/// The local receipt instant is an observation about WHEN state arrived, not
/// part of the state. Two sessions that saw the same frames are in the same
/// state regardless of when they saw them.
///
/// # This test was right and its fixture was too small
///
/// It feeds a bare prompt, so it covers only the field that `PartialEq`
/// excludes by name. It stayed green while the same `Instant` reading reached
/// the comparison through `Effect::ends_at`, which is compared -- because no
/// effect is in the fixture (review MO-1).
///
/// `tests/effects_are_replayable.rs` is the case this one could not see. The
/// pair is worth keeping separate: this asserts the exclusion, that one
/// asserts nothing routes around it.
#[test]
fn equality_ignores_when_the_clock_reading_arrived_locally() {
    let prompt = b"<prompt time=\"1789775821\">R&gt;</prompt>\n";

    let build = || {
        let mut parser = cena_protocol::Parser::new();
        let mut state = GameState::default();
        for frame in parser.push_bytes(prompt) {
            state.apply(&frame);
        }
        state
    };

    let first = build();
    std::thread::sleep(std::time::Duration::from_millis(50));
    let second = build();

    assert_eq!(
        first, second,
        "the same frames must produce equal state whatever the local clock \
         said at the time -- this is what criterion 7's replay compares"
    );
}

/// **After a reconnect, "not stunned" and "unknown" are different answers.**
///
/// The typed accessors collapse them: `status.stunned()` answers a confident
/// `false` for an indicator the game has never mentioned, and after a
/// reconnect that is every indicator — `reconnect.rs` clears the map and
/// `indicator` is absent from the login burst, unanimously across all seven
/// captured logins.
///
/// A behavior that casts on "not stunned" would then act on a belief nothing
/// supports, which is the stale-belief failure `plan/12` §5.2 exists to
/// prevent (review MO-4).
#[test]
fn an_unreported_indicator_is_unknown_not_false() {
    let mut status = cena_model::StatusInfo::default();
    status.set("stunned", true);

    // Reported, and true.
    assert!(status.stunned());
    assert_eq!(status.known().stunned(), Some(true));

    // Reported, and false: the game said no.
    status.set("stunned", false);
    assert!(!status.stunned());
    assert_eq!(
        status.known().stunned(),
        Some(false),
        "the game having said 'no' must stay distinguishable from silence"
    );

    // Never reported at all.
    assert!(
        !status.dead(),
        "the collapsing accessor still reads false, which is right for a \
         renderer: no icon is the correct display for both states"
    );
    assert_eq!(
        status.known().dead(),
        None,
        "but anything that ACTS must be able to tell 'the game says you are \
         alive' from 'the game has not said'. plan/12 section 5.2 calls the \
         second Unknown, and treating it as a confident answer is the failure \
         that rule exists to prevent."
    );
}

/// **Indicators SURVIVE a reconnect**, and the burst re-declares them anyway.
///
/// This test used to be `a_reconnect_makes_every_indicator_unknown_rather_
/// than_false`, and its doc said *"`invalidate_for_reconnect` clears the
/// indicators, and nothing re-teaches them until the player acts"*. Both
/// halves are false, and the second is measurably so.
///
/// > **AUTHOR, 2026-09-20:** *"time stops for 99.9% of things when you're
/// > offline"*
///
/// A character out of the world does not stop standing, and a stun does not
/// tick away while nobody is playing. MEASURED in two captures: the burst
/// declares **all ten** indicators explicitly, `visible="n"` included --
///
/// ```text
/// <indicator id="IconSTANDING" visible="y"/>
/// <indicator id="IconSTUNNED"  visible="n"/>
/// ```
///
/// -- so the server states the full truth about every one, unprompted. Keeping
/// them is correct, and anything that did change is corrected in the same
/// breath.
///
/// The three-state distinction this test was written to protect still matters
/// and still has a test: `a_cleared_status_is_unknown_rather_than_reported_
/// false` in `reconnect_invalidation.rs` asserts it against `StatusInfo::clear`
/// directly, which is the path that does drop indicators.
#[test]
fn a_reconnect_keeps_the_indicators_the_burst_will_re_declare() {
    let mut state = cena_model::GameState::default();
    state.status.set("stunned", true);
    state.status.set("dead", false);

    state.invalidate_for_reconnect();

    assert_eq!(
        state.status.known().stunned(),
        Some(true),
        "a stun does not tick away while the character is out of the world"
    );
    assert_eq!(
        state.status.known().dead(),
        Some(false),
        "and a reported-false indicator stays reported-false, not Unknown"
    );
}
