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

#[cfg(test)]
mod tests {
    use super::*;

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
