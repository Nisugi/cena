//! What a chunk line carries: the parser's runs, links included.
//!
//! **The regression this guards.** Until 2026-09-20 a `ChunkLine` was
//! `{ text, bold }` and dropped every `Link` at construction
//! (`state/streams.rs`, the `push_line` call). `chunks.rs`'s own module doc
//! claimed *"a chunk here holds parsed lines and no consumer needs a second
//! parser"* -- true of the pipeline, false of the type. The `info` reader never
//! noticed because a stat line has no links; the combat port would have on its
//! first line, because a swing's target is a link and nothing else.
//!
//! The wire below is a real attack shape, and the pronoun form is the one
//! `plan/12` §3a verified: a link is how the game says *who*.

use cena_model::GameState;
use cena_model::state::chunks::ChunkLine;
use cena_protocol::Parser;
use cena_protocol::frame::LinkKind;

/// Fold wire bytes with NO prompt, so the chunk stays open and observable.
fn open_chunk_after(wire: &[u8]) -> GameState {
    let mut parser = Parser::new();
    let mut state = GameState::default();
    for frame in parser.push_bytes(wire) {
        state.apply(&frame);
    }
    state
}

/// **A line keeps its links.**
///
/// A swing at a creature: the target arrives as `<pushBold/>a <a exist=
/// noun=>kobold</a><popBold/>`, and the only fact that says *which* kobold is
/// the link's `exist`. It must reach the chunk.
#[test]
fn a_chunk_line_keeps_its_links() {
    let state = open_chunk_after(
        b"You swing a sword at <pushBold/>a <a exist=\"1\" noun=\"kobold\">kobold</a><popBold/>!\n",
    );
    let lines = state.open_chunk().lines();
    assert_eq!(lines.len(), 1, "guard: one line accumulated, no prompt");

    let found: Vec<_> = lines[0].links().collect();
    assert_eq!(found.len(), 1, "exactly one link on the line: {found:?}");
    assert_eq!(found[0].text, "kobold");
    assert_eq!(
        found[0].kind,
        LinkKind::Exist {
            id: "1".to_owned(),
            noun: "kobold".to_owned(),
        },
        "the id and noun are the link's, not re-parsed from text"
    );
}

/// The text and bold the `info` reader depends on still derive correctly.
///
/// The old type stored these; the new one computes them. Same answers, or the
/// `info` and `skill` readers regress silently.
#[test]
fn text_and_bold_derive_from_the_runs() {
    let state = open_chunk_after(
        b"You swing a sword at <pushBold/>a <a exist=\"1\" noun=\"kobold\">kobold</a><popBold/>!\n",
    );
    let line = &state.open_chunk().lines()[0];

    assert_eq!(line.text(), "You swing a sword at a kobold!");
    assert_eq!(
        line.bold().concat(),
        "a kobold",
        "the bolded span, whatever the parser split it into: {:?}",
        line.bold()
    );
    assert_eq!(line.bold_refs().concat(), "a kobold");
}

/// A pronoun link resolves to the creature's own id.
///
/// `plan/12` §3a: *"The pronoun `her` carries the creature's own `exist`, so it
/// resolves without a heuristic -- Lich needed a fix for exactly this after a
/// 2026-09-07 hunt log recorded an attacker as 'his'."* This is that fact
/// reaching the chunk.
#[test]
fn a_pronoun_link_carries_the_creatures_id() {
    let state = open_chunk_after(
        b"The briar wraps itself around <pushBold/><a exist=\"340826187\" noun=\"skald\">her</a><popBold/>.\n",
    );
    let line = &state.open_chunk().lines()[0];
    let ids: Vec<&str> = line
        .links()
        .filter_map(|l| match &l.kind {
            LinkKind::Exist { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(ids, ["340826187"], "`her` is a link to the skald");
}

/// A plain line, built for tests, has no links and no bold.
#[test]
fn a_plain_line_has_no_links() {
    let line = ChunkLine::plain("A kobold arrives.");
    assert_eq!(line.text(), "A kobold arrives.");
    assert!(line.bold().is_empty());
    assert_eq!(line.links().count(), 0);
}

/// Only the main stream reaches the chunk, as before.
///
/// The link fix must not have loosened the stream gate: a linked line on the
/// `thoughts` stream is still someone else's words.
#[test]
fn a_linked_line_on_another_stream_still_stays_out() {
    let state = open_chunk_after(
        b"<pushStream id='thoughts'/>You swing at <a exist=\"9\" noun=\"x\">x</a>!<popStream/>\n",
    );
    assert_eq!(
        state.open_chunk().lines().len(),
        0,
        "a `thoughts` line must not enter the chunk, links or no links"
    );
}
