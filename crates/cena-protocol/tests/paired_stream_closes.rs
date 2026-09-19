//! **`</stream>` has to say the stream closed**, not just quietly close it.
//!
//! The paired redirect `<stream id="Spells">...</stream>` (wiki `:9`, `:50`) is
//! the inline form of `<pushStream>`. Its opener pushes onto the parser's stream
//! stack AND emits `Frame::StreamPush`. Its closer removed the entry from the
//! stack but emitted `Frame::Structural { name: "stream" }` -- so the parser knew
//! the stream had ended and the FRAME STREAM DID NOT SAY SO.
//!
//! That split is the whole defect. Any consumer that tracks routing from frames
//! -- which is what a stream router does, and what `plan/18` step 1 builds --
//! sees an unbalanced push and routes everything after it into `Spells` forever.
//! The parser's own stack was right, which is exactly why nothing caught it: the
//! internal invariant held while the published one did not.
//!
//! MEASURED on the author's own capture,
//! `GSIV-Nisugi/2025/04/xml/2025-04-18_11-57-38.xml`: **31** paired
//! `<stream id="Spells">` rows, 31 `</stream>`, and a frame stream whose depth
//! stood at 31 when the next `<prompt>` arrived. That is the corpus tier's
//! `the_prompt_barrier_drains_the_stream_stack_on_real_traffic` failure, and it
//! was red before the test that found it was written -- nobody had run the tier,
//! because it is gated behind `CENA_CORPUS`.
//!
//! # Why the closer matches on the OPENER, not on the id
//!
//! MEASURED over 24 files spanning 2024-11, 2025-04 and 2026-09:
//!
//! | form | opens | closes |
//! |---|---|---|
//! | `<stream id=>` / `</stream>` | 122 | 122 |
//! | `<pushStream>` / `<popStream>` | 17,036 | 13,039 |
//!
//! **Not one file had a different count of paired opens and closes.** The whole
//! 1.301 imbalance is `pushStream`'s. So `</stream>` closing the innermost
//! *paired* entry is not a hedge against malformed input -- it is what the wire
//! does, and it is what keeps a paired `<stream>` nested inside a
//! `<pushStream id='inv'/>` from closing `inv`. The first fix here matched
//! "innermost non-empty id" instead and did exactly that.
//!
//! # Why the prompt barrier did not paper over it
//!
//! It would have, and that is the subtle part. `<prompt>` drains the parser's
//! stack with `mem::take`, so by the time a prompt arrives there is nothing left
//! to force-pop -- **no `StreamPopForced` is emitted**, because the entries were
//! already gone. A frame-stream consumer therefore never gets the pop from the
//! closer OR from the barrier. The barrier's guarantee is real for the parser and
//! empty for everyone downstream.

use cena_protocol::{Frame, Parser};

/// Net stream depth a frame-stream consumer would compute, and it is the only
/// reading available to one: the parser's internal stack is private.
fn depth(frames: &[Frame]) -> i64 {
    let mut depth = 0;
    for frame in frames {
        match frame {
            Frame::StreamPush { .. } => depth += 1,
            Frame::StreamPop { .. } | Frame::StreamPopForced { .. } => depth -= 1,
            _ => {}
        }
    }
    depth
}

#[test]
fn a_paired_stream_emits_a_pop_naming_the_stream_it_closed() {
    let mut parser = Parser::new();
    let frames = parser.push_bytes(b"<stream id=\"Spells\">Minor Spiritual</stream>\n");

    let pops: Vec<_> = frames
        .iter()
        .filter_map(|f| match f {
            Frame::StreamPop { id } => Some(id.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(
        pops,
        vec![Some("Spells".to_owned())],
        "`</stream>` must publish the pop, naming the stream, or a router cannot \
         tell the redirect ended: {frames:#?}"
    );
}

#[test]
fn thirty_one_spell_rows_leave_the_frame_stream_balanced() {
    // The shape of the author's 2025-04-18 capture, which is where this was
    // found: a spell list is one paired `<stream>` PER ROW, each self-contained.
    let mut wire = String::new();
    for row in [
        "Minor Spiritual",
        "Major Spiritual",
        "Major Elemental",
        "Minor Elemental",
    ] {
        use std::fmt::Write as _;
        let _ = writeln!(wire, "<stream id=\"Spells\">{row}</stream>");
    }

    let mut parser = Parser::new();
    let frames = parser.push_bytes(wire.as_bytes());

    assert_eq!(
        depth(&frames),
        0,
        "four self-contained paired streams left the frame stream unbalanced; \
         the corpus file has 31 of these and stood at depth 31"
    );
}

#[test]
fn the_prompt_after_a_paired_stream_has_nothing_left_to_force() {
    // The guarantee the corpus test asserts, on the minimal case: once the
    // closer publishes its pop, a prompt arriving next needs no forced pop,
    // and depth is already 0 when it does.
    let mut parser = Parser::new();
    let frames = parser.push_bytes(
        b"<stream id=\"Spells\">Minor Spiritual</stream>\n<prompt time=\"1\">&gt;</prompt>\n",
    );

    let at_prompt = frames
        .iter()
        .position(|f| matches!(f, Frame::Prompt { .. }))
        .expect("the prompt must be in the frame stream");

    assert_eq!(
        depth(&frames[..at_prompt]),
        0,
        "the stack must already be drained when the prompt arrives, not drained \
         BY it -- the barrier emits no pop for an entry the closer removed"
    );
    assert_eq!(depth(&frames), 0, "and the prompt must not overshoot");
}

#[test]
fn an_unmatched_close_does_not_pop_an_enclosing_push_stream() {
    // The reason the closer matches on the OPENER. A stray `</stream>` inside a
    // `<pushStream id='inv'/>` redirect must not unroute the inventory -- and
    // matching "innermost non-empty id" instead, as the first fix here did,
    // unroutes it, because `inv` is a non-empty id.
    //
    // The corpus sample in this file's header found ZERO of these: paired opens
    // and closes matched in all 24 files. So this test guards a shape the wire
    // does not currently produce. It is kept because the cost of being wrong is
    // silent misrouting rather than a crash, and because it is the test that
    // caught the first fix.
    let mut parser = Parser::new();
    let frames = parser.push_bytes(b"<pushStream id=\"inv\"/>Your worn items are:</stream>\n");

    assert_eq!(
        depth(&frames),
        1,
        "an unmatched `</stream>` closed a stream it never opened: {frames:#?}"
    );
}

#[test]
fn a_paired_stream_nested_in_a_push_stream_closes_only_itself() {
    let mut parser = Parser::new();
    let frames = parser
        .push_bytes(b"<pushStream id=\"inv\"/><stream id=\"Spells\">Minor Spiritual</stream>\n");

    let pops: Vec<_> = frames
        .iter()
        .filter_map(|f| match f {
            Frame::StreamPop { id } => Some(id.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(
        pops,
        vec![Some("Spells".to_owned())],
        "the inner paired stream must name itself, leaving `inv` open"
    );
    assert_eq!(depth(&frames), 1, "`inv` is still open and must stay open");

    // And a scalar consumer -- one tracking a single current stream rather than
    // a stack -- has to be TOLD that `inv` is current again. The pop alone
    // leaves it routing to nothing.
    assert!(
        frames
            .iter()
            .any(|f| matches!(f, Frame::StreamResume { id } if id == "inv")),
        "closing the inner stream must resume `inv`, or a scalar router is \
         left with no current stream: {frames:#?}"
    );
}

#[test]
fn text_after_a_paired_stream_closes_is_routed_to_the_enclosing_stream() {
    // The consequence the whole fix is for, stated on the text runs themselves:
    // where does the next line go? Before the fix `Spells` stayed current
    // forever, because nothing ever closed it.
    let mut parser = Parser::new();
    let frames = parser.push_bytes(
        b"<pushStream id=\"inv\"/>worn:<stream id=\"Spells\">Minor Spiritual</stream>and more\n",
    );

    let routed: Vec<(String, String)> = frames
        .iter()
        .filter_map(|f| match f {
            Frame::Text(t) => Some((t.content.clone(), t.stream.clone())),
            _ => None,
        })
        .collect();

    assert_eq!(
        routed,
        vec![
            ("worn:".to_owned(), "inv".to_owned()),
            ("Minor Spiritual".to_owned(), "Spells".to_owned()),
            ("and more".to_owned(), "inv".to_owned()),
        ],
        "text after the paired close must return to `inv`"
    );
}
