//! **A frame boundary is not a line boundary**, and a consumer has to be able
//! to tell them apart.
//!
//! # The two bugs this sits between
//!
//! The parser emits one text run per markup boundary, and `push_bytes` strips
//! the newline. So a consumer has neither fact for free, and both naive readings
//! were shipped:
//!
//! * **one line per frame** printed the author's worn inventory as
//!   `  a` / `pebbled grey leather doublet`, because an `<a exist=...>` link sits
//!   between those two runs;
//! * **accumulate until a newline** joined every line in the room together,
//!   because no newline ever reaches a frame.
//!
//! `TextFrame::ends_line` is the missing fact. `VellumFE` never needed it: its
//! public API is `parse_line`, so its caller splits with `data.lines()` and knows
//! every boundary implicitly (`src/core/app_core/state.rs:1472`). Cena moved the
//! split inside `push_bytes` -- which is what lets it handle a chunk ending
//! mid-line -- and this restores what that hid.

use cena_protocol::{Frame, Parser};

/// Rebuild display lines the way a terminal would, using only `ends_line`.
fn display_lines(chunk: &[u8]) -> Vec<String> {
    let mut parser = Parser::new();
    let mut lines = Vec::new();
    let mut pending = String::new();
    for frame in parser.push_bytes(chunk) {
        if let Frame::Text(text) = frame {
            pending.push_str(&text.content);
            if text.ends_line {
                lines.push(std::mem::take(&mut pending));
            }
        }
    }
    if !pending.is_empty() {
        lines.push(pending);
    }
    lines
}

/// **The author's own inventory line**, from the live capture of 2026-09-18.
///
/// The leading text and the link text are separate runs on one wire line.
#[test]
fn a_line_split_by_a_link_reassembles_as_one_line() {
    let wire = b"  a <a exist=\"347174333\" noun=\"doublet\">pebbled grey leather doublet</a>\n";
    assert_eq!(
        display_lines(wire),
        vec!["  a pebbled grey leather doublet".to_owned()],
        "the link boundary is not a line boundary -- this printed as two lines"
    );
}

/// **And separate wire lines stay separate**, which is the other failure.
#[test]
fn separate_wire_lines_do_not_join() {
    let wire = b"You swing at the kobold!\nYou miss.\n";
    assert_eq!(
        display_lines(wire),
        vec![
            "You swing at the kobold!".to_owned(),
            "You miss.".to_owned()
        ],
        "these joined into `You swing at the kobold!You miss.` when the printer \
         waited for a newline that push_bytes had already stripped"
    );
}

/// Only the LAST run of a line is marked, so a line with several runs produces
/// one boundary rather than one per run.
#[test]
fn only_the_last_run_of_a_line_ends_it() {
    let mut parser = Parser::new();
    let frames = parser.push_bytes(
        b"one <a exist=\"1\" noun=\"x\">two</a> three <a exist=\"2\" noun=\"y\">four</a>\n",
    );
    let flags: Vec<bool> = frames
        .iter()
        .filter_map(|frame| match frame {
            Frame::Text(text) => Some(text.ends_line),
            _ => None,
        })
        .collect();

    assert!(
        flags.len() > 1,
        "the fixture must yield several runs, or this asserts nothing: {flags:?}"
    );
    assert_eq!(
        flags.iter().filter(|ends| **ends).count(),
        1,
        "exactly one run ends the line: {flags:?}"
    );
    assert!(
        *flags.last().expect("non-empty"),
        "...and it is the last one: {flags:?}"
    );
}

/// **A chunk that ends mid-line marks nothing**, because the line is not over.
///
/// This is the case `push_bytes` exists for and the one a per-call marker would
/// get wrong: the runs from the first chunk must stay unterminated until the
/// newline actually arrives in the second.
#[test]
fn a_line_split_across_two_chunks_ends_once() {
    let mut parser = Parser::new();
    let first = parser.push_bytes(b"You see a ");
    let second = parser.push_bytes(b"kobold.\n");

    let ended: usize = first
        .iter()
        .chain(second.iter())
        .filter(|frame| matches!(frame, Frame::Text(text) if text.ends_line))
        .count();
    assert_eq!(
        ended, 1,
        "one wire line, one boundary -- even though it arrived in two reads"
    );

    // And the text itself must reassemble.
    let joined: String = first
        .iter()
        .chain(second.iter())
        .filter_map(|frame| match frame {
            Frame::Text(text) => Some(text.content.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(joined, "You see a kobold.");
}

/// A line that produces no display text marks nothing -- there is no run for a
/// printer to terminate, and inventing one would emit a blank line per prompt.
#[test]
fn a_line_with_no_text_marks_nothing() {
    let mut parser = Parser::new();
    let frames = parser.push_bytes(b"<prompt time=\"1789775900\">&gt;</prompt>\n");
    assert!(
        !frames
            .iter()
            .any(|frame| matches!(frame, Frame::Text(text) if text.ends_line)),
        "a bare prompt contributes no text run, so nothing is marked"
    );
}
