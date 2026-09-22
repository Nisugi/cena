//! Training points: the four `expr` labels the model was not reading.
//!
//! Found by censusing the `<dialogData>` labels at the author's request
//! (2026-09-21). MEASURED over one live session:
//!
//! ```text
//! 14,366 <label> in all, and every one of them in a dialog the model
//! already read -- except four:
//!
//!   expr: yourLvl (read), PTPs, MTPs, p2m, m2p (none read)
//! ```
//!
//! **Lich does not read these at all** -- `grep -rniE 'ptp|mtp'` over
//! `reference/lich-5/lib` finds nothing -- so the wire is the only authority
//! and its own tooltips are the documentation.
//!
//! The dialog below is verbatim from that session.

use cena_model::GameState;
use cena_protocol::Parser;

/// The real login-burst `expr` dialog that carries the points.
const POINTS: &str = concat!(
    r#"<dialogData id='expr'><label id='PTPs' value='3673 PTPs' justify='4' top='0' "#,
    r#"left='20' anchor_top='mindState' width='80' height='20' "#,
    r#"tooltip='Physical Training Points'/><label id='MTPs' value='0 MTPs' justify='4' "#,
    r#"top='0' left='0' anchor_top='mindState' anchor_left='PTPs' height='20' width='80' "#,
    r#"tooltip='Mental Training Points'/><label id='p2m' value='2866 P2M' justify='4' "#,
    r#"top='0' left='20' anchor_top='PTPs' height='20' width='80' "#,
    r#"tooltip='Physical tps that have been converted to Mental tps'/>"#,
    r#"<label id='m2p' value='0 M2P' justify='4' top='0' left='0' anchor_top='MTPs' "#,
    r#"anchor_left='p2m' height='20' width='80' "#,
    r#"tooltip='Mental tps that have been converted to Physical tps'/></dialogData>"#,
    "\n",
);

fn fed(wire: &str) -> GameState {
    let mut state = GameState::default();
    for frame in Parser::new().push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
    state
}

#[test]
fn the_real_dialog_teaches_all_four() {
    let exp = &fed(POINTS).character.experience;
    assert_eq!(exp.physical_training, Some(3673));
    assert_eq!(
        exp.mental_training,
        Some(0),
        "zero points is a real reading"
    );
    assert_eq!(exp.physical_converted, Some(2866));
    assert_eq!(exp.mental_converted, Some(0));
}

#[test]
fn not_told_is_not_zero() {
    // §5.2. A character whose `expr` dialog has not arrived has no reading,
    // and `Some(0)` above proves the distinction is not academic: mental
    // training really is zero for this character.
    let exp = &GameState::default().character.experience;
    assert_eq!(exp.physical_training, None);
    assert_eq!(exp.mental_training, None);
}

#[test]
fn a_routine_expr_refresh_does_not_erase_them() {
    // **A ONE-CHARACTER CENSUS GOT THIS BACKWARDS, and the author said so.**
    //
    // > **AUTHOR, 2026-09-21:** *"you only see the one because my character is
    // > not doing regular experience so he doesn't gain tps, he is doing
    // > ascension experience which doesn't report there"*. And: *"this is also
    // > why I expected you to sleuth through the lich parser, xmlparser, and
    // > all of it's related things ... rather than just looking at what my
    // > character only sees"*.
    //
    // MEASURED, both characters, same rule:
    //
    // | character | experience | `PTPs` labels |
    // |---|---|---|
    // | Nisugi, one session | ascension | **1** |
    // | Nerten, three logs | regular | **10,598** |
    //
    // I had read 1-of-158 as "the points arrive in the login burst". They
    // arrive whenever they CHANGE, and a character earning regular experience
    // changes them constantly. So this test is far more load-bearing than that
    // reading suggested: a dialog that replaced the whole struct would wipe the
    // points on every mind-state tick, ten thousand times a session.
    let refresh = concat!(
        r#"<dialogData id='expr'><progressBar id='mindState' value='62' text='muddled'/>"#,
        r#"<label id='yourLvl' value='Level 100'/></dialogData>"#,
        "\n",
    );
    let state = fed(&format!("{POINTS}{refresh}"));
    let exp = &state.character.experience;
    assert_eq!(exp.physical_training, Some(3673), "the refresh erased them");
    assert_eq!(
        exp.level.as_deref(),
        Some("Level 100"),
        "guard: it was read"
    );
}

#[test]
fn the_number_is_taken_and_the_unit_is_not() {
    // `3673 PTPs` -- the unit repeats the id, so only the digits are the fact.
    // Unlike `level`, whose whole `Level 100` string is kept because a display
    // wants it: a point count is arithmetic.
    let state = fed("<dialogData id='expr'><label id='PTPs' value='12,345 PTPs'/></dialogData>\n");
    assert_eq!(
        state.character.experience.physical_training,
        Some(12_345),
        "a separator must not read as a smaller number"
    );
}

#[test]
fn a_label_that_is_not_a_number_is_not_read_as_zero() {
    let state = fed("<dialogData id='expr'><label id='PTPs' value='unknown'/></dialogData>\n");
    assert_eq!(state.character.experience.physical_training, None);
}

#[test]
fn these_labels_only_count_inside_the_expr_dialog() {
    // The dialog is load-bearing: `payload.rs:182` records that a label's
    // enclosing dialog is what tells `yourLvl` from a map legend. A `PTPs`
    // label in another dialog is another dialog's business.
    let state =
        fed("<dialogData id='minivitals'><label id='PTPs' value='999 PTPs'/></dialogData>\n");
    assert_eq!(state.character.experience.physical_training, None);
}

#[test]
fn a_reconnect_keeps_them_because_the_character_still_has_them() {
    // Training points are the character's, not the world's -- the same side of
    // `reconnect.rs`'s line as skills and experience. The burst re-sends them
    // anyway, but losing them would make a reconnect look like a respend.
    let mut state = fed(POINTS);
    state.invalidate_for_reconnect();
    assert_eq!(state.character.experience.physical_training, Some(3673));
}

#[test]
fn spending_points_is_a_new_reading_not_a_merge() {
    // The author's correction says these arrive whenever they CHANGE, so the
    // interesting case is a second dialog with different numbers -- a guild
    // visit, or a level. The new reading must win outright.
    let spent = concat!(
        r#"<dialogData id='expr'><label id='PTPs' value='73 PTPs'/>"#,
        r#"<label id='p2m' value='2900 P2M'/></dialogData>"#,
        "\n",
    );
    let state = fed(&format!("{POINTS}{spent}"));
    let exp = &state.character.experience;
    assert_eq!(exp.physical_training, Some(73), "3673 survived a respend");
    assert_eq!(exp.physical_converted, Some(2900));
    // And a label the second dialog did NOT carry keeps its last reading.
    assert_eq!(exp.mental_training, Some(0));
}
