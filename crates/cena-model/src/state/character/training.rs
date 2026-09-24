//! Training points: the four `expr` labels, moved down under Rule 4.1.
//!
//! # How these were found, and the lesson the author gave with them
//!
//! Censusing the `<dialogData>` labels of one live session found five named
//! labels in `expr` and the model reading one. But the census also misled me,
//! and the author said why:
//!
//! > **AUTHOR, 2026-09-21:** *"you only see the one because my character is not
//! > doing regular experience so he doesn't gain tps, he is doing ascension
//! > experience which doesn't report there"*. And: *"this is also why I expected
//! > you to sleuth through the lich parser, xmlparser, and all of it's related
//! > things to see what else is missing rather than just looking at what my
//! > character only sees"*.
//!
//! MEASURED across two characters, which is what settled it:
//!
//! | character | experience | `PTPs` labels |
//! |---|---|---|
//! | Nisugi, one session | ascension | **1** |
//! | Nerten, three logs | regular | **10,598** |
//!
//! A one-character log shows what that character does. These labels arrive
//! whenever the points CHANGE, and an ascension character's never do.
//!
//! **Lich does not read them at all** -- `grep -rniE 'ptp|mtp'` over
//! `reference/lich-5/lib` finds nothing -- so the wire is the only authority
//! and its own tooltips are the documentation.

use super::Experience;

// The fields these fill live on `Experience` (`character.rs`), beside the rest
// of what `expr` teaches. Their rationale is the module doc above: the NUMBER
// is kept rather than the label, unlike `Experience::level`, whose whole
// `Level 100` string is kept because a display wants it. A point count is
// arithmetic -- "can I afford this rank" -- and every consumer would otherwise
// re-parse it.

/// Read one `expr` label into [`Experience`], if it is a training-point one.
///
/// Returns whether it was, so the caller's `match` can fall through.
pub(super) fn read_label(exp: &mut Experience, id: &str, value: &str) -> bool {
    let Some(number) = leading_number(value) else {
        // A label whose value is not a number is still one of OURS -- claiming
        // it keeps the caller from trying other arms -- but teaches nothing.
        // `None` rather than `0`, because zero points is a real reading.
        return matches!(id, "PTPs" | "MTPs" | "p2m" | "m2p");
    };
    match id {
        "PTPs" => exp.physical_training = Some(number),
        "MTPs" => exp.mental_training = Some(number),
        "p2m" => exp.physical_converted = Some(number),
        "m2p" => exp.mental_converted = Some(number),
        _ => return false,
    }
    true
}

/// The digits at the start of a label's value, e.g. `3673 PTPs` -> `3673`.
///
/// Commas are stripped, as `currency.rs` does for the same reason: the wire
/// spells these bare at the sizes MEASURED (`3673 PTPs`), and a format that
/// grows a separator later must not silently read as a smaller number.
///
/// `None` for a value that does not start with a digit -- never `0`, because
/// zero training points is a real reading and "not a number" is not.
///
/// **The first whitespace-separated token, read whole.** This used to take
/// the leading digits and stop, so `12a3 PTPs` read as `12` -- a partial
/// parse of something the wire never sent. The token now goes through the
/// shared strict reader (`state/numbers.rs`), which refuses it.
fn leading_number(value: &str) -> Option<u32> {
    crate::state::numbers::grouped(value.split_whitespace().next()?)
}
