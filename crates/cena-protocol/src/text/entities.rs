//! XML entities and control characters: wire bytes to display text.
//!
//! Split out of `text.rs` under Rule 4.1 (`plan/05:352-353`) -- move code
//! down, do not raise the cap. `text.rs` lexes tags; this decodes what sits
//! between them, which is a separate concern and the one the layer above
//! actually reads.

/// Decode XML entities, exactly once.
///
/// Deliberately **not** idempotent: `&amp;lt;` decodes to `&lt;`, not to `<`.
/// Vellum pins this in a test (`src/parser/tests.rs:133`) and it is correct --
/// decoding to a fixed point would let `&amp;lt;script&amp;gt;` on the wire
/// become real markup one layer up. Unknown entities such as `&unknown;` are
/// left alone rather than dropped.
#[must_use]
pub fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp..];
        // A `;` more than 10 bytes away is not an entity, it is prose with an
        // ampersand in it -- "Stalking & Hiding" appears in the corpus.
        //
        // Scanned over BYTES rather than by slicing to a 12-byte window:
        // `&rest[..12]` panics when byte 12 lands inside a multi-byte
        // character. A proptest over arbitrary bytes found that immediately
        // (`&\u{9c}\u{98}...`), which is the whole reason plan/06 section 1.5
        // calls this generator non-negotiable. Entity names are ASCII, so a
        // byte scan cannot split one.
        let Some(semi) = after.bytes().take(12).position(|b| b == b';') else {
            out.push('&');
            rest = &after[1..];
            continue;
        };
        let entity = &after[1..semi];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some('\u{a0}'),
            _ => numeric_entity(entity),
        };
        match decoded {
            Some(c) => out.push(c),
            // Unknown entity: leave it verbatim. Never silently dropped.
            None => out.push_str(&after[..=semi]),
        }
        rest = &after[semi + 1..];
    }
    out.push_str(rest);
    out
}

/// `#65` -> `A`, `#x42` -> `B`. `None` if it is not a numeric entity.
fn numeric_entity(entity: &str) -> Option<char> {
    let digits = entity.strip_prefix('#')?;
    let code = match digits.strip_prefix(['x', 'X']) {
        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
        None => digits.parse().ok()?,
    };
    char::from_u32(code)
}

/// Drop C0 control characters (except tab) and DEL.
///
/// The wire occasionally leaks stray control bytes and rendering them corrupts
/// terminal output. Newlines never reach here: input is line-framed.
#[must_use]
pub fn strip_control_chars(text: &str) -> String {
    if !text
        .chars()
        .any(|c| (c.is_control() && c != '\t') || c == '\u{7f}')
    {
        return text.to_owned();
    }
    text.chars()
        .filter(|c| !((c.is_control() && *c != '\t') || *c == '\u{7f}'))
        .collect()
}

/// Decode an ATTRIBUTE value: entities once, then control characters out --
/// except `\n` and `\t`.
///
/// **Attribute values were decoded and never stripped.** Every text path
/// pairs `decode_entities` with [`strip_control_chars`] (`emit.rs`,
/// `inner.rs`, the prompt in `dispatch.rs`, found by review PR-9), and the
/// two attribute readers in `text.rs` did not. So
/// `<pushStream id='a&#27;]52;c;Zm9v&#7;'/>` put a real ESC and BEL into
/// `Text.stream`, and `<component id='room&#27;[2J objs'>` into
/// `Component.id` -- strings a frontend prints as a window title or log
/// line. Entities are decoded HERE, so the control characters do not exist
/// on the wire for anything upstream to have caught; the game socket is
/// plain TCP (`plan/10`), so they are modifiable in flight.
///
/// **Why not [`strip_control_chars`] itself:** it drops newlines, on the
/// grounds that text input is line-framed and none can arrive. An attribute
/// value is the one place a newline legitimately arrives, as an entity:
/// `<objective description=>` carries `&#10;` (`payload.rs`, `Objective`).
/// Tab survives for the same reason it survives there. Everything else in
/// C0/C1 plus DEL goes -- a principled rule, not a list of known-bad bytes.
#[must_use]
pub fn decode_attribute_value(raw: &str) -> String {
    let kept = |c: char| !(c.is_control() || c == '\u{7f}') || c == '\n' || c == '\t';
    let decoded = decode_entities(raw);
    if decoded.chars().all(kept) {
        return decoded;
    }
    decoded.chars().filter(|c| kept(*c)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribute_values_lose_escapes_but_keep_newlines_and_tabs() {
        // OSC-52 (clipboard write) and a clear-screen, both built from
        // entities so no raw control byte is on the wire.
        assert_eq!(
            decode_attribute_value("a&#27;]52;c;Zm9v&#7;"),
            "a]52;c;Zm9v"
        );
        assert_eq!(decode_attribute_value("room&#27;[2J objs"), "room[2J objs");
        assert_eq!(decode_attribute_value("x&#x9b;y&#127;z"), "xyz");
        // The legitimate cases: `<objective description=>` uses `&#10;`.
        assert_eq!(decode_attribute_value("one&#10;two"), "one\ntwo");
        assert_eq!(decode_attribute_value("a&#9;b"), "a\tb");
        assert_eq!(decode_attribute_value("plain"), "plain");
    }

    #[test]
    fn entities_decode_exactly_once() {
        assert_eq!(decode_entities("&gt;"), ">");
        assert_eq!(decode_entities("a &amp; b"), "a & b");
        // The non-idempotence that is deliberate: one pass, not a fixed point.
        assert_eq!(decode_entities("&amp;lt;"), "&lt;");
        assert_eq!(decode_entities("&#65;&#x42;"), "AB");
        // Unknown entity survives verbatim rather than vanishing.
        assert_eq!(decode_entities("&unknown;"), "&unknown;");
        // Prose ampersand, from the corpus. Not an entity, not mangled.
        assert_eq!(decode_entities("Stalking & Hiding"), "Stalking & Hiding");
        // A lone trailing ampersand must not panic or eat the string.
        assert_eq!(decode_entities("ends with &"), "ends with &");
    }

    #[test]
    fn control_characters_are_stripped_but_tabs_survive() {
        assert_eq!(strip_control_chars("a\u{1}b\u{7f}c"), "abc");
        assert_eq!(strip_control_chars("col\tcol"), "col\tcol");
    }
}
