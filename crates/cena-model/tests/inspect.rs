//! The `appraise` and `measure` replies bigshot reads (`state/inspect.rs`).
//!
//! Every line here is SYNTHETIC, built from the patterns bigshot matches
//! (`reference/scripts/scripts/bigshot.lic:6617-6619`, `:5666`) and the
//! verdicts the other sacrifice scripts name
//! (`reference/lich_repo_mirror/lib/sacrifice_decide.lic:46-50`). No committed
//! fixture carries either reply.

use cena_model::state::inspect::{appraised_frail, is_appraise_size, measured_percent};

#[test]
fn the_size_line_is_bigshots_pattern() {
    assert!(is_appraise_size("The kobold is small in size."));
    assert!(is_appraise_size(
        "The massive troll king is huge in size, and looks mean."
    ));
    assert!(
        is_appraise_size("The a b is c is tiny in size"),
        "the last ` is ` before the word"
    );
    assert!(
        !is_appraise_size("  The kobold is small in size."),
        "anchored at the start"
    );
    assert!(
        !is_appraise_size("The kobold is very small in size."),
        "one word, `\\w+`"
    );
    assert!(
        !is_appraise_size("The  is small in size."),
        "a name of one or more characters"
    );
    assert!(
        !is_appraise_size("The kobold is  in size."),
        "a word of one or more characters"
    );
    assert!(
        !is_appraise_size("The kobold is small-ish in size."),
        "`-` is not `\\w`"
    );
    assert!(!is_appraise_size("You appraise the kobold."));
}

#[test]
fn frail_is_read_off_the_replys_last_line() {
    let reply = [
        "You carefully appraise the kobold.",
        "The kobold is small in size.",
        "You sense that its soul is enticingly frail.",
    ];
    assert_eq!(appraised_frail(reply), Some(true));

    let manipulable = [
        "The kobold is small in size.",
        "You sense that its soul is susceptible to manipulation.",
    ];
    assert_eq!(appraised_frail(manipulable), Some(false));

    // bigshot tests `.last`: frail on the size line with another line after
    // it is not read as frail.
    let earlier = [
        "The kobold is small in size, and enticingly frail.",
        "Its soul is stalwart and formidable.",
    ];
    assert_eq!(appraised_frail(earlier), Some(false));

    // A one-line reply: the size line is the last line.
    assert_eq!(
        appraised_frail(["The kobold is small in size, and enticingly frail."]),
        Some(true)
    );
}

#[test]
fn a_trailing_blank_line_is_not_the_last_line() {
    let reply = [
        "The kobold is small in size.",
        "You sense that its soul is enticingly frail.",
        "   ",
    ];
    assert_eq!(appraised_frail(reply), Some(true));
}

#[test]
fn no_size_line_is_no_appraisal() {
    assert_eq!(
        appraised_frail(["You sense that its soul is enticingly frail."]),
        None
    );
    assert_eq!(appraised_frail(std::iter::empty::<&str>()), None);
}

#[test]
fn measure_reads_the_percent_in_any_case() {
    assert_eq!(
        measured_percent(
            "You gaze intently at your staff and judge the briars to be about 100 percent."
        ),
        Some(100)
    );
    assert_eq!(measured_percent("TO BE ABOUT 42 PERCENT."), Some(42));
    assert_eq!(
        measured_percent("to be about 7 percent"),
        None,
        "bigshot's `\\.` is a literal full stop"
    );
    assert_eq!(
        measured_percent("to be about percent."),
        None,
        "`\\d+`: one digit or more"
    );
    assert_eq!(measured_percent("to be about 5x percent."), None);
    assert_eq!(
        measured_percent("to be about ten percent, or to be about 30 percent."),
        Some(30),
        "the first place the whole pattern completes"
    );
    assert_eq!(
        measured_percent("Now, why are you trying to measure that?"),
        None
    );
}
