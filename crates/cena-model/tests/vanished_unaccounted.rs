//! The author's hiding rule, all three conditions at last.
//!
//! > **AUTHOR, 2026-09-20:** *"but gone just means not in the room, doesn't
//! > mean hid. We have creature arrival and leaving messaging though, which
//! > would get tagged somewhere along the way and get pushed to the
//! > creature."*
//!
//! A two-condition version -- gone and not dead -- was written, found wrong,
//! and deleted, because a creature that simply walked out satisfies both.
//!
//! The third condition is read off the **markup**, and that was the author's
//! second correction:
//!
//! > **AUTHOR, 2026-09-21:** *"the room would be giving a link with the id"*.
//!
//! The wire line below is the real shape, from `GST-Nisugi/2026-08-21`: the
//! creature's `exist` id appears twice -- the pronoun is a link too -- and the
//! direction is a bare `<d>` command link.

use cena_model::GameState;
use cena_protocol::Parser;

/// A room holding one ghast: the full idiom, because a creature joins the
/// roster only when a `<crtrStatus>` vouches for a bolded `room objs` link.
const GHAST_IN_ROOM: &str = concat!(
    "<component id='room objs'>You also see ",
    "<pushBold/><a exist=\"8365650\" noun=\"ghast\">a cadaverous tatterdemalion ghast</a><popBold/>.",
    "<crtrStatus exist=\"8365650\" inferior=\"1\"/></component>\n",
);

/// The departure, verbatim from the wire.
const GHAST_FLEES: &str = concat!(
    "<pushBold/>A <a exist=\"8365650\" noun=\"ghast\">cadaverous tatterdemalion ghast</a>",
    "<popBold/> leans back on <pushBold/><a exist=\"8365650\" noun=\"ghast\">his</a>",
    "<popBold/> haunches and bounds <d>southwest</d>.\n",
);

/// The same room, emptied. A `room objs` refresh replaces the roster.
const ROOM_EMPTY: &str = "<component id='room objs'></component>\n";

const PROMPT: &str = "<prompt time=\"100\">&gt;</prompt>\n";

fn fed(wire: &str) -> GameState {
    let mut state = GameState::default();
    for frame in Parser::new().push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
    state
}

#[test]
fn a_real_departure_names_the_creature_and_the_direction() {
    let wire = format!("{GHAST_IN_ROOM}{GHAST_FLEES}{PROMPT}");
    let state = fed(&wire);
    assert_eq!(
        state.creatures().fled(8_365_650),
        Some(Some("southwest")),
        "both facts came off the markup"
    );
}

#[test]
fn a_creature_seen_to_leave_is_accounted_for() {
    let wire = format!("{GHAST_IN_ROOM}{GHAST_FLEES}{PROMPT}{ROOM_EMPTY}{PROMPT}");
    let state = fed(&wire);
    assert_eq!(
        state.creatures().vanished_unaccounted().collect::<Vec<_>>(),
        Vec::<i64>::new(),
        "it was seen to leave, so nothing is unaccounted for"
    );
}

#[test]
fn a_creature_that_simply_vanished_is_not_accounted_for() {
    // The same room, emptied, with NO departure line. This is the case the
    // two-condition version could not tell from the one above.
    let wire = format!("{GHAST_IN_ROOM}{PROMPT}{ROOM_EMPTY}{PROMPT}");
    let state = fed(&wire);

    assert_eq!(
        state.creatures().fled(8_365_650),
        None,
        "nothing said it left"
    );
    assert_eq!(
        state.creatures().vanished_unaccounted().collect::<Vec<_>>(),
        vec![8_365_650],
        "gone, not dead, and not seen to leave"
    );
}

#[test]
fn a_departure_by_a_creature_this_room_does_not_know_is_ignored() {
    // A line about someone else's fight is not this room's news. The id is not
    // in the registry, so nothing is recorded.
    let elsewhere = GHAST_FLEES.replace("8365650", "999");
    let wire = format!("{GHAST_IN_ROOM}{elsewhere}{PROMPT}{ROOM_EMPTY}{PROMPT}");
    let state = fed(&wire);

    assert_eq!(state.creatures().fled(999), None);
    assert_eq!(
        state.creatures().vanished_unaccounted().collect::<Vec<_>>(),
        vec![8_365_650],
        "the ghast is still unaccounted for"
    );
}

#[test]
fn a_clickable_direction_with_no_creature_is_not_a_departure() {
    // A bare `<d>` appears in room descriptions and the compass. Requiring a
    // BOLDED creature link in the same line is what keeps those out.
    let wire = format!("{GHAST_IN_ROOM}A path leads <d>southwest</d> toward the bridge.\n{PROMPT}");
    assert_eq!(fed(&wire).creatures().fled(8_365_650), None);
}

#[test]
fn a_creature_line_with_no_direction_is_not_a_departure() {
    // It swung at something and stayed. Both halves are required.
    let wire = format!(
        "{GHAST_IN_ROOM}<pushBold/>A <a exist=\"8365650\" noun=\"ghast\">cadaverous \
         tatterdemalion ghast</a><popBold/> snarls.\n{PROMPT}"
    );
    assert_eq!(fed(&wire).creatures().fled(8_365_650), None);
}

#[test]
fn a_d_link_that_is_not_a_direction_is_not_one() {
    // A bare `<d>` carries `go bridge`, `climb wall` and worse. Only the
    // eleven directions count (`creature.rb:858`, which includes `out`).
    let wire = format!(
        "{GHAST_IN_ROOM}<pushBold/>A <a exist=\"8365650\" noun=\"ghast\">cadaverous \
         tatterdemalion ghast</a><popBold/> tries to <d>climb wall</d>.\n{PROMPT}"
    );
    assert_eq!(fed(&wire).creatures().fled(8_365_650), None);
}

#[test]
fn out_is_a_direction_even_though_it_is_not_a_compass_point() {
    let leaves = GHAST_FLEES.replace("<d>southwest</d>", "<d>out</d>");
    let wire = format!("{GHAST_IN_ROOM}{leaves}{PROMPT}");
    assert_eq!(
        fed(&wire).creatures().fled(8_365_650),
        Some(Some("out")),
        "its absence from Lich's list once broke every such line"
    );
}

#[test]
fn a_creature_still_in_the_room_is_not_vanished_at_all() {
    // The guard on the whole inference: `departed` is about the roster, and a
    // creature still on it has not gone anywhere.
    let wire = format!("{GHAST_IN_ROOM}{PROMPT}");
    assert!(
        fed(&wire)
            .creatures()
            .vanished_unaccounted()
            .next()
            .is_none()
    );
}

#[test]
fn two_creatures_are_tracked_apart() {
    // One leaves, one vanishes. A single flag would have answered for both.
    let wire = concat!(
        "<component id='room objs'>You also see ",
        "<pushBold/><a exist=\"8365650\" noun=\"ghast\">a cadaverous tatterdemalion ghast</a>",
        "<popBold/> and <pushBold/><a exist=\"8365619\" noun=\"banshee\">a flickering \
         mist-wreathed banshee</a><popBold/>.",
        "<crtrStatus exist=\"8365650\" inferior=\"1\"/>",
        "<crtrStatus exist=\"8365619\" inferior=\"1\"/></component>\n",
        "<pushBold/>A <a exist=\"8365650\" noun=\"ghast\">cadaverous tatterdemalion ghast</a>",
        "<popBold/> leans back on <pushBold/><a exist=\"8365650\" noun=\"ghast\">his</a>",
        "<popBold/> haunches and bounds <d>southwest</d>.\n",
        "<prompt time=\"100\">&gt;</prompt>\n",
        "<component id='room objs'></component>\n",
        "<prompt time=\"101\">&gt;</prompt>\n",
    );
    let state = fed(wire);

    assert_eq!(state.creatures().fled(8_365_650), Some(Some("southwest")));
    assert_eq!(state.creatures().fled(8_365_619), None);
    assert_eq!(
        state.creatures().vanished_unaccounted().collect::<Vec<_>>(),
        vec![8_365_619],
        "only the one nothing explained"
    );
}

#[test]
fn an_unbolded_creature_link_is_not_a_departure() {
    // **MEASURED by mutation**: dropping the bold requirement left every test
    // green, because no test had an UNBOLDED `exist` link beside a direction.
    // That combination is real -- an inventory item, a room object or a player
    // is an unbolded `<a exist>` -- and bold is the wire's own mark for a
    // creature (`state/room.rs:207`).
    let wire = format!(
        "{GHAST_IN_ROOM}A <a exist=\"8365650\" noun=\"cloak\">dark cloak</a> \
         lies here, dropped <d>southwest</d> of the arch.\n{PROMPT}"
    );
    assert_eq!(fed(&wire).creatures().fled(8_365_650), None);
}
