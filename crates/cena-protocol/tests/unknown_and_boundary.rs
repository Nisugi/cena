//! Rule 2.2 (unknown tags survive) and the read boundary (split tags rejoin).
//!
//! Both are behaviors the reference gets partly wrong, so these are tests of
//! the rule rather than characterizations of the port. `plan/05` §0: each one
//! names what makes it go RED.

use cena_protocol::Parser;
use cena_protocol::frame::Frame;

// ---------------------------------------------------------------------------
// Rule 2.2 -- plan/05:276-283
// ---------------------------------------------------------------------------

#[test]
fn an_unmodelled_tag_becomes_a_typed_unknown_carrying_its_bytes() {
    // Vellum's unknown tag becomes a Text element indistinguishable from game
    // prose (`src/parser.rs:1047-1049`): it has the passthrough but not the
    // TYPE, so no consumer can tell "the game said something new" from "the
    // game said hello". This test goes RED if that behavior is reintroduced.
    let mut parser = Parser::new();
    let frames = parser.parse_line("<unknownFutureTag attr=\"value\"/>");
    let unknown: Vec<(&str, &str)> = frames
        .iter()
        .filter_map(|f| match f {
            Frame::UnknownTag { name, raw } => Some((name.as_str(), raw.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(
        unknown,
        vec![("unknownFutureTag", "<unknownFutureTag attr=\"value\"/>")],
        "an unmodelled tag must arrive as Frame::UnknownTag with the original \
         bytes, so a reader diagnosing a protocol change sees what was sent"
    );
}

#[test]
fn an_unknown_tag_never_removes_the_prose_around_it() {
    // Rule 2.2: never silently dropped. The text on both sides must survive
    // too, or the user loses game output to a protocol change.
    let mut parser = Parser::new();
    let frames =
        parser.parse_line("before<unknownFutureTag/>between<anotherNewTag>inner</anotherNewTag>");
    let text: String = frames
        .iter()
        .filter_map(|f| match f {
            Frame::Text(t) => Some(t.content.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        text.contains("before") && text.contains("between") && text.contains("inner"),
        "prose around unknown markup must survive; got {text:?}"
    );
    let names: Vec<&str> = frames
        .iter()
        .filter_map(|f| match f {
            Frame::UnknownTag { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        names,
        vec!["unknownFutureTag", "anotherNewTag", "anotherNewTag"],
        "both the open and the close of an unknown paired tag are news"
    );
}

#[test]
fn the_client_injected_tags_cena_does_not_speak_fall_through_to_unknown() {
    // <vellumImg> is written by VellumFE into its own logs and never sent by
    // the game (reference/VellumFE/src/core/inline_image.rs:5). Cena does not
    // inject it, so if one ever arrives it is genuinely unknown -- modelling
    // it would teach Cena a wire vocabulary that does not exist.
    let mut parser = Parser::new();
    for tag in ["<vellumImg src='x.png'/>", "<vellumCmd>x</vellumCmd>"] {
        let frames = parser.parse_line(tag);
        assert!(
            frames.iter().any(|f| matches!(f, Frame::UnknownTag { .. })),
            "{tag} must reach Frame::UnknownTag, not a handler; got {frames:#?}"
        );
    }
}

#[test]
fn a_known_but_unhandled_tag_still_produces_a_frame() {
    // Vellum logs these at debug! and DISCARDS them (`src/parser.rs:1044`) --
    // roughly 50 of the 116 tags vanish with no user-visible trace. The M1
    // scope decision says every variant parses and renders, so nothing here
    // may be dropped.
    let mut parser = Parser::new();
    for tag in [
        "<indicator id='IconBLEEDING' visible='y'/>",
        "<roundTime value='1764475410'/>",
        "<crtrStatus exist='123' stunned='1'/>",
        "<streamWindow id='main' title='Story'/>",
        "<skin id='healthSkin' name='healthBar'/>",
    ] {
        let frames = parser.parse_line(tag);
        assert!(
            !frames.is_empty(),
            "{tag} produced no frame at all -- that is the silent drop this \
             port exists to avoid"
        );
    }
}

#[test]
fn the_four_tags_vellum_drops_before_the_unknown_check_are_not_dropped_here() {
    // `src/parser.rs:1030-1036` discards <compDef, </compDef>, <streamWindow
    // and <skin with NO log, not even debug!. Two of the four are in M1's room
    // path. This test goes RED if that branch is ever ported.
    let mut parser = Parser::new();
    let frames = parser.parse_line("<compDef id='room desc'>A room.</compDef>");
    assert!(
        frames
            .iter()
            .any(|f| matches!(f, Frame::Component { id, .. } if id == "room desc")),
        "compDef carries the room description; dropping it loses the room"
    );
}

// ---------------------------------------------------------------------------
// The read boundary -- a tag split across two socket reads
// ---------------------------------------------------------------------------

#[test]
fn a_tag_split_across_two_reads_is_rejoined_before_parsing() {
    // The failure this prevents: half a tag parsed as if complete, which
    // desyncs the parser for the rest of the session.
    let mut parser = Parser::new();
    let first = parser.push_bytes(b"<nav rm='750");
    assert!(
        first.is_empty(),
        "an incomplete line must yield no frames at all, not a guess: {first:#?}"
    );
    assert!(parser.pending_len() > 0, "the fragment must stay buffered");

    let second = parser.push_bytes(b"3251'/>\n");
    assert!(
        second
            .iter()
            .any(|f| matches!(f, Frame::RoomId { id } if id == "7503251")),
        "the rejoined tag must parse as one frame; got {second:#?}"
    );
    assert_eq!(parser.pending_len(), 0);
}

#[test]
fn a_split_at_every_byte_offset_gives_identical_frames() {
    // The strong form: whatever the TCP layer does to this line, the frames
    // are the same. Drives the boundary at all 87 interior offsets rather
    // than at one hand-picked spot.
    let line = "<nav rm='7503251'/><compDef id='room exits'>Obvious exits: <d>east</d></compDef>\n";
    let bytes = line.as_bytes();

    let mut whole = Parser::new();
    let expected = whole.push_bytes(bytes);
    assert!(!expected.is_empty(), "the control case must produce frames");

    for split in 1..bytes.len() {
        let mut parser = Parser::new();
        let mut got = parser.push_bytes(&bytes[..split]);
        got.extend(parser.push_bytes(&bytes[split..]));
        assert_eq!(
            got, expected,
            "splitting the read at byte {split} changed the frames, so the \
             parser is sensitive to TCP fragmentation"
        );
    }
}

#[test]
fn a_tag_that_never_closes_is_typed_rather_than_smuggled_into_prose() {
    // Vellum appends the fragment to the text buffer and silently desyncs
    // (`src/parser.rs:733-736`), with no log and no test. Its own fixture has
    // this case at `tests/fixtures/parser_edge_cases.xml:31` and asserts
    // nothing about it. Here it is typed.
    let mut parser = Parser::new();
    let frames = parser.parse_line("A line with trailing partial tag <pushStr");
    assert!(
        frames
            .iter()
            .any(|f| matches!(f, Frame::MalformedTag { raw } if raw == "<pushStr")),
        "an unterminated tag must be Frame::MalformedTag, not prose: {frames:#?}"
    );
    assert!(
        frames
            .iter()
            .any(|f| matches!(f, Frame::Text(t) if t.content.contains("A line with"))),
        "and the prose before it must still reach the user"
    );
}

#[test]
fn a_malformed_tag_does_not_desync_the_next_line() {
    // The consequence that makes the above worth typing: state must be clean
    // for the line after.
    let mut parser = Parser::new();
    let _ = parser.parse_line("broken <pushStr");
    let frames = parser.parse_line("<nav rm='999'/>ordinary");
    assert!(
        frames
            .iter()
            .any(|f| matches!(f, Frame::RoomId { id } if id == "999")),
        "the line after a malformed tag must parse normally; got {frames:#?}"
    );
}

#[test]
fn an_unclosed_component_is_reported_and_does_not_swallow_the_session() {
    // This test replaces `a_multi_line_component_is_captured_across_the_line
    // _boundary`, which asserted that an unclosed `<component>` buffered until
    // its close arrived. Two measurements retired that behavior:
    //
    //   * 447,095 `<compDef` opens across 272 stratified corpus files, and
    //     ZERO with no close on the same line;
    //   * the parser driven over 60 of those files -- 1,230,355 wire lines --
    //     entering the capture state zero times.
    //
    // The old test's own comment claimed "the corpus carries multi-line
    // <component> bodies". It does not. What the buffering did carry was a
    // black hole: bounded by 256 KiB of accumulated body rather than by lines,
    // it silently discarded 4,297 consecutive lines of 60-character game prose
    // before giving up.
    //
    // So an unclosed paired tag is now a MalformedTag -- Rule 2.2, typed and
    // surfaced -- and the line after it parses normally.
    let mut parser = Parser::new();
    let first = parser.parse_line("<component id='room objs'>You also see");
    assert!(
        first
            .iter()
            .any(|f| matches!(f, Frame::MalformedTag { raw } if raw.starts_with("<component"))),
        "an unclosed paired tag must be reported, not buffered: {first:#?}"
    );

    // The consequence that matters: the next line is NOT swallowed.
    let second = parser.parse_line("a rock.");
    assert!(
        second
            .iter()
            .any(|f| matches!(f, Frame::Text(t) if t.content.contains("a rock."))),
        "the line after an unclosed tag must reach the user; got {second:#?}"
    );

    // And the parser is clean enough to parse a real room on the line after.
    let third = parser.parse_line("<nav rm='7503251'/>");
    assert!(
        third
            .iter()
            .any(|f| matches!(f, Frame::RoomId { id } if id == "7503251")),
        "an unclosed tag must not desync the parser; got {third:#?}"
    );
}

#[test]
fn a_component_that_closes_on_its_own_line_still_parses() {
    // The form the wire actually sends, 447,095 times in a 272-file sample:
    // open and close on one line. Removing the capture path must not have
    // touched it.
    let mut parser = Parser::new();
    let frames = parser.parse_line(
        "<component id='room objs'>You also see a <a exist='1' noun='rock'>rock</a>.</component>",
    );
    let body = frames
        .iter()
        .find_map(|f| match f {
            Frame::Component { id, body } if id == "room objs" => Some(body),
            _ => None,
        })
        .expect("a single-line component still yields its frame");
    assert_eq!(
        body.plain(),
        "You also see a rock.",
        "the body must arrive parsed, with markup removed (Rule 2.1)"
    );
}

#[test]
fn cp1252_high_bytes_decode_rather_than_disconnecting() {
    // The game stream is CP1252. `read_line` on a String returns InvalidData
    // on any non-UTF-8 byte, which used to hard-disconnect Vellum
    // (`src/network.rs:531`). 0x92 is a right single quote.
    let mut parser = Parser::new();
    let frames = parser.push_bytes(b"Kertigen\x92s Honor\n");
    let text: String = frames
        .iter()
        .filter_map(|f| match f {
            Frame::Text(t) => Some(t.content.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        text, "Kertigen\u{2019}s Honor",
        "a CP1252 high byte must decode, not drop the line"
    );
}
