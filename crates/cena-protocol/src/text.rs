//! Tag lexing: finding markup, naming it, and reading its attributes.
//!
//! Ported from `reference/VellumFE/src/parser/text.rs`, minus everything that
//! resolves colour -- that is presentation and Rule 2.1 (`plan/05:270-274`)
//! keeps it above this crate.
//!
//! Entity decoding and control-character stripping live in `entities`, split
//! out under Rule 4.1 (`plan/05:352-353`) when this file went over its cap:
//! move code down, do not raise the cap. They are re-exported here because
//! every caller reaches them through `text::`.

mod entities;
#[cfg(test)]
mod termination;

pub use entities::{decode_attribute_value, decode_entities, strip_control_chars};

use crate::frame::{Link, LinkKind};

/// First `<` in `s` that plausibly starts real markup.
///
/// A `<` preceded by `$` is the game's own broken escaping: ability HELP text
/// ships `$<a href=$Q...$>...$</a$>`. Interpreting such a tag can corrupt
/// parser state -- imagine a mangled `$<pushStream>` -- so mangled markup
/// renders as literal text instead. Ported from
/// `reference/VellumFE/src/parser/text.rs:143-154`; the corpus survey found 0
/// occurrences in a 968K-line sample, but the failure mode when it does arrive
/// is a link colour bleeding over everything after it.
#[must_use]
pub fn find_tag_start(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut from = 0;
    while let Some(p) = s[from..].find('<').map(|p| p + from) {
        if p > 0 && bytes[p - 1] == b'$' {
            from = p + 1;
            continue;
        }
        if !opens_markup(&bytes[p + 1..]) {
            from = p + 1;
            continue;
        }
        return Some(p);
    }
    None
}

/// Does what follows a `<` actually begin markup?
///
/// Without this guard any `<` started a tag that ran to the next `>`, so the
/// prose `"5 < 10 and 10 > 5"` became `Text("5 ")`, `UnknownTag { name: "",
/// raw: "< 10 and 10 >" }`, `Text(" 5")` -- and a consumer rendering text and
/// logging unknown tags, which is the documented intent, showed the player
/// `"5   5"`. It also produced `UnknownTag { name: "" }` for `<>` and `</>`,
/// an element name that cannot exist.
///
/// An XML name starts with a letter, `_` or `:`; `/` opens a close tag and
/// `!` a comment or declaration, both of which the caller handles. Everything
/// else after a `<` is prose and stays prose.
///
/// This costs nothing the corpus relies on: `grep -h -coE '[a-z] < '` over 272
/// stratified files returns 0, so the shape is absent from live traffic. The
/// live cousin, `<#>` in Lich error echoes, still reaches the user correctly
/// -- `#` is not a name-start char, so the whole thing stays text rather than
/// becoming `UnknownTag { name: "#" }`.
fn opens_markup(after: &[u8]) -> bool {
    match after.first() {
        Some(b'/' | b'!') => true,
        Some(c) => c.is_ascii_alphabetic() || *c == b'_' || *c == b':',
        None => false,
    }
}

/// Element name of a raw tag: `<pushStream id=..>` -> `pushStream`,
/// `</compDef>` -> `compDef`. Empty when the input is not tag-shaped.
///
/// The terminator set includes `/` and whitespace, which matters: the corpus
/// carries `<style id="roomName" />` with a space before `/>`, so a scanner
/// assuming `/` abuts `>` mis-reads the name.
#[must_use]
pub fn tag_name(tag: &str) -> &str {
    let body = tag
        .strip_prefix("</")
        .or_else(|| tag.strip_prefix('<'))
        .unwrap_or(tag);
    let end = body
        .find(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
        .unwrap_or(body.len());
    &body[..end]
}

/// Is this a closing tag?
///
/// Tolerates the game's `$`-mangled form `</a$>`
/// (`reference/VellumFE/src/parser/text.rs:241`).
#[must_use]
pub fn is_close_tag(tag: &str) -> bool {
    tag.starts_with("</")
}

/// Value of `name=` in a raw tag, accepting either quoting.
///
/// The wire mixes `id='room'` and `id="room"` freely, sometimes within one
/// tag -- `vitals.xml` has `value="guarded" cmd='...'` on one `<dropDownBox>`.
#[must_use]
pub fn attribute(tag: &str, name: &str) -> Option<String> {
    // **An empty name does not advance, and the loop never ends.**
    //
    // `"abc".find("")` is `Some(0)` -- for every haystack, at every offset --
    // so `from = at + name.len()` leaves `from` where it was, index 0 is
    // preceded by nothing (`preceded_ok`), the `=` test fails, `continue`,
    // and the same zero-width match is found again. Not a slow path: a hang,
    // in a `pub fn` of a `pub mod`, reachable by any caller that computes an
    // attribute name and gets an empty string.
    //
    // `None` is the honest answer rather than a panic. There is no attribute
    // with no name, so "not found" is exactly true, and a parser whose stated
    // contract is that it never panics (`plan/06` 1.5, "Non-negotiable") must
    // not gain a panic at a leaf. Found by review (PR-11).
    //
    // (The walker below no longer searches for the name at all, so an empty
    // name cannot hang it; the check stays, as the contract it states.)
    if name.is_empty() {
        return None;
    }
    // KNOWN LIMIT, documented rather than closed. An unquoted value
    // (`<pushStream id=room name>`) is indistinguishable here from the
    // attribute being absent, so the caller's `unwrap_or_default()`
    // yields `StreamPush { id: "" }` -- a typed, confident frame that
    // is wrong, and worse than an `UnknownTag`, because an empty id
    // reads as a legitimate push to the main window.
    //
    // Closing it properly means `attribute` returning a three-way
    // answer (absent / malformed / value) and all 46 of its call sites
    // deciding what to do with the middle case. That is a large change
    // for a shape the corpus survey measured at **0 occurrences in
    // 5,079,826 tags**, and `plan/05` §-1 says build the simplest
    // thing that works.
    //
    // UPGRADE TRIGGER: the Tier 2 replay reporting any unquoted
    // attribute value, or a single real `StreamPush { id: "" }` that
    // is not a bare `<pushStream/>`. Until then this crate claims
    // only that it handles the wire it has seen.
    pairs(tag)
        .find(|(key, _)| *key == name)
        // Decoded AND control-stripped: an id or title decoded to a raw ESC
        // reaches a frontend's window title. See `decode_attribute_value`.
        .map(|(_, value)| decode_attribute_value(value))
}

/// Each `key='value'` pair of a raw tag, in wire order, undecoded.
///
/// The one walk both [`attribute`] and [`attributes`] read by. `attribute`
/// had its own: a search for `name=` anywhere in the tag, preceded by a
/// space, which found one inside another attribute's quoted value
/// (`<a text="say id=5" id='7'>` answered `5` for `id`) where `attributes`
/// walked past it (the review of 2026-09-29). A key is the last token
/// before its `=`, so a valueless attribute is dropped rather than merged
/// into the next key; an unquoted value is skipped to the next space.
fn pairs(tag: &str) -> impl Iterator<Item = (&str, &str)> {
    let body = tag
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim_end_matches('/');
    // Skip `<name` / `</name`.
    let mut rest = body.find(char::is_whitespace).map_or("", |at| &body[at..]);
    std::iter::from_fn(move || {
        loop {
            let trimmed = rest.trim_start();
            let eq = trimmed.find('=')?;
            let key = trimmed[..eq].trim();
            let key = key.rsplit(char::is_whitespace).next().unwrap_or(key);
            let after_eq = trimmed[eq + 1..].trim_start();
            let quote = after_eq.chars().next().filter(|q| *q == '\'' || *q == '"');
            let Some((quote, end)) =
                quote.and_then(|q| after_eq[q.len_utf8()..].find(q).map(|end| (q, end)))
            else {
                // A malformed pair: SKIP IT and keep going, rather than
                // returning what has been collected so far. Returning early
                // silently dropped every attribute after the bad one --
                // `<crtrStatus exist='1' stunned='1' bad=unquoted bleeding='1'
                // dead='1'/>` yielded only `exist` and `stunned`, losing
                // `bleeding` and `dead` with no trace. For a Heal or Hunt
                // behavior reading creature status that is a wrong decision,
                // not a cosmetic one.
                //
                // The corpus survey measured 0 unquoted attribute values in
                // 5,079,826 tags, so this is about what happens when the wire
                // changes, not about traffic seen today -- which is exactly
                // what Rule 2.2 (`plan/05:276-283`) is for. An unterminated
                // quote has no next pair to find, so the walk ends.
                let space = trimmed[eq..].find(char::is_whitespace)?;
                rest = &trimmed[eq + space..];
                continue;
            };
            let value = &after_eq[quote.len_utf8()..][..end];
            rest = &after_eq[quote.len_utf8() + end + quote.len_utf8()..];
            if !key.is_empty() {
                return Some((key, value));
            }
        }
    })
}

/// Value of `name=` parsed as a number, if it is one.
#[must_use]
pub fn attribute_u32(tag: &str, name: &str) -> Option<u32> {
    attribute(tag, name)?.trim().parse().ok()
}

/// Every attribute in a raw tag, in wire order.
///
/// The shape Vellum uses for `crtrStatus` and `roommeta` and states the
/// principle for at `src/parser.rs:134-137`: keep them raw, so the layer above
/// owns the flag-name mapping. This crate does not know what `stunned` means.
#[must_use]
pub fn attributes(tag: &str) -> Vec<(String, String)> {
    pairs(tag)
        .map(|(key, value)| (key.to_owned(), decode_attribute_value(value)))
        .collect()
}

/// Build a [`Link`] from an `<a>` or `<d>` tag.
///
/// The kind is decided by what the wire actually carries, replacing Vellum's
/// `_direct_` / `_url_` string sentinels with [`LinkKind`].
#[must_use]
pub fn link_from_tag(tag: &str) -> Option<Link> {
    let coord = attribute(tag, "coord");
    if let Some(href) = attribute(tag, "href") {
        return Some(Link {
            kind: LinkKind::Url { href },
            text: String::new(),
            coord,
        });
    }
    if let Some(id) = attribute(tag, "exist") {
        return Some(Link {
            kind: LinkKind::Exist {
                id,
                noun: attribute(tag, "noun").unwrap_or_default(),
            },
            text: String::new(),
            coord,
        });
    }
    let cmd = attribute(tag, "cmd")?;
    Some(Link {
        kind: LinkKind::Direct { cmd },
        text: String::new(),
        coord,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attributes_accept_either_quoting_and_do_not_match_suffixes() {
        assert_eq!(
            attribute("<nav rm='7503251'/>", "rm").as_deref(),
            Some("7503251")
        );
        assert_eq!(
            attribute("<streamWindow id=\"room\"/>", "id").as_deref(),
            Some("room")
        );
        // `id` must not match inside `exist_id`; `top` must not match `stop`.
        assert_eq!(
            attribute("<x exist_id='7' id='real'/>", "id").as_deref(),
            Some("real")
        );
        assert_eq!(attribute("<x stop='1'/>", "top"), None);
        assert_eq!(attribute("<x/>", "missing"), None);
        assert_eq!(attribute("<x a='1'/>", ""), None);
    }

    /// The two readers walk the same pairs: a name inside another
    /// attribute's value is that value's text, not an attribute.
    #[test]
    fn a_name_inside_a_quoted_value_is_not_an_attribute() {
        let tag = "<a noun='x' text=\"say id=5 to him\" id='7' cmd='look id=9'>";
        assert_eq!(attribute(tag, "id").as_deref(), Some("7"));
        assert_eq!(attribute(tag, "to"), None);
        assert_eq!(
            attributes(tag),
            vec![
                ("noun".to_owned(), "x".to_owned()),
                ("text".to_owned(), "say id=5 to him".to_owned()),
                ("id".to_owned(), "7".to_owned()),
                ("cmd".to_owned(), "look id=9".to_owned()),
            ]
        );
        // And the same answer whatever comes first, malformed pairs included.
        for tag in [
            "<x a=1 b='2' c=\"3\"/>",
            "<roommeta weather='rain' bonfire inside='1'/>",
            "<x a='unterminated/>",
            "<x a = '1'  b= \"2\" />",
        ] {
            for (key, value) in attributes(tag) {
                assert_eq!(attribute(tag, &key), Some(value), "{tag}: {key}");
            }
        }
        assert_eq!(attribute("<x a = '1'/>", "a").as_deref(), Some("1"));
    }

    #[test]
    fn tag_names_survive_a_space_before_the_slash() {
        // `<style id="roomName" />` is the real corpus form.
        assert_eq!(tag_name("<style id=\"roomName\" />"), "style");
        assert_eq!(tag_name("<popStream/>"), "popStream");
        assert_eq!(tag_name("</compDef>"), "compDef");
        assert_eq!(tag_name("<a>"), "a");
        assert_eq!(tag_name("not a tag"), "not");
    }

    #[test]
    fn dollar_escaped_markup_is_not_treated_as_a_tag() {
        // The game's broken HELP escaping. Interpreting it corrupts state.
        assert_eq!(find_tag_start("plain $<a href=$Q"), None);
        assert_eq!(find_tag_start("text <b>"), Some(5));
        assert_eq!(find_tag_start("$< then <b>"), Some(8));
    }

    #[test]
    fn attribute_bags_keep_wire_order_and_stop_at_malformed() {
        let attrs = attributes("<crtrStatus exist='123' stunned='1' bleeding='0'/>");
        assert_eq!(
            attrs,
            vec![
                ("exist".to_owned(), "123".to_owned()),
                ("stunned".to_owned(), "1".to_owned()),
                ("bleeding".to_owned(), "0".to_owned()),
            ]
        );
        // A malformed pair is SKIPPED, not a full stop. Stopping dropped
        // every attribute after it: `bleeding` and `dead` below vanished
        // silently, and a Heal behavior reading creature status would then act
        // on a corpse it believed to be alive.
        assert_eq!(
            attributes("<x a=1 b='2'/>"),
            vec![("b".to_owned(), "2".to_owned())]
        );
        assert_eq!(
            attributes("<crtrStatus exist='1' stunned='1' bad=unquoted bleeding='1' dead='1'/>"),
            vec![
                ("exist".to_owned(), "1".to_owned()),
                ("stunned".to_owned(), "1".to_owned()),
                ("bleeding".to_owned(), "1".to_owned()),
                ("dead".to_owned(), "1".to_owned()),
            ]
        );
        // A valueless attribute is dropped, not merged into the next key.
        // `("bonfire inside", "1")` is a name no consumer can match, and it
        // took `inside` down with it.
        assert_eq!(
            attributes("<roommeta weather='rain' bonfire inside='1' sanctuary='1'/>"),
            vec![
                ("weather".to_owned(), "rain".to_owned()),
                ("inside".to_owned(), "1".to_owned()),
                ("sanctuary".to_owned(), "1".to_owned()),
            ]
        );
        // An unterminated quote has no next pair to find, so the scan ends.
        assert_eq!(attributes("<x a='unterminated/>"), vec![]);
        assert_eq!(attributes("<popStream/>"), vec![]);
    }

    #[test]
    fn link_kind_comes_from_the_wire_not_a_sentinel() {
        let exist = link_from_tag("<a exist=\"-482279\" noun=\"counter\">").unwrap();
        assert_eq!(
            exist.kind,
            LinkKind::Exist {
                id: "-482279".to_owned(),
                noun: "counter".to_owned()
            }
        );
        let direct = link_from_tag("<d cmd='go out'>").unwrap();
        assert_eq!(
            direct.kind,
            LinkKind::Direct {
                cmd: "go out".to_owned()
            }
        );
        // A bare `<d>` carries no command; the text is the command.
        assert!(link_from_tag("<d>").is_none());
    }
}
