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
