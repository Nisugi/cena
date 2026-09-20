//! Rule 2.2a: the model drops nothing the parser preserved.
//!
//! > **The author, 2026-09-20:** *"model drops nothing without my explicit
//! > permission."*
//!
//! `cena-protocol/tests/every_tag_is_observable.rs` proves every tag reaches
//! a consumer as a frame. It says nothing about what happens next, and the
//! gap between those two statements is where vitals lost `current`/`max` for
//! a whole milestone: the tag was observable, its contents were not
//! preserved, and every vitals test asserted on the percentage because the
//! percentage was all there was.
//!
//! # What this file checks, and what it cannot
//!
//! A *partially consumed* payload is the dangerous shape. When `GameState`
//! matches a `Frame` arm and reads some of its fields, the unread ones are
//! invisible: no test fails, and a later consumer has no way to learn the
//! datum was ever on the wire. A frame with **no** arm is a different and
//! safer case -- `state.rs`'s `_ => {}` drops nothing, because the actor has
//! already broadcast the frame to observers.
//!
//! So each test below feeds real wire text and asserts a specific field
//! survives into `GameState`. That is narrow by construction: it catches
//! regressions on the fields we have audited, not fields nobody has thought
//! about yet.
//!
//! # Two ledgers, and why they are separate
//!
//! [`KNOWN_DROPS`] holds fields the **author** decided to drop.
//! [`UNDECIDED`] holds fields the model does not keep and nobody has ruled
//! on yet. Writing a justification into the first list on the author's
//! behalf is the exact move Rule 2.2a forbids, so an unasked field goes in
//! the second and is owed a question.

use cena_model::{GameState, VitalsExt};
use cena_protocol::Parser;

fn state_after(lines: &[&str]) -> GameState {
    let mut parser = Parser::new();
    let mut state = GameState::default();
    for line in lines {
        for frame in parser.parse_line(line) {
            state.apply(&frame);
        }
    }
    state
}

/// Payload fields the model does not keep, and **the author's decision for
/// each**.
///
/// Rule 2.2a: a drop is the author's call, not the implementer's. A row here
/// records a decision that was actually asked for and given; a field nobody
/// has asked about is NOT listed, it is [`UNDECIDED`] -- and the difference
/// between the two lists is the whole point of the rule.
///
/// A field that is *routed elsewhere* rather than stored is not a drop and
/// belongs in neither list: `Style::preset` and `Style::mono` reach
/// `cena-ui` on the frame itself, which is where `payload.rs:71` says they
/// belong.
const KNOWN_DROPS: &[(&str, &str, &str)] = &[];

/// Payload fields the model does not keep and **the author has not ruled
/// on**. Each is owed a question, not a justification written on his behalf.
///
/// This list existing is not a rule violation; leaving it unasked is.
const UNDECIDED: &[(&str, &str, &str)] = &[
    (
        "Link",
        "coord",
        "Map coordinates on a movement link, e.g. coord=\"2524,1864\". VERIFIED unreachable: `grep -rn coord crates/ --include=*.rs` finds only payload.rs, so no consumer in any crate can see it. Asked 2026-09-20.",
    ),
    (
        "DialogWidgets",
        "widgets",
        "The whole payload is unconsumed -- no Frame::DialogWidgets arm -- so the actor still broadcasts it and nothing is lost to observers. Whether GameState should hold dialog widgets is the open question. Asked 2026-09-20.",
    ),
];

#[test]
fn every_drop_is_either_the_authors_decision_or_an_open_question() {
    // Both lists want a real reason at the place a reader would look; the
    // difference is only who decided.
    for (payload, field, why) in KNOWN_DROPS.iter().chain(UNDECIDED) {
        assert!(
            why.len() > 80,
            "{payload}::{field} is listed with no real justification. \n             Rule 2.2a wants the reason, not a label."
        );
    }
    // A field cannot be both settled and open.
    for (payload, field, _) in KNOWN_DROPS {
        assert!(
            !UNDECIDED.iter().any(|(p, f, _)| p == payload && f == field),
            "{payload}::{field} is in both lists"
        );
    }
}

/// The defect that produced Rule 2.2a, pinned.
mod progress_bar {
    use super::*;

    const HEALTH: &str = "<dialogData id='minivitals'><progressBar id='health' value='95' \
         text='health 213/223'/></dialogData>";

    #[test]
    fn the_amount_survives_not_just_the_percent() {
        let state = state_after(&[HEALTH]);
        let health = state.vitals.health().expect("health");
        assert_eq!(
            health.amount(),
            Some((213, 223)),
            "the wire spelled out 213/223 and cena-protocol parsed it into \
             payload::Amount. A model that keeps only `percent` reports 95 \
             and cannot answer 'how many points of healing do I need'."
        );
    }

    #[test]
    fn the_games_own_percent_survives_too() {
        // Both, not either: 213/223 is 95.5%, so a consumer cannot recover
        // the game's own rounding from the pair.
        assert_eq!(
            state_after(&[HEALTH]).vitals.health().map(|v| v.percent),
            Some(95)
        );
    }

    #[test]
    fn a_cooldowns_time_remaining_survives() {
        // 154,313 progress bars carry `time=` (measured, payload.rs:197).
        // A Heal or Hunt behavior cannot tell when a cooldown expired
        // without it.
        let state = state_after(&[
            "<prompt time='1700000000'>&gt;</prompt>",
            "<dialogData id='Cooldowns'><progressBar id='110572' value='100' \
             text='Multi-Strike' time='00:00:37'/></dialogData>",
        ]);
        let effect = state
            .effects
            .get("110572")
            .expect("the cooldown reached the model");
        assert_eq!(
            effect.ends_at,
            Some(1_700_000_037),
            "time= must survive as an absolute end, or a behavior polls blind"
        );
    }
}

/// Text runs: the markup that was open is structure, and structure is the
/// §3a bargain the whole parser exists to keep.
mod text_runs {
    use super::*;

    #[test]
    fn bold_depth_survives_into_the_room_model() {
        // Bold is what distinguishes a creature from scenery, which is why
        // `payload::Style` calls it semantic rather than a font instruction.
        // If it were dropped, `room objs` could not tell them apart.
        let state = state_after(&[
            "<component id='room objs'>You also see <pushBold/><a exist='101' \
             noun='lizard'>a cave lizard</a><popBold/> and a rusty lamppost.</component>",
        ]);
        let creatures: Vec<&str> = state
            .room
            .creatures
            .iter()
            .map(|c| c.text.as_str())
            .collect();
        assert_eq!(
            creatures,
            ["a cave lizard"],
            "the bolded link is a creature and the unbolded prose is not; \
             dropping bold_depth collapses that distinction"
        );
    }

    #[test]
    fn a_links_exist_id_and_noun_both_survive() {
        let state = state_after(&[
            "<component id='room objs'>You also see <pushBold/><a exist='101' \
             noun='lizard'>a cave lizard</a><popBold/>.</component>",
        ]);
        let creature = state.room.creatures.first().expect("the creature");
        assert_eq!(
            (creature.id.as_str(), creature.noun.as_str()),
            ("101", "lizard"),
            "the id targets it and the noun names its kind; neither is \
             recoverable from the display text"
        );
    }
}

/// A gauge that states no pair says so, rather than inventing one.
///
/// Drop-nothing cuts both ways: fabricating a maximum the wire never sent is
/// as wrong as discarding one it did. `payload::Amount` refuses Vellum's
/// `(percentage, 100)` for this reason.
#[test]
fn a_label_only_bar_reports_no_amount_rather_than_a_fabricated_one() {
    let state = state_after(&[
        "<dialogData id='minivitals'><progressBar id='mindState' value='34' \
         text='clear'/></dialogData>",
    ]);
    let mind = state.vitals.vital("mindState").expect("the bar landed");
    assert_eq!((mind.percent, mind.amount()), (34, None));
}

/// An unmodelled frame is not a dropped frame: the actor broadcasts it.
/// This is the arm `state.rs` ends with, and it is the honest shape --
/// what Rule 2.2a forbids is reading a payload and keeping part of it.
#[test]
fn an_unconsumed_frame_leaves_the_model_unchanged_rather_than_erroring() {
    let before = GameState::default();
    let after = state_after(&["<flooberty zorp='3'/>"]);
    assert_eq!(
        after.unknown_tags.len(),
        1,
        "and the unknown tag itself IS recorded (Rule 2.2)"
    );
    assert_eq!(after.vitals, before.vitals);
}
