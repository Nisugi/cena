//! Drop-nothing: every tag the wire can send must be observable.
//!
//! The author's scope rule, in his words:
//!
//! > "vellum drops some things it doesn't want, just because we didn't know
//! > what to do with them at the time. So Cena shouldn't drop anything that
//! > comes in, it should all be processed and figured out how to handle."
//!
//! `parser/dispatch.rs` documents two silent-drop paths from Vellum that were
//! refused. The port then introduced a **third**: `markup_tag`'s two `_ => {}`
//! arms and `close_tag`'s `_ if tags::is_known(name) => {}`. Being *in* the
//! table was what silenced a tag -- the same dead-ratchet shape `tags.rs`
//! records, where an explicit arm shields a name from the check meant to catch
//! it. This file is what keeps that closed.
//!
//! # Why `is_empty()` is the right detector (plan/05 §0)
//!
//! A silent arm is, exactly, an arm that returns without pushing to `frames`.
//! Each probe line below holds one tag and no prose, so
//! `parse_line(line).is_empty()` is true **iff** that tag's dispatch path
//! pushed nothing. There is no way to add a `_ => {}` in `dispatch.rs`,
//! `markup.rs` or `thin.rs` and keep this green.
//!
//! VERIFIED RED before the fix, with the arms as the port left them:
//!
//! ```text
//! $ cargo test -p cena-protocol --test every_tag_is_observable
//! 133 tag/form combinations produce NO frame, so a consumer cannot see them
//! at all.
//!   <FEStart> in close form
//!   ...
//!   <style> in selfclose form
//! test result: FAILED. 0 passed; 6 failed
//! ```
//!
//! **123 close forms -- every tag in the table -- plus 10 self-closing**
//! (`a b d i output popBold preset pushBold resource style`). The design brief
//! for this work predicted 9 self-closing; it missed `style`, which entered
//! the table while that brief was being written. Which is the argument for
//! reading the count out of `tags.rs` at test time rather than writing a
//! literal here.

use cena_protocol::{Frame, Parser};

/// A tag whose close (or bare self-close) carries no frame of its own, and
/// the field a consumer reads the effect from instead.
///
/// These are **not** exemptions from the drop-nothing rule -- every one of
/// them still produces a frame, and the test below proves it. They are a
/// record of which tags are *additionally* observable through a neighbouring
/// frame's fields, so a reader can tell "the close is redundant here" from
/// "the close is all we have".
const OBSERVABLE_VIA: &[(&str, &str)] = &[
    ("a", "Text.link becomes None/outer when the scope pops"),
    ("d", "Text.link becomes None/outer when the scope pops"),
    ("preset", "Text.style.preset changes"),
    ("style", "Text.style.preset changes"),
    ("pushBold", "Text.style.bold_depth"),
    ("popBold", "Text.style.bold_depth"),
    ("output", "Text.style.mono"),
    // Measured over 60 stratified corpus files: 234,531 `<b>` blocks, and
    // 234,531 of them wrap a `<pushBold/>...<popBold/>` pair; **zero** `<b>`
    // blocks contain no pushBold. If `<b>` also fed bold_depth every
    // monsterbold creature would read depth 2. `<i>` does not occur at all
    // (0 in those 60 files).
    (
        "b",
        "redundant wrapper: 234531/234531 <b> blocks wrap <pushBold/>",
    ),
    (
        "i",
        "absent from this wire: 0 occurrences in 60 stratified files",
    ),
    ("prompt", "dispatched whole; body is Prompt.text"),
    ("component", "dispatched whole; body is Component.body"),
    ("compDef", "dispatched whole; body is Component.body"),
    ("inv", "dispatched whole; body is ContainerItem.content"),
    ("compass", "dispatched whole; body is Compass.directions"),
    ("spell", "dispatched whole; body is Spell.text"),
    ("left", "dispatched whole; body is LeftHand.item"),
    ("right", "dispatched whole; body is RightHand.item"),
    ("worldEvent", "dispatched whole; body is WorldEvent.text"),
    ("dialogData", "the next ProgressBar.dialog becomes None"),
];

#[test]
fn every_known_tag_is_observable_in_every_form() {
    let tags = known_wire_tags();
    assert_eq!(
        tags.len(),
        cena_protocol::tags::known_count(),
        "the extractor read {} names out of src/tags.rs but the table holds \
         {}. The scan below would be blind to the difference, so it fails \
         here instead.",
        tags.len(),
        cena_protocol::tags::known_count()
    );

    let mut silent = Vec::new();
    for tag in &tags {
        for (form, line) in [
            ("open", format!("<{tag}>X</{tag}>")),
            ("close", format!("</{tag}>")),
            ("selfclose", format!("<{tag}/>")),
        ] {
            let mut parser = Parser::new();
            if parser.parse_line(&line).is_empty() {
                silent.push(format!("  <{tag}> in {form} form"));
            }
        }
    }

    assert!(
        silent.is_empty(),
        "{} tag/form combinations produce NO frame, so a consumer cannot see \
         them at all.\n\nThe author's rule: nothing from the wire is \
         discarded. If the effect is genuinely carried by a neighbouring \
         frame, that is a reason to ALSO record it in OBSERVABLE_VIA -- not a \
         reason to emit nothing, because a tag at end-of-line has no \
         neighbour to carry it. Emit Frame::Structural.\n{}",
        silent.len(),
        silent.join("\n")
    );
}

/// The author's own three reproductions, asserted by name rather than swept
/// into the loop above, because they are what he measured by hand.
#[test]
fn the_authors_reproductions() {
    // `</indicator>` and `</castTime>`: his two named examples of a close
    // form that produced nothing.
    for close in ["</indicator>", "</castTime>", "</roommeta>", "</nav>"] {
        let frames = Parser::new().parse_line(close);
        assert_eq!(
            frames.len(),
            1,
            "{close} must produce exactly one frame, got {frames:?}"
        );
        assert!(
            matches!(&frames[0], Frame::Structural { name, raw }
                if raw == close && close.contains(name.as_str())),
            "{close} should be Frame::Structural carrying its raw bytes, got \
             {:?}",
            frames[0]
        );
    }

    // Bold and italic text. The author measured three Text frames all with
    // bold_depth 0 and read it as "the bold is lost entirely".
    //
    // The text frames are RIGHT to read depth 0 -- `<b>` carries no bold on
    // this wire, `<pushBold/>` does (see OBSERVABLE_VIA). What WAS lost is
    // that a `<b>` element was present at all. Now it is not: each of the
    // four markup tags emits a Structural frame, interleaved with the text.
    let frames = Parser::new().parse_line("<b>bold</b> and <i>ital</i>");
    let names: Vec<&str> = frames
        .iter()
        .filter_map(|f| match f {
            Frame::Structural { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        names,
        ["b", "b", "i", "i"],
        "every <b>/<i> open and close must be recorded, got {frames:?}"
    );
    for frame in &frames {
        if let Frame::Text(text) = frame {
            assert_eq!(
                text.style.bold_depth, 0,
                "<b> must NOT feed bold_depth: 234531/234531 <b> blocks in \
                 the corpus wrap a <pushBold/> pair, so counting both would \
                 report depth 2 on every monsterbold creature. {frame:?}"
            );
        }
    }

    // And the tag that really does carry bold still does.
    let frames = Parser::new().parse_line("<pushBold/>bold<popBold/> plain");
    let depths: Vec<u16> = frames
        .iter()
        .filter_map(|f| match f {
            Frame::Text(t) => Some(t.style.bold_depth),
            _ => None,
        })
        .collect();
    assert_eq!(depths, [1, 0], "pushBold owns bold_depth: {frames:?}");
}

/// A `<resource>` with no `picture=` used to produce nothing at all.
#[test]
fn a_resource_without_a_picture_is_still_seen() {
    let frames = Parser::new().parse_line("<resource/>");
    assert_eq!(frames.len(), 1, "{frames:?}");
    assert!(
        matches!(&frames[0], Frame::Structural { name, .. } if name == "resource"),
        "{frames:?}"
    );
    // The modelled form is unchanged.
    let frames = Parser::new().parse_line("<resource picture='42'/>");
    assert_eq!(frames, vec![Frame::RoomPicture { id: 42 }]);
}

/// Every name in [`OBSERVABLE_VIA`] must still be a tag, and must still be
/// observable. A stale entry reads as a reviewed decision about a tag that no
/// longer exists -- the same failure `architecture.rs:286` guards for statics.
#[test]
fn the_observable_via_table_has_no_stale_entries() {
    for (tag, why) in OBSERVABLE_VIA {
        assert!(
            cena_protocol::tags::is_known(tag),
            "OBSERVABLE_VIA names `{tag}` ({why}) but it is not in \
             KNOWN_WIRE_TAGS any more"
        );
        assert!(
            why.len() > 10,
            "OBSERVABLE_VIA's entry for `{tag}` needs a real field or \
             measurement, not {why:?}"
        );
        assert!(
            !Parser::new().parse_line(&format!("</{tag}>")).is_empty(),
            "`{tag}` is listed in OBSERVABLE_VIA, which records that its \
             effect ALSO shows in a neighbouring frame. It is not a licence \
             to emit nothing."
        );
    }
}

/// The structural filter exists so a renderer does not write it itself.
#[test]
fn structural_frames_can_be_filtered_in_one_call() {
    let frames = Parser::new().parse_line("<b>bold</b><pushStream id='thoughts'/>hi");
    assert!(
        frames.iter().any(Frame::is_structural),
        "expected structural frames in {frames:?}"
    );
    let kept: Vec<&Frame> = frames.iter().filter(|f| !f.is_structural()).collect();
    assert!(
        kept.iter().all(|f| !matches!(f, Frame::Structural { .. })),
        "{kept:?}"
    );
    assert!(
        kept.iter().any(|f| matches!(f, Frame::StreamPush { .. })),
        "the filter must not eat real frames: {kept:?}"
    );
}

/// Recorded proof that this file can go RED (plan/05 §0).
///
/// Deleting the `Structural` push from `markup_tag`'s close arm in
/// `src/parser/markup.rs` -- restoring the `_ => {}` the port had -- and
/// running this suite:
///
/// The `frames.push(Frame::structural(...))` in `markup_tag`
/// (`src/parser/markup.rs`) was replaced with `let _ = frames;` -- restoring
/// the `_ => {}` the port had -- and this suite run:
///
/// ```text
/// $ cargo test -p cena-protocol --test every_tag_is_observable
/// 18 tag/form combinations produce NO frame, so a consumer cannot see them
/// at all.
///   <a> in close form      <a> in selfclose form
///   <b> in close form      <b> in selfclose form
///   <d> in close form      <d> in selfclose form
///   <i> in close form      <i> in selfclose form
///   <output> in close form <output> in selfclose form
///   <popBold> ...          <preset> ...  <pushBold> ...  <style> ...
/// test result: FAILED. 1 passed; 5 failed
/// ```
///
/// Restoring the push returned all six to green. Recorded here rather than
/// re-run, because a test that mutates its own crate's source is worse than
/// the bug it guards.
#[test]
fn structural_arm_deletion_is_caught() {
    // The mechanism the doc comment above describes, asserted directly: a
    // lone markup close tag has no neighbouring frame to be observable in, so
    // its own frame is the only thing standing between it and a silent drop.
    let frames = Parser::new().parse_line("</b>");
    assert_eq!(
        frames,
        vec![Frame::Structural {
            name: "b".to_owned(),
            raw: "</b>".to_owned(),
        }],
        "a bare markup close must still be seen"
    );
}

/// Read `KNOWN_WIRE_TAGS` out of `src/tags.rs` at test time.
///
/// Lexical, the same technique `tags::arms` uses, and for the same reason:
/// making the table `pub` to satisfy a test would widen the crate's API and
/// need a second `ALLOWED_STATICS` entry. `known_count()` is the cross-check
/// that catches an extractor which silently matches nothing.
fn known_wire_tags() -> Vec<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("tags.rs");
    // `panic!` is denied workspace-wide, so every failure below is an
    // `assert!` with the path in its message -- which is what a reader needs
    // anyway when the extractor stops matching.
    let source = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        !source.is_empty(),
        "cannot read {} -- the extractor below would then report zero tags          and the count cross-check would be the only thing catching it",
        path.display()
    );
    let start = source.find("static KNOWN_WIRE_TAGS");
    assert!(start.is_some(), "no KNOWN_WIRE_TAGS in {}", path.display());
    let body = &source[start.unwrap_or_default()..];
    let end = body.find("];");
    assert!(
        end.is_some(),
        "KNOWN_WIRE_TAGS is not terminated in {}",
        path.display()
    );
    body[..end.unwrap_or_default()]
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            line.strip_prefix('"')
                .and_then(|rest| rest.split('"').next())
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
        })
        .collect()
}
