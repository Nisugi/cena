//! **M2 step 1: streams route, so the login dump stops polluting the room.**
//!
//! `Frame::{StreamPush, StreamPop, StreamPopForced, StreamResume, StreamWindow,
//! ClearStream}` all parsed and none was modelled, so every line landed in one
//! undifferentiated place. The author's first live session showed the cost: the
//! services table, the premium notice and the news banner all rendered inline
//! with the room.
//!
//! # The routing rule is the frame's own `stream`, not a mutable cursor
//!
//! `TextFrame::stream` is stamped by the parser on every run
//! (`cena-protocol/src/parser/emit.rs`), so the model needs no `current_stream`
//! to keep in sync. `VellumFE` carries one (`current_stream: String`,
//! `core/app_core/state.rs:67`) and has to fix it up on pop, resume and prompt --
//! three places that can disagree. Reading it off the frame is not a port of
//! that; it is the same routing with the state removed.
//!
//! # What clears a buffer: `clearStream`, NOT `pushStream`
//!
//! This is where Cena diverges from Vellum, and it is measured rather than
//! preferred. Vellum clears `inv` and `reserve` on PUSH
//! (`core/messages/element.rs:451-464`), treating each push as a fresh snapshot.
//!
//! MEASURED over 16 files across 4 characters: of **2,722** `pushStream` tags,
//! **2,721** are immediately preceded by a `clearStream` of the same id. The wire
//! idiom is `streamWindow` (declare) -> `clearStream` (empty) -> `pushStream`
//! (open) -> content -> `popStream` (close).
//!
//! **The single exception is the one that matters**: a `thoughts` push with no
//! clear before it. `thoughts` is a live feed -- ESP chatter accumulating over a
//! session -- so clearing on push would throw away every thought but the newest.
//! Vellum's own code documents the same hazard from the other direction: its
//! `perception` buffer is *"NOT cleared on pushStream ... cleared on clearStream
//! (which comes before all entries)"*, and its `sprite` component is exempted
//! from an unchanged-check because *"the game sends it EMPTY on every room
//! change"*.
//!
//! So honouring `clearStream` gets the snapshot behaviour for `room` and `inv`
//! for free -- the wire sends the clear -- and gets accumulation right for
//! `thoughts` without a per-stream exception list. One rule, no special cases,
//! and it is the wire's own rule.
//!
//! # Census, for scale
//!
//! MEASURED over 24 files across 6 characters:
//!
//! | | distinct | total |
//! |---|---|---|
//! | `pushStream` ids | 6 (`room`, `inv`, `bounty`, `society`, `thoughts`, `charprofile`) | 4,573 |
//! | paired `<stream id=>` ids | 1 (`Spells`) | 353 |
//! | `streamWindow` ids | 16 | 6,344 |
//! | `clearStream` ids | 6 | 4,583 |
//!
//! > **A CORRECTION TO THIS CENSUS, 2026-09-21.** `charprofile` is listed
//! > above among the pushed ids, and `profile`'s own output does **not** arrive
//! > that way. MEASURED over the two `profile` runs in
//! > `E:\Gemstone\dev\lich-5\logs\GSIV-Nisugi`: `exposeStream id="charprofile"` twice,
//! > `pushStream id="charprofile"` **zero times**. The text prints into the
//! > MAIN window; the stream is declared, cleared and exposed, which is a
//! > display instruction about a window.
//! >
//! > The census counted tag ids and was right about what it counted. What was
//! > wrong was reading "this id appears on a pushStream somewhere" as "this
//! > feature's text arrives in that stream" -- and it cost a reader built on an
//! > empty buffer (`character/profile.rs`). Whatever pushes `charprofile` in
//! > the census files, it is not the profile those logs show.
//!
//! **Windows are declared far more widely than they are pushed to** -- 16 against
//! 6 -- so a router must not assume a push for every declared window, and must
//! not create a buffer just because a window was announced.

use cena_model::GameState;
use cena_protocol::Parser;

/// Fold a wire chunk the way the actor does.
fn fold(wire: &[u8]) -> GameState {
    let mut state = GameState::default();
    let mut parser = Parser::new();
    for frame in parser.push_bytes(wire) {
        state.apply(&frame);
    }
    state
}

/// The plain text of one stream's buffer, line by line.
fn lines(state: &GameState, stream: &str) -> Vec<String> {
    state
        .stream(stream)
        .iter()
        .map(cena_protocol::runs::Runs::plain)
        .collect()
}

#[test]
fn text_inside_a_push_lands_in_that_stream_and_not_in_main() {
    let state = fold(
        b"You see a rock.\n<pushStream id='thoughts'/>[OOC] someone: hi\n<popStream/>\nBack here.\n",
    );

    assert_eq!(lines(&state, "thoughts"), vec!["[OOC] someone: hi"]);
    assert_eq!(
        lines(&state, ""),
        vec!["You see a rock.", "Back here."],
        "main must hold only what was outside the push"
    );
}

#[test]
fn an_unknown_stream_is_empty_rather_than_missing() {
    // A caller asking about a stream the session has never seen gets an empty
    // slice, not a panic and not an `Option` to unwrap. There is no difference
    // worth modelling between "declared and empty" and "never mentioned": both
    // render as nothing.
    let state = GameState::default();
    assert!(state.stream("familiar").is_empty());
}

#[test]
fn a_clear_stream_empties_that_buffer_and_no_other() {
    let state = fold(
        b"<pushStream id='inv'/>old item\n<popStream/>\n\
          <pushStream id='thoughts'/>a thought\n<popStream/>\n\
          <clearStream id='inv'/>\n\
          <pushStream id='inv'/>new item\n<popStream/>\n",
    );

    assert_eq!(
        lines(&state, "inv"),
        vec!["new item"],
        "the clear must drop the old snapshot"
    );
    assert_eq!(
        lines(&state, "thoughts"),
        vec!["a thought"],
        "and must not touch a stream it did not name"
    );
}

#[test]
fn a_push_alone_does_not_clear_so_a_live_feed_accumulates() {
    // THE MEASURED EXCEPTION. 2,721 of 2,722 pushes carry a clear; the one that
    // does not is `thoughts`, a live feed. Clearing on push -- which is what
    // VellumFE does for `inv` and `reserve` -- would keep only the newest
    // thought of a session.
    let state = fold(
        b"<pushStream id='thoughts'/>first\n<popStream/>\n\
          <pushStream id='thoughts'/>second\n<popStream/>\n\
          <pushStream id='thoughts'/>third\n<popStream/>\n",
    );

    assert_eq!(
        lines(&state, "thoughts"),
        vec!["first", "second", "third"],
        "a push cleared an accumulating feed; the wire sends clearStream when it \
         wants a snapshot, and it did not send one here"
    );
}

#[test]
fn the_wire_idiom_gives_room_snapshot_semantics_for_free() {
    // `clearStream` + `pushStream` is how the game replaces a room, 3,014 times
    // in the sample. No per-stream rule is needed: honouring the clear IS the
    // snapshot.
    let state = fold(
        b"<clearStream id='room'/>\n<pushStream id='room'/>[Old Room]\n<popStream/>\n\
          <clearStream id='room'/>\n<pushStream id='room'/>[New Room]\n<popStream/>\n",
    );

    assert_eq!(lines(&state, "room"), vec!["[New Room]"]);
}

#[test]
fn a_nested_push_routes_to_the_inner_stream_and_resumes_the_outer() {
    // The `StreamResume` frame earning its keep. Text after the inner pop must
    // return to the enclosing stream, not to main.
    let state = fold(
        b"<pushStream id='inv'/>worn:\n<pushStream id='thoughts'/>interrupt\n<popStream/>\nmore worn\n<popStream/>\n",
    );

    assert_eq!(lines(&state, "inv"), vec!["worn:", "more worn"]);
    assert_eq!(lines(&state, "thoughts"), vec!["interrupt"]);
    assert!(
        lines(&state, "").is_empty(),
        "nothing belonged to main: {:?}",
        lines(&state, "")
    );
}

#[test]
fn a_paired_stream_routes_its_body_and_closes_itself() {
    // `<stream id="Spells">...</stream>` is the only paired form on the wire
    // (353 occurrences, all `Spells`). Each row is self-contained, so the rows
    // accumulate into one buffer rather than replacing each other.
    let state = fold(
        b"<stream id=\"Spells\">Minor Spiritual</stream>\n\
          <stream id=\"Spells\">Major Elemental</stream>\n\
          after\n",
    );

    assert_eq!(
        lines(&state, "Spells"),
        vec!["Minor Spiritual", "Major Elemental"]
    );
    assert_eq!(
        lines(&state, ""),
        vec!["after"],
        "the paired close must return routing to main"
    );
}

#[test]
fn a_prompt_returns_routing_to_main_even_with_no_pop() {
    // The prompt barrier, from the model's side. A stream left open by a missing
    // `popStream` -- 1,530 `inv` pushes against a 1.301 push/pop ratio -- must
    // not swallow the next room.
    let state =
        fold(b"<pushStream id='inv'/>worn items\n<prompt time=\"1\">&gt;</prompt>\n[A Room]\n");

    assert_eq!(lines(&state, "inv"), vec!["worn items"]);
    assert_eq!(
        lines(&state, ""),
        vec!["[A Room]"],
        "the barrier must hand routing back, or one torn stream eats everything \
         after it"
    );
}

#[test]
fn a_stream_window_declares_without_creating_a_buffer() {
    // 16 distinct `streamWindow` ids against 6 pushed. A declaration is layout
    // information -- `cena-ui`'s, at M4 -- and modelling it as content would
    // invent 10 empty buffers per login.
    let state = fold(b"<streamWindow id='familiar' title='Familiar' location='center'/>\n");

    assert!(
        state.stream("familiar").is_empty(),
        "a declared-but-never-pushed window must not become a buffer"
    );
    assert!(
        state.streams().next().is_none(),
        "and must not appear in the stream list at all"
    );
}

#[test]
fn the_streams_are_listed_in_a_stable_order() {
    // Criterion 7: a replay that iterated a `HashMap` would be non-deterministic
    // by construction, which is the argument `Vitals` already makes for being a
    // `BTreeMap`.
    let state = fold(
        b"<pushStream id='thoughts'/>t\n<popStream/>\n\
          <pushStream id='inv'/>i\n<popStream/>\n\
          <pushStream id='bounty'/>b\n<popStream/>\n",
    );
    let ids: Vec<&str> = state.streams().map(|(id, _)| id).collect();
    assert_eq!(
        ids,
        vec!["bounty", "inv", "thoughts"],
        "stream ids must iterate in a deterministic order"
    );
}

#[test]
fn a_reconnect_forgets_every_stream() {
    // `plan/12` 5.2: the login burst re-sends the room and inventory, and
    // anything it does not re-send is unobserved rather than stale. A `thoughts`
    // buffer from the previous session is a different session's chatter.
    let mut state = fold(b"<pushStream id='inv'/>worn\n<popStream/>\n");
    assert!(!state.stream("inv").is_empty());

    state.invalidate_for_reconnect();

    assert!(
        state.streams().next().is_none(),
        "stream buffers survived a reconnect"
    );
}

#[test]
fn an_empty_line_inside_a_stream_is_kept() {
    // An empty line is layout the game chose -- the blank between the worn-items
    // header and the list. Dropping it reflows someone else's formatting.
    let state = fold(b"<pushStream id='inv'/>Your worn items are:\n\nan item\n<popStream/>\n");
    assert_eq!(
        lines(&state, "inv"),
        vec!["Your worn items are:", "", "an item"]
    );
}

#[test]
fn a_line_split_by_markup_is_buffered_as_one_line() {
    // **The test the other twelve did not need.** Every case above uses
    // single-run lines, so all twelve passed with the line assembly GUTTED --
    // review pattern (C) in `plan/19`, a guard sized to the case that was
    // convenient rather than the case that occurs.
    //
    // This is the author's own worn-inventory line from the 2026-09-18 capture.
    // The parser emits one run per markup boundary, so `  a` and
    // `pebbled grey leather doublet` are two frames of ONE displayed line -- and
    // buffering each as its own line is exactly what printed
    //
    //     [inv]   a
    //     [inv] pebbled grey leather doublet
    //
    // down the author's screen.
    let state = fold(
        b"<pushStream id='inv'/>  a <a exist=\"1\" noun=\"doublet\">pebbled grey leather doublet</a>
<popStream/>
",
    );

    assert_eq!(
        lines(&state, "inv"),
        vec!["  a pebbled grey leather doublet"],
        "a line split at a link boundary was buffered as several lines"
    );
}

#[test]
fn two_streams_can_be_mid_line_at_once() {
    // Why the pending line is keyed by stream rather than being a single
    // cursor. A `pushStream` can interrupt an unterminated run, and the
    // enclosing stream has to resume its half-built line afterwards.
    //
    // With one shared pending line, `inv`'s opening fragment and `thoughts`'
    // content would concatenate into whichever stream terminated first.
    let state = fold(
        b"<pushStream id='inv'/>worn: <pushStream id='thoughts'/>a thought
<popStream/>a doublet
<popStream/>
",
    );

    assert_eq!(
        lines(&state, "thoughts"),
        vec!["a thought"],
        "the inner stream's line must not absorb the outer's fragment"
    );
    assert_eq!(
        lines(&state, "inv"),
        vec!["worn: a doublet"],
        "the outer stream must resume the line it had half-built"
    );
}

#[test]
fn a_line_split_across_two_reads_is_still_one_line() {
    // A chunk ending mid-line is ordinary -- `push_bytes` exists to handle it,
    // and `Recorder` preserves the real chunk boundaries so a replay drives this
    // path with a real split rather than an invented one.
    //
    // NOTE: the parser holds an unterminated TAIL itself, so a chunk that ends
    // mid-line emits no frames at all. What reaches the model is the assembled
    // line once the newline arrives -- and it still arrives as several runs,
    // which is what this asserts.
    let mut state = GameState::default();
    let mut parser = Parser::new();
    for chunk in [
        &b"<pushStream id='inv'/>  a <a exist=\"1\" noun=\"d\">pebbled grey "[..],
        &b"leather doublet</a>
<popStream/>
"[..],
    ] {
        for frame in parser.push_bytes(chunk) {
            state.apply(&frame);
        }
    }

    assert_eq!(
        lines(&state, "inv"),
        vec!["  a pebbled grey leather doublet"],
        "a line split across two READS was buffered as more than one line"
    );
}

/// **Stream history is bounded**, which it was not.
///
/// Review finding 6: every completed line was retained forever, including
/// ordinary main-window output. `clearStream` empties a NAMED stream on the
/// game's say-so and the prompt closes the analysis chunk, but nothing ever
/// dropped a line from this history -- so a long session accumulated text,
/// styles and links without limit.
#[test]
fn stream_history_is_capped() {
    let mut parser = cena_protocol::Parser::new();
    let mut state = cena_model::GameState::default();
    let over = cena_model::MAX_STREAM_LINES + 500;
    let mut wire = Vec::new();
    for i in 0..over {
        wire.extend_from_slice(format!("line {i}\n").as_bytes());
    }
    for frame in parser.push_bytes(&wire) {
        state.apply(&frame);
    }

    assert_eq!(
        state.stream("").len(),
        cena_model::MAX_STREAM_LINES,
        "the buffer must stop at its cap"
    );
    assert_eq!(
        state.lines_seen(),
        over as u64,
        "and every line was still FED to the model"
    );
    assert_eq!(state.lines_dropped(), 500, "reported, not silent");
}

/// The cap drops the OLDEST line, keeping recent history.
///
/// Same rule as `chunks.rs`: the recent lines are the ones a reader wants, and
/// a scrollback that forgets its beginning is a scrollback rather than a leak.
#[test]
fn the_cap_drops_the_oldest_line() {
    let mut parser = cena_protocol::Parser::new();
    let mut state = cena_model::GameState::default();
    let mut wire = Vec::new();
    for i in 0..=cena_model::MAX_STREAM_LINES {
        wire.extend_from_slice(format!("line {i}\n").as_bytes());
    }
    for frame in parser.push_bytes(&wire) {
        state.apply(&frame);
    }

    let first = state
        .stream("")
        .first()
        .map(cena_protocol::runs::Runs::plain);
    assert_eq!(
        first.as_deref(),
        Some("line 1"),
        "line 0 was dropped, line 1 is now the oldest"
    );
}
