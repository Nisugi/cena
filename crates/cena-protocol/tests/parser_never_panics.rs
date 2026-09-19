//! The parser never panics. `plan/06` §1.5 calls this non-negotiable:
//! "hostile or corrupt input must degrade to `Frame::Unknown`, never crash a
//! session."
//!
//! Three tiers, because random bytes alone are a weak generator: they almost
//! never produce a *plausible* tag, so they exercise the scanner's reject path
//! and little else.
//!
//! 1. **Real fragments** ([`HOSTILE_FRAGMENTS`]) -- malformations taken from
//!    the corpus survey and from Vellum's own adversarial fixture, including
//!    the ones its parser handles badly.
//! 2. **Structured generation** -- a grammar that emits tag-shaped garbage, so
//!    the *handlers* are reached rather than only the scanner.
//! 3. **Arbitrary bytes** -- including invalid UTF-8, which is the real
//!    CP1252 case.
//!
//! A panic anywhere in this crate would kill one session (`plan/12` §5.5 keeps
//! `panic = "unwind"` so it does not kill the process), but a session dying on
//! a protocol change is exactly the outcome Rule 2.2 exists to prevent.

use cena_protocol::Parser;
use cena_protocol::frame::Frame;
use proptest::prelude::*;

/// Malformations that really occur, or that the reference mishandles.
///
/// Sources: the corpus survey (5,079,826 tags sampled), Vellum's
/// `tests/fixtures/parser_edge_cases.xml`, and the two confirmed reference
/// bugs. Each is a case where a naive scanner does something wrong rather than
/// merely unexpected.
const HOSTILE_FRAGMENTS: &[&str] = &[
    // Trailing partial tag -- Vellum smuggles this into prose and desyncs.
    "A line with trailing partial tag <pushStr",
    "<",
    "<<<<",
    ">>>>",
    "</",
    "</>",
    "<>",
    // The game's own broken HELP escaping, both sides of it.
    "$<a href=$Qhttp://example$Q$>text$</a$>",
    "<a href='x'>unclosed link",
    "</a></a></a>",
    // Space before the slash -- the real corpus form of <style>.
    "<style id=\"roomName\" />",
    // Mode switches that never close: 23,328 opens, 0 closes in the sample.
    "<style id=\"roomDesc\"/>",
    "<output class=\"mono\"/>",
    "<output class=\"\"/>",
    // Stream push without a pop: measured ratio 1.301.
    "<pushStream id='inv'/>Your worn items are:",
    "<popStream/>",
    "<popStream id='room'/>",
    // Unbalanced bold.
    "<pushBold/>orphaned",
    "<popBold/><popBold/><popBold/>",
    // Entities, including unknown and malformed ones.
    "&unknown; &#65; &#x42; &amp;amp; &#; &#xZZ; &",
    "Stalking & Hiding",
    // Numbers: the negative-health case and a max that is not a number.
    "<progressBar id='health' value='0' text='health -10/125'/>",
    "<progressBar id='health' value='0' text='health /'/>",
    "<progressBar id='x' value='999999999999999' text='a/b'/>",
    // Attributes that are empty, repeated, or unquoted.
    "<nav rm=''/>",
    "<nav rm='1' rm='2'/>",
    "<nav rm=7503251/>",
    "<component id=>",
    // Deep nesting and a component that never closes.
    "<component id='room objs'>unterminated",
    "<compDef id='a'><compDef id='b'><compDef id='c'>",
    // A tag name that is only punctuation or whitespace.
    "< >",
    "<   nav rm='1'/>",
    // Control characters and a lone CR.
    "text\u{1}with\u{7f}controls",
    "text\r",
    // Unknown markup, the Rule 2.2 case.
    "<unknownFutureTag attr=\"value\"/>between<anotherNewTag>inner</anotherNewTag>",
    // A prompt with no time, and one that is not a number.
    "<prompt>&gt;</prompt>",
    "<prompt time='not-a-number'>&gt;</prompt>",
    // Very long attribute value.
    "<nav rm='000000000000000000000000000000000000000000000000000'/>",
];

#[test]
fn every_hostile_fragment_parses_without_panicking() {
    for fragment in HOSTILE_FRAGMENTS {
        let mut parser = Parser::new();
        // Each on its own parser, and again on a shared one, because a
        // fragment that corrupts state only hurts the lines after it.
        let _ = parser.parse_line(fragment);
        let _ = parser.parse_line("<prompt time='1'>&gt;</prompt>");
    }

    // And all of them in sequence through one parser, which is the case that
    // actually resembles a session.
    let mut parser = Parser::new();
    for fragment in HOSTILE_FRAGMENTS {
        let _ = parser.parse_line(fragment);
    }
}

#[test]
fn a_hostile_fragment_never_leaves_the_parser_unable_to_see_the_next_room() {
    // Stronger than "does not panic": state must recover. A parser that
    // survives garbage but then misses every room is no better.
    for fragment in HOSTILE_FRAGMENTS {
        let mut parser = Parser::new();
        let _ = parser.parse_line(fragment);
        // The prompt is the resync barrier; after it the parser must be sane.
        let _ = parser.parse_line("<prompt time='1'>&gt;</prompt>");
        let frames = parser.parse_line("<nav rm='7503251'/>");
        assert!(
            frames
                .iter()
                .any(|f| matches!(f, Frame::RoomId { id } if id.as_deref() == Some("7503251"))),
            "after {fragment:?} and a prompt, the parser could not see a room \
             change -- state did not recover"
        );
    }
}

/// Tag-shaped garbage: reaches the handlers, not just the scanner.
fn tagish() -> impl Strategy<Value = String> {
    let name = prop::sample::select(vec![
        "nav",
        "prompt",
        "progressBar",
        "compDef",
        "component",
        "pushStream",
        "popStream",
        "compass",
        "dialogData",
        "a",
        "d",
        "style",
        "output",
        "pushBold",
        "inv",
        "unknownFutureTag",
        "",
        " ",
    ]);
    let attr = prop::sample::select(vec![
        "",
        " id='x'",
        " id=\"x\"",
        " rm='1'",
        " value='-1'",
        " text='health -10/125'",
        " id=",
        " ='x'",
        " id='",
        " exist='1' noun='y'",
    ]);
    let close = prop::sample::select(vec!["/>", ">", "", " />", "/"]);
    (name, attr, close, 0usize..3).prop_map(|(n, a, c, slashes)| {
        let open = if slashes % 2 == 0 { "<" } else { "</" };
        format!("{open}{n}{a}{c}")
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    /// Arbitrary text, including the tag characters, never panics.
    #[test]
    fn arbitrary_text_never_panics(line in ".*") {
        let mut parser = Parser::new();
        let _ = parser.parse_line(&line);
    }

    /// Sequences of tag-shaped garbage never panic and never wedge.
    #[test]
    fn sequences_of_tagish_garbage_never_panic(parts in prop::collection::vec(tagish(), 0..24)) {
        let mut parser = Parser::new();
        for part in &parts {
            let _ = parser.parse_line(part);
        }
        // The resync barrier still works after anything.
        let _ = parser.parse_line("<prompt time='1'>&gt;</prompt>");
        let frames = parser.parse_line("<nav rm='42'/>");
        prop_assert!(
            frames.iter().any(|f| matches!(f, Frame::RoomId { id } if id.as_deref() == Some("42"))),
            "parser failed to recover after {parts:?}"
        );
    }

    /// Arbitrary BYTES -- not just valid UTF-8. The wire is CP1252, so
    /// invalid-UTF-8 input is the ordinary case, not the hostile one.
    #[test]
    fn arbitrary_bytes_never_panic(bytes in prop::collection::vec(any::<u8>(), 0..512)) {
        let mut parser = Parser::new();
        let _ = parser.push_bytes(&bytes);
        let _ = parser.push_bytes(b"\n");
    }

    /// Arbitrary split points over arbitrary bytes: the read boundary must be
    /// safe wherever the network happens to cut.
    #[test]
    fn arbitrary_bytes_split_anywhere_never_panic(
        bytes in prop::collection::vec(any::<u8>(), 0..256),
        split in 0usize..256,
    ) {
        let at = split.min(bytes.len());
        let mut parser = Parser::new();
        let _ = parser.push_bytes(&bytes[..at]);
        let _ = parser.push_bytes(&bytes[at..]);
        let _ = parser.push_bytes(b"\n");
    }

    /// Real fragments, arbitrarily interleaved and split. This is the
    /// generator most likely to find something: plausible input in an
    /// implausible order.
    #[test]
    fn hostile_fragments_interleaved_never_panic(
        picks in prop::collection::vec(0usize..HOSTILE_FRAGMENTS.len(), 0..16),
    ) {
        let mut parser = Parser::new();
        for pick in picks {
            let fragment = HOSTILE_FRAGMENTS[pick];
            let _ = parser.push_bytes(fragment.as_bytes());
            let _ = parser.push_bytes(b"\n");
        }
    }
}

#[test]
fn a_runaway_line_with_no_newline_does_not_grow_without_bound() {
    // A peer that never sends a newline must cost bounded memory, not the
    // process. Vellum applies the same guard to multi-line captures
    // (`src/parser/handlers.rs:199-206`).
    let mut parser = Parser::new();
    let chunk = vec![b'x'; 64 * 1024];
    for _ in 0..16 {
        let _ = parser.push_bytes(&chunk);
    }
    assert!(
        parser.pending_len() <= 256 * 1024,
        "1 MiB of newline-free input left {} bytes buffered",
        parser.pending_len()
    );
}

#[test]
fn a_component_that_never_closes_swallows_nothing_at_all() {
    // This test used to feed 4096 lines of `"x".repeat(256)` and assert that
    // the parser had recovered by the end. It passed because of its chosen
    // constant, not because the parser was bounded in any way a player would
    // notice: 4096 * 256 bytes is comfortably over the old 256 KiB capture
    // cap, and at a realistic 60-character line the SAME test found 4,297
    // lines already silently discarded before the cap fired.
    //
    // So it now uses a game-prose line length and asserts the real contract:
    // not "recovers eventually" but "loses nothing, starting immediately".
    let mut parser = Parser::new();
    let opened = parser.parse_line("<component id='room objs'>opened and never closed");
    assert!(
        opened
            .iter()
            .any(|f| matches!(f, Frame::MalformedTag { .. })),
        "an unclosed paired tag is reported at once: {opened:#?}"
    );

    // Every subsequent line reaches the user. Not one is buffered.
    for i in 0..4096 {
        let line = format!("line {i} of ordinary game prose, about sixty chars.");
        let frames = parser.parse_line(&line);
        assert!(
            frames
                .iter()
                .any(|f| matches!(f, Frame::Text(t) if t.content.contains("ordinary game prose"))),
            "line {i} after an unclosed tag was swallowed; got {frames:#?}"
        );
    }

    let frames = parser.parse_line("<nav rm='7503251'/>");
    assert!(
        frames
            .iter()
            .any(|f| matches!(f, Frame::RoomId { id } if id.as_deref() == Some("7503251"))),
        "the parser must still be in sync afterwards"
    );
}

#[test]
fn an_oversized_line_never_smuggles_markup_into_prose() {
    // The length guard used to flush at the cap and RE-PARSE from an arbitrary
    // byte, which is usually mid-tag. `<pushStream id='room'/>` past the cap
    // became `MalformedTag { raw: "<push" }` plus the literal text
    // `Stream id='room'/>BODY` -- raw markup rendered to the user, which is
    // the precise bug `parser.rs`'s header says this crate does not inherit.
    let mut oversized = "A".repeat(256 * 1024 - 5);
    oversized.push_str("<pushStream id='room'/>BODY<prompt time='1'>&gt;</prompt>\n");

    let mut parser = Parser::new();
    let frames = parser.push_bytes(oversized.as_bytes());

    for frame in &frames {
        if let Frame::Text(t) = frame {
            assert!(
                !t.content.contains("pushStream")
                    && !t.content.contains("Stream id=")
                    && !t.content.contains("/>"),
                "markup reached the text stream: {:?}",
                t.content
            );
        }
    }
    assert!(
        frames
            .iter()
            .any(|f| matches!(f, Frame::MalformedTag { .. })),
        "the oversized line must be reported as one typed frame: {frames:#?}"
    );

    // And the parser resumes on the next newline, in sync.
    let after = parser.push_bytes(b"<nav rm='7503251'/>\n");
    assert!(
        after
            .iter()
            .any(|f| matches!(f, Frame::RoomId { id } if id.as_deref() == Some("7503251"))),
        "the line after an oversized one must parse normally; got {after:#?}"
    );
}

#[test]
fn the_login_settings_blob_survives_being_bigger_than_the_line_cap() {
    // The one legitimate oversized line, and the reason the guard above has an
    // exception: VERIFIED at 513,700 bytes on one line in the corpus. It is
    // consumed as a region, so it must yield ClientSettings and NOT the
    // MalformedTag an ordinary runaway yields -- crying wolf on every login is
    // the failure the region exists to prevent.
    let mut blob = String::from("<settings client='1' major='1'>");
    while blob.len() < 513_700 {
        blob.push_str("<h id='1'/><dc id='2'/><ignores/><panels/>");
    }
    blob.push_str("</settings>\n<nav rm='7503251'/>\n");

    let mut parser = Parser::new();
    let frames = parser.push_bytes(blob.as_bytes());

    assert!(
        frames.iter().any(|f| matches!(f, Frame::ClientSettings)),
        "an oversized settings blob must still be recognised: {frames:#?}"
    );
    assert!(
        !frames.iter().any(|f| matches!(f, Frame::UnknownTag { .. })),
        "the blob's private element names must not leak: {frames:#?}"
    );
    assert!(
        frames
            .iter()
            .any(|f| matches!(f, Frame::RoomId { id } if id.as_deref() == Some("7503251"))),
        "traffic after the blob must parse normally: {frames:#?}"
    );
}
