//! The `<dir value=>` tokens inside a `<compass>`.
//!
//! Split out of `dispatch.rs` under Rule 4.1 (`plan/05:352-353`) -- move code
//! down, do not raise the cap. That file sat at 728 of 800 and a test module
//! for this function would not fit; [`directions`] is the part of it that is
//! not dispatch, taking no `Parser` and emitting no frame.
//!
//! It is also **the function PR-10 named as untested**, and the move is what
//! makes testing it possible rather than a rewrite of a file that is nearly
//! full.

use crate::text;

/// The `<dir value=>` tokens inside a `<compass>`.
pub(super) fn directions(tag: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = tag;
    while let Some(at) = rest.find("<dir ") {
        rest = &rest[at..];
        let Some(end) = rest.find('>') else { break };
        if let Some(value) = text::attribute(&rest[..=end], "value") {
            out.push(value);
        }
        rest = &rest[end + 1..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// A compass with an arbitrary run of `<dir>` tokens, well-formed or not.
    ///
    /// **The existing suite could not reach this function at all.** MEASURED
    /// before writing this: `tests/parser_never_panics.rs` contains the string
    /// `<dir` **zero times**. `"compass"` is in its `tagish()` name list, but
    /// a bare `<compass/>` walks this loop zero times, so neither the
    /// `find('>')`-missing break nor the `attribute`-returns-`None` arm was
    /// ever executed. The tests passed over code they never ran -- the failure
    /// mode `CLAUDE.md` names, where the input never reaches the distinction.
    fn compass() -> impl Strategy<Value = String> {
        let token = prop::sample::select(vec![
            "<dir value=\"n\"/>",
            "<dir value='sw'/>",
            "<dir value=\"\"/>",
            // No `value` attribute, but WITH the trailing space the scan
            // requires -- `<dir/>` alone never matches `"<dir "` and so never
            // reaches the `attribute` call at all.
            "<dir foo=\"x\"/>",
            "<dir />",
            "<dir/>",           // unmatched by the scan; kept as a control
            "<dir value=\"n\"", // never closed: the `break` arm
            "<dir >",
            "<notdir value=\"n\"/>",
            "text between",
        ]);
        prop::collection::vec(token, 0..8)
            .prop_map(|parts| format!("<compass>{}</compass>", parts.concat()))
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1024))]

        /// **A direction is never invented.**
        ///
        /// The count can only fall below the number of `<dir ` tokens -- one
        /// without a `value`, or one that never closes, yields nothing. It can
        /// never rise. A compass that reported an exit the game did not send
        /// would route a behavior into a wall.
        #[test]
        fn directions_never_exceed_the_dir_tokens_present(tag in compass()) {
            let found = directions(&tag);
            let tokens = tag.matches("<dir ").count();
            prop_assert!(
                found.len() <= tokens,
                "{} directions from {} tokens in {tag:?}",
                found.len(),
                tokens
            );
        }

        /// Every direction returned was written in the input.
        ///
        /// Paired with the count bound above, this is the conservation claim:
        /// nothing invented, nothing transformed. Together they say the
        /// function only ever selects.
        #[test]
        fn every_direction_came_from_the_markup(tag in compass()) {
            for value in directions(&tag) {
                prop_assert!(
                    tag.contains(&value) || value.is_empty(),
                    "direction {value:?} appears nowhere in {tag:?}"
                );
            }
        }

        /// Arbitrary bytes never panic and never wedge the scan.
        ///
        /// The loop advances `rest` by hand, so a slice that failed to move
        /// would hang rather than crash -- which a panic-only test would not
        /// notice. Proptest's own timeout is what catches that here.
        #[test]
        fn arbitrary_input_terminates(tag in ".*") {
            let _ = directions(&tag);
        }
    }

    #[test]
    fn a_real_compass_reads_every_exit() {
        // Real wire shape, so the properties above are anchored to a case with
        // a known answer. A property that holds vacuously over inputs yielding
        // nothing would satisfy both bounds trivially.
        let tag = concat!(
            "<compass><dir value=\"n\"/><dir value=\"e\"/>",
            "<dir value=\"sw\"/><dir value=\"out\"/></compass>"
        );
        assert_eq!(directions(tag), ["n", "e", "sw", "out"]);
    }

    #[test]
    fn an_unterminated_dir_stops_the_scan_without_hanging() {
        // The `break` arm, which nothing reached before this module existed.
        assert_eq!(
            directions("<compass><dir value=\"n\"/><dir value=\"e\""),
            ["n"]
        );
    }

    #[test]
    fn a_dir_without_a_value_yields_nothing_rather_than_an_empty_exit() {
        // The `attribute` -> None arm. An empty string pushed here would be a
        // direction no behavior could walk.
        //
        // **`<dir/>` DOES NOT TEST THIS**, and a mutation proved it: the scan
        // looks for `"<dir "` WITH a trailing space, so `<dir/>` is never
        // matched and the loop runs zero times. A first draft asserted exactly
        // that input, and a mutant pushing `unwrap_or_default()` passed all
        // sixteen tests green. The assertion was right and the input never
        // reached the branch -- the third time that failure appeared today.
        //
        // `<dir foo="x"/>` is the form that reaches the `attribute` call and
        // gets `None` back.
        assert_eq!(
            directions("<compass><dir foo=\"x\"/></compass>"),
            Vec::<String>::new()
        );
        // The unmatched form, recorded so the distinction is not re-lost.
        assert_eq!(
            directions("<compass><dir/></compass>"),
            Vec::<String>::new()
        );
    }
}
