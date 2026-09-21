//! Why a move did not happen.
//!
//! Every wire line marked MEASURED below was taken from the 208 live Lich XML
//! logs; the rest are Lich's own `CAUSES` phrases (`move.rb:61-75`), which the
//! author confirmed as real game behavior (2026-09-21) after the corpus was
//! found unable to speak to nine of the fourteen:
//!
//! > *"we can take those 9 as being real. They weren't just added there by a
//! > hallucinating llm."*

use cena_model::state::movement::{
    MAX_REMEDIES, MAX_ROLLS, MoveFailure, classify, roll_cause, stand_failure_cause,
};

#[test]
fn the_three_causes_the_corpus_confirms() {
    // MEASURED after a movement command in the 208 live logs: Map 39,
    // Roundtime 95, Closed 1. These are the exact lines that were found.
    assert_eq!(classify("You can't go there."), MoveFailure::Map);
    assert_eq!(classify("Where are you trying to go?"), MoveFailure::Map);
    assert_eq!(
        classify("The cobblestone archway appears to be closed."),
        MoveFailure::Closed
    );
    assert_eq!(classify("...wait 4 seconds."), MoveFailure::Roundtime);
}

#[test]
fn the_nine_the_corpus_could_not_confirm_still_classify() {
    // Zero occurrences after a move in this corpus -- because 386 movement
    // commands in 4.4M lines is a character standing still, not because the
    // phrases are wrong. One per cause, from Lich's table.
    for (line, expected) in [
        (
            "You are in far too much agony to do that.",
            MoveFailure::Injured,
        ),
        ("You are overburdened.", MoveFailure::Encumbered),
        ("You can't do that while engaged!", MoveFailure::Engaged),
        ("You must remain hidden or invisible.", MoveFailure::Hidden),
        ("You realize your hands were empty.", MoveFailure::Hands),
        ("You should stand up first.", MoveFailure::Position),
        ("You attempt to swim across.", MoveFailure::Swim),
        ("You try to drag the body along.", MoveFailure::Drag),
        ("The guard says you may not pass.", MoveFailure::Denied),
    ] {
        assert_eq!(classify(line), expected, "{line:?}");
    }
}

#[test]
fn an_unrecognised_line_is_unknown_rather_than_a_guess() {
    // `None` would make every caller handle a case meaning the same thing:
    // the caller already knows the move failed, only the reason is open.
    assert_eq!(classify("A gust of wind blows past."), MoveFailure::Unknown);
    assert_eq!(classify(""), MoveFailure::Unknown);
}

mod the_false_positives_the_corpus_found {
    //! **These are why the classifier is not stateless.**
    //!
    //! MEASURED: scanning every line rather than only lines answering a move
    //! turned 39 real `Map` hits into 5,783 and 95 `Roundtime` into 12,918.
    //! The extras are not near-misses; they are ordinary prose.
    //!
    //! These tests assert what the table DOES say about such lines, so the
    //! hazard is recorded rather than implied. A consumer that asks about an
    //! arbitrary line gets a confident wrong answer, every time.
    use super::{MoveFailure, classify};

    #[test]
    fn an_amulet_description_reads_as_engaged() {
        // Real wire, and the reason `Engaged`'s pattern is dangerous: it is
        // the bare word `engaged`.
        let line = "The amulet is a crystal amulet which allows you to \
                    THINK LOCATION {target} while engaged in ESP.";
        assert_eq!(
            classify(line),
            MoveFailure::Engaged,
            "the table cannot tell this from a combat refusal -- only the \
             caller knowing a move is outstanding can"
        );
    }

    #[test]
    fn glancing_at_your_hands_reads_as_a_hands_failure() {
        // Real wire: "You glance down at your empty hands."
        assert_eq!(
            classify("You glance down at your empty hands."),
            MoveFailure::Hands
        );
    }
}

#[test]
fn roundtime_is_anchored_where_every_other_pattern_is_not() {
    // `move.rb:74` anchors this one with `^` alone. A line merely CONTAINING
    // "wait 4" is not the game telling us to wait -- and without the anchor
    // any sentence mentioning a wait would stall a router.
    assert_eq!(classify("...wait 4 seconds."), MoveFailure::Roundtime);
    assert_eq!(classify("wait 2 seconds."), MoveFailure::Roundtime);
    assert_eq!(
        classify("The innkeeper asks you to wait 5 minutes."),
        MoveFailure::Unknown,
        "an unanchored match here would stall on ordinary prose"
    );
}

#[test]
fn order_decides_when_a_line_satisfies_two_causes() {
    // First match wins (`move.rb:59-60`), and these are the real collisions.
    //
    // `Map` holds "too far away"; `Engaged` holds the bare "engaged". A line
    // with both must answer Engaged, because Engaged is listed first -- and
    // that matters: Map is the ONE cause that invalidates an exit, so the
    // wrong answer here deletes a good exit from the map.
    assert_eq!(
        classify("You are engaged and it is too far away."),
        MoveFailure::Engaged,
        "Engaged precedes Map in the table"
    );
}

#[test]
fn only_a_map_failure_invalidates_the_exit() {
    // `move`'s tri-state return (`move.rb:11-13`): false means the exit is
    // wrong and a caller MAY DROP IT; nil means blocked for now, keep it.
    //
    // Getting this backwards silently corrupts the map -- an exit deleted
    // because the character was in combat is a route never found again.
    assert!(MoveFailure::Map.invalidates_the_exit());
    for cause in MoveFailure::ALL {
        if cause != MoveFailure::Map {
            assert!(
                !cause.invalidates_the_exit(),
                "{cause} must not delete an exit"
            );
        }
    }
}

mod a_stand_that_keeps_failing {
    //! The line cannot say why, so the character must.
    //!
    //! *"You struggle, but fail to stand" is the same text for a heavy pack
    //! and for leg wounds* (`move.rb:105-107`). A port reading only the line
    //! answers `Position` for an overburdened character, and the caller then
    //! retries standing forever instead of dropping something.
    use super::{MoveFailure, stand_failure_cause};

    #[test]
    fn overburdened_outranks_wounds() {
        // Lich tests encumbrance first (`move.rb:110`), so a character who is
        // both gets Encumbered.
        assert_eq!(
            stand_failure_cause(true, true),
            MoveFailure::Encumbered,
            "encumbrance is tested before wounds"
        );
        assert_eq!(stand_failure_cause(true, false), MoveFailure::Encumbered);
    }

    #[test]
    fn limb_wounds_alone_are_an_injury() {
        assert_eq!(stand_failure_cause(false, true), MoveFailure::Injured);
    }

    #[test]
    fn neither_falls_back_to_position() {
        // Lich's default, and the one thing true whatever else is: the
        // character is down.
        assert_eq!(stand_failure_cause(false, false), MoveFailure::Position);
    }
}

mod the_skill_roll_branch {
    //! `roll_cause` (`move.rb:100-103`).
    use super::{MoveFailure, roll_cause};

    #[test]
    fn a_swim_drag_or_guard_line_keeps_its_own_name() {
        assert_eq!(roll_cause("You attempt to swim across."), MoveFailure::Swim);
        assert_eq!(roll_cause("You try to drag the body."), MoveFailure::Drag);
        assert_eq!(roll_cause("You may not pass."), MoveFailure::Denied);
    }

    #[test]
    fn anything_else_on_that_branch_is_a_climb() {
        // **The branch is only entered for a climb or a swim**, so this
        // fallback is information rather than a guess -- and it is the ONLY
        // way to reach Climb, which has no phrases of its own.
        assert_eq!(
            roll_cause("You slip and fall back down."),
            MoveFailure::Climb
        );
        assert_eq!(roll_cause(""), MoveFailure::Climb);
    }

    #[test]
    fn climb_is_unreachable_from_classify_alone() {
        // Guard: if a future edit gave Climb its own phrases, this fails and
        // whoever did it has to decide whether roll_cause still makes sense.
        use cena_model::state::movement::classify;
        assert_eq!(
            classify("You slip and fall back down."),
            MoveFailure::Unknown
        );
    }
}

#[test]
fn every_cause_round_trips_through_its_symbol() {
    for cause in MoveFailure::ALL {
        assert_eq!(MoveFailure::parse(cause.as_str()), Some(cause), "{cause}");
    }
    assert_eq!(MoveFailure::parse("nonsense"), None);
}

#[test]
fn the_table_is_complete_and_has_no_duplicates() {
    // Keyed off ALL so a variant added without a symbol is a failure rather
    // than a silent gap.
    assert_eq!(MoveFailure::ALL.len(), 14, "move.rb lists fourteen causes");
    let mut seen: Vec<&str> = MoveFailure::ALL.iter().map(|c| c.as_str()).collect();
    seen.sort_unstable();
    let before = seen.len();
    seen.dedup();
    assert_eq!(seen.len(), before, "two causes share a symbol");
}

#[test]
fn the_retry_budgets_differ_because_the_failures_differ() {
    // A remedy either works or does not (`move.rb:47-50`); a skill roll
    // legitimately fails several times first (`:52-56`). Collapsing these to
    // one number would either give up on a climb or hammer a locked door.
    // The relation, as a compile-time check rather than a runtime one.
    // `assert!(MAX_ROLLS > MAX_REMEDIES)` on two consts is decoration --
    // clippy says so ("this assertion has a constant value"), and it is the
    // `plan/05` §0 failure from the other side: a test that cannot fail.
    // A const item makes the same claim bind at compile time, where it
    // actually stops someone swapping the two numbers.
    const _: () = assert!(MAX_ROLLS > MAX_REMEDIES);

    assert_eq!(MAX_REMEDIES, 3);
    assert_eq!(MAX_ROLLS, 20);
}
