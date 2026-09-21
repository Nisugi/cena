//! A tag's inner text, the two ways a caller can want it.
//!
//! Split out of `dispatch.rs` under Rule 4.1 (`plan/05:352-353`) -- move code
//! down, do not raise the cap. That file reached 401 lines against its cap of
//! 400 when the drop-nothing fix landed, and these two free functions are the
//! part of it that is not dispatch: neither takes a `Parser`, neither emits a
//! frame, and both are about turning bytes between two tags into a string.

use crate::text;

/// Raw bytes between an open tag and its matching close, within one string.
///
/// **Still markup.** Callers that hand the result to the layer above want
/// [`inner_display_text`] instead; this one exists for the two callers that
/// parse the body themselves -- `prompt`, which decodes entities, and
/// `parse_runs`, which turns markup into [`crate::runs::Runs`].
pub(super) fn inner_text(tag: &str) -> String {
    let Some(open_end) = tag.find('>') else {
        return String::new();
    };
    let body = &tag[open_end + 1..];
    match body.rfind("</") {
        Some(close) => body[..close].to_owned(),
        None => body.to_owned(),
    }
}

/// A tag's inner text as the layer above should receive it: nested markup
/// flattened away, entities decoded, control characters stripped.
///
/// Rule 2.1 (`plan/05:270-274`) says nothing above this crate sees an unparsed
/// string, and five thin-tier fields broke it by handing back
/// [`inner_text`] verbatim -- `Spell.text`, `LeftHand.item`, `RightHand.item`,
/// `WorldEvent.text` and `ActiveEffect.text`. A `<worldEvent>` reading
/// `"A <b>great</b> storm &amp; flood!"` reached the caller with the markup
/// and the entity intact, which is the same violation this crate fixes for
/// `Component` with `Runs`.
///
/// The flatten is Vellum's (`src/parser/handlers.rs:718-737`). Unlike `Runs`
/// it discards the style spans rather than modelling them: these are
/// single-value fields and inventing a run vector for them would be structure
/// no caller asked for (`plan/05` §-1). The upgrade trigger, recorded rather
/// than pre-built: a consumer that needs the bold span inside a `<worldEvent>`
/// moves that field to `Runs`, exactly as `Component` already is.
///
/// A `<` with no `>` after it ends the scan, so an unterminated nested tag
/// truncates the text rather than leaking half a tag into it.
pub(super) fn inner_display_text(tag: &str) -> String {
    let raw = inner_text(tag);
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw.as_str();
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        let Some(gt) = rest[lt..].find('>') else {
            // An unterminated nested tag truncates the text rather than
            // leaking half a tag into it.
            rest = "";
            break;
        };
        rest = &rest[lt + gt + 1..];
    }
    out.push_str(rest);
    text::strip_control_chars(&text::decode_entities(&out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// A tag with a body, which is the shape `tagish()` in
    /// `tests/parser_never_panics.rs` cannot produce.
    ///
    /// **That is the gap PR-10 names.** That generator emits one tag at a
    /// time -- `<x/>`, `<x>`, `</x` -- and never a body BETWEEN two tags. So
    /// `inner_text`'s whole job, the `rfind("</")` arm, was never reached: the
    /// suite exercised only the early return and the `None => body` arm. A
    /// property that never reaches the code under test passes for the same
    /// reason an empty test does.
    fn tag_with_body() -> impl Strategy<Value = String> {
        let name = prop::sample::select(vec!["a", "d", "component", "preset", "b"]);
        let body = prop::sample::select(vec![
            "",
            "plain",
            "with <b>nested</b> markup",
            "entity &amp; more",
            "unterminated <b",
            "</early>",
            ">bare gt",
            "a > b",
        ]);
        let close = prop::sample::select(vec![true, false]);
        (name, body, close).prop_map(|(n, b, closed)| {
            if closed {
                format!("<{n}>{b}</{n}>")
            } else {
                format!("<{n}>{b}")
            }
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1024))]

        /// The result is always a **substring of the input**.
        ///
        /// `inner_text` slices; it never synthesises. This is what makes it
        /// safe for `parse_runs` to re-scan the result as markup -- a function
        /// that invented bytes could invent a tag.
        #[test]
        fn inner_text_only_ever_returns_a_slice_of_its_input(tag in tag_with_body()) {
            let inner = inner_text(&tag);
            prop_assert!(
                inner.is_empty() || tag.contains(inner.as_str()),
                "inner_text({tag:?}) = {inner:?}, which is not a substring"
            );
        }

        /// The body starts strictly **after** the opening tag closes.
        ///
        /// A first draft of this asserted the opening tag appears nowhere in
        /// the result, and proptest refuted it in eight cases with
        /// `<b>with <b>nested</b> markup</b>`: the body legitimately CONTAINS
        /// `<b>`, because the body is nested markup. `inner_text` returns
        /// markup by contract -- [`inner_display_text`] is the flattening one.
        ///
        /// So the real invariant is positional, not lexical: whatever is
        /// returned begins past the first `>`. That is what stops the opening
        /// tag being re-scanned as part of the body.
        #[test]
        fn inner_text_starts_after_the_opening_tag(tag in tag_with_body()) {
            let inner = inner_text(&tag);
            if let (Some(open_end), false) = (tag.find('>'), inner.is_empty())
                && let Some(at) = tag.find(inner.as_str())
            {
                prop_assert!(
                    at > open_end,
                    "body {inner:?} starts at {at}, inside the opening tag of {tag:?}"
                );
            }
        }

        /// Arbitrary input never panics and never grows.
        ///
        /// The length bound is the substring property's cheap corollary, and
        /// it holds for inputs `tag_with_body` cannot express.
        #[test]
        fn inner_text_never_grows_its_input(tag in ".*") {
            let inner = inner_text(&tag);
            prop_assert!(inner.len() <= tag.len());
        }

        /// `inner_display_text` yields **no markup at all**.
        ///
        /// Rule 2.1: nothing above this crate sees an unparsed string. The
        /// five thin-tier fields named in this module's doc broke exactly
        /// this, and a property is the right shape for it because the claim
        /// is universal rather than about five known cases.
        #[test]
        fn inner_display_text_never_leaks_a_tag(tag in tag_with_body()) {
            let shown = inner_display_text(&tag);
            prop_assert!(
                !shown.contains('<'),
                "markup reached the display text: {shown:?} from {tag:?}"
            );
        }
    }
}
