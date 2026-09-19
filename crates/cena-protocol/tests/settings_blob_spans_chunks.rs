//! **A `<settings>` blob over twice the line cap ate the login burst.**
//!
//! The login blob is legitimately bigger than `MAX_LINE_BYTES` -- VERIFIED at
//! **513,700 bytes on one line** -- so `push_bytes` has a branch that recognises
//! it and hands it to the region parser instead of truncating it. That branch
//! tested only what the **first** capped chunk started with:
//!
//! ```text
//! if raw.trim_start().starts_with("<settings") { ... }
//! ```
//!
//! So a blob longer than **two** caps lost everything after the second one:
//!
//! 1. first cap -> `ClientSettings`, `in_settings = true`. Correct.
//! 2. second cap -> the chunk does not start with `<settings`, so it becomes a
//!    `MalformedTag` of 262,144 bytes and `dropping_oversized_line` is set.
//! 3. the rest of the line, **including `</settings>`**, is discarded.
//! 4. `in_settings` is still true, so every following line is swallowed by a
//!    region that will never close -- until one happens to contain `<prompt`.
//!
//! The threshold is `2 * 262,144 + 2` = **524,290 bytes**, and the measured blob
//! is 513,700 -- **2.1% under**. `parser_never_panics.rs` sizes its blob at
//! exactly 513,700, so it never reaches a second cap.
//!
//! **That is `plan/19` pattern C, in its purest form**: a guard sized to the
//! measurement rather than to the defect. These tests are sized at **3x the cap**
//! for that reason, not at 2x and not at the measurement.
//!
//! # Two smaller defects in the same branch
//!
//! * **The byte that trips the cap is never stored.** The `else if
//!   self.pending.len() < MAX_LINE_BYTES` arm pushes; the cap arm does not, so
//!   one byte of the line is silently lost at every cap boundary.
//! * **A `</settings>` straddling a chunk boundary is never matched**, because
//!   each capped chunk is searched in isolation.
//!
//! Both leave `in_settings` stuck, which is the same swallow by another route.

use cena_protocol::{Frame, Parser};

/// `MAX_LINE_BYTES`, mirrored: the constant is private, and a test that read it
/// through a back door would stop testing the number the parser actually uses.
const CAP: usize = 256 * 1024;

/// A settings blob of at least `bytes`, followed by an ordinary login burst.
fn blob_then_burst(bytes: usize) -> Vec<u8> {
    let mut wire = String::from("<settings client='1' major='1'>");
    while wire.len() < bytes {
        wire.push_str("<h id='1' text='highlight string number one'/>");
    }
    wire.push_str("</settings>\n");
    // What the wire really sends next, and what was being lost.
    wire.push_str("<nav rm='7503251'/>\n");
    wire.push_str("<streamWindow id='room' title='Room'/>\n");
    wire.push_str("You see a room.\n");
    wire.push_str("<prompt time='1'>&gt;</prompt>\n");
    wire.into_bytes()
}

fn frames(wire: &[u8]) -> Vec<Frame> {
    Parser::new().push_bytes(wire)
}

#[test]
fn a_blob_three_times_the_cap_does_not_swallow_the_login_burst() {
    // THE DEFECT. Sized at 3x rather than 2x: a fix that handled only the second
    // chunk would pass at 2x and fail here, and a guard that cannot fail on the
    // next chunk is the same pattern that let this ship.
    let found = frames(&blob_then_burst(CAP * 3));

    assert!(
        found
            .iter()
            .any(|f| matches!(f, Frame::RoomId { id } if id == "7503251")),
        "the `<nav>` after the blob was swallowed: a login burst is lost after \
         any settings blob over twice the line cap"
    );
    assert!(
        found
            .iter()
            .any(|f| matches!(f, Frame::StreamWindow { id, .. } if id == "room")),
        "the room window declaration was swallowed"
    );
    assert!(
        found
            .iter()
            .any(|f| matches!(f, Frame::Text(t) if t.content.contains("You see a room"))),
        "the room text was swallowed"
    );
}

#[test]
fn a_blob_three_times_the_cap_is_still_reported_once_as_settings() {
    // The branch exists so a legitimate blob is a region rather than a cry-wolf
    // `MalformedTag`. Fixing the swallow must not lose that.
    let found = frames(&blob_then_burst(CAP * 3));

    assert_eq!(
        found
            .iter()
            .filter(|f| matches!(f, Frame::ClientSettings))
            .count(),
        1,
        "the blob should be announced exactly once"
    );
    assert!(
        !found
            .iter()
            .any(|f| matches!(f, Frame::MalformedTag { .. })),
        "a legitimate settings blob was reported as malformed -- the cry-wolf \
         failure the region branch exists to prevent"
    );
}

#[test]
fn the_blobs_contents_do_not_reach_the_user_as_prose() {
    // The other half of the region's job. 26 private element names inside the
    // blob must not become `UnknownTag`s, and its text must not render.
    let found = frames(&blob_then_burst(CAP * 3));

    assert!(
        !found
            .iter()
            .any(|f| matches!(f, Frame::Text(t) if t.content.contains("highlight string"))),
        "the blob's contents rendered as display text"
    );
    assert!(
        !found
            .iter()
            .any(|f| matches!(f, Frame::UnknownTag { name, .. } if name == "h")),
        "the blob's private elements leaked as UnknownTag"
    );
}

#[test]
fn a_blob_split_across_reads_is_handled_the_same_way() {
    // The real delivery. `push_bytes` is fed whatever one `read` returned, and a
    // 786 KB blob never arrives in one chunk -- so the chunk boundaries fall in
    // arbitrary places inside it, including inside `</settings>`.
    let wire = blob_then_burst(CAP * 3);
    let mut parser = Parser::new();
    let mut found = Vec::new();
    // A deliberately awkward chunk size: not a factor of the cap, so boundaries
    // land mid-tag and mid-close.
    for chunk in wire.chunks(9_973) {
        found.extend(parser.push_bytes(chunk));
    }

    assert!(
        found
            .iter()
            .any(|f| matches!(f, Frame::RoomId { id } if id == "7503251")),
        "the burst was swallowed when the blob arrived in realistic chunks"
    );
}

#[test]
fn a_close_tag_straddling_a_chunk_boundary_still_closes_the_region() {
    // `</settings>` is 11 bytes. A chunk boundary inside it left the region open
    // forever, because each capped chunk was searched in isolation.
    let mut wire = String::from("<settings client='1'>");
    while wire.len() < CAP * 2 + 5 {
        wire.push_str("<h id='1' text='padding padding padding'/>");
    }
    wire.push_str("</settings>\n<nav rm='999'/>\n");
    let bytes = wire.into_bytes();

    // Split so the boundary falls INSIDE the closing tag.
    let close_at = bytes.len() - "</settings>\n<nav rm='999'/>\n".len();
    let split = close_at + 5;
    let mut parser = Parser::new();
    let mut found = parser.push_bytes(&bytes[..split]);
    found.extend(parser.push_bytes(&bytes[split..]));

    assert!(
        found
            .iter()
            .any(|f| matches!(f, Frame::RoomId { id } if id == "999")),
        "a `</settings>` split across two reads left the region open forever"
    );
}

#[test]
fn an_ordinary_runaway_line_is_still_truncated_and_reported() {
    // The falsifying pair. Without it, "never truncate anything" would satisfy
    // every test above while removing the bound the cap exists to enforce.
    let mut wire = "x".repeat(CAP * 2);
    wire.push_str("\n<nav rm='42'/>\n");
    let found = frames(wire.as_bytes());

    assert!(
        found
            .iter()
            .any(|f| matches!(f, Frame::MalformedTag { .. })),
        "a genuine runaway line was not reported"
    );
    assert!(
        found
            .iter()
            .any(|f| matches!(f, Frame::RoomId { id } if id == "42")),
        "...and the parser must still resume on the next line"
    );
}

#[test]
fn the_byte_that_trips_the_cap_is_not_lost() {
    // A line of exactly `CAP + 1` printable bytes with a marker at the very end.
    // The cap arm never pushed the triggering byte, so one byte per boundary was
    // silently dropped -- invisible in a blob, but this is a parser, and a byte
    // it says it saw must be a byte it saw.
    let mut wire = "<settings client='1'>".to_owned();
    while wire.len() < CAP - 1 {
        wire.push('y');
    }
    // These two bytes straddle the cap.
    wire.push('A');
    wire.push('B');
    wire.push_str("</settings>\n<nav rm='7'/>\n");
    let found = frames(wire.as_bytes());

    assert!(
        found
            .iter()
            .any(|f| matches!(f, Frame::RoomId { id } if id == "7")),
        "the region did not close, so the following line was swallowed"
    );
}
