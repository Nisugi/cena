//! **`unknown_tags` was unbounded on the production path.**
//!
//! Criterion 8 requires an unmodelled tag to **survive to display** (`plan/12`
//! §8, Rule 2.2), so `GameState` keeps them rather than counting and dropping.
//! It kept them with a bare `Vec::push`, forever, across every reconnect, and
//! deep-cloned the whole thing on every `subscribe()`
//! (`supervisor.rs:175`, `actor/handle.rs:145`).
//!
//! That is the same shape as the `Recorder` leak, which was an append-only `Vec`
//! copying every chunk in a client designed for 3-25 simultaneous characters.
//!
//! # It is latent, not live, and that is why the bound is generous
//!
//! MEASURED 2026-09-19 over a 3 MB capture (`GSIV-Getho/2025-09-04_00-27-11`):
//! **zero** unknown tags. The tag table covers everything the wire currently
//! sends, so nothing is growing today.
//!
//! The failure needs Simutronics to add one tag that rides every prompt --
//! which is exactly the event Rule 2.2 exists for, and the one where a client
//! must keep working. At ~86,000 prompts a day per session that is ~86K entries,
//! each holding the tag's raw bytes, cloned per subscriber, times 25 characters.
//!
//! So the bound is not a trade against a cost we are paying. It is a ceiling on
//! a cost we are not paying yet, sized so that no realistic session reaches it.
//!
//! # What is kept when it fills
//!
//! The **first** occurrences, not the most recent. An unknown tag is a
//! diagnostic, and the interesting fact is *that a new tag appeared* and what it
//! looked like -- the ten-thousandth copy of the same tag teaches nothing the
//! first one did not. `Recorder` keeps the newest because it is a transcript
//! being replayed; this keeps the oldest because it is a census.
//!
//! The per-name counts keep rising after the ring fills, so "how often" is never
//! lost even though "every instance" is.

use cena_model::GameState;
use cena_protocol::Parser;

fn fold(wire: &[u8]) -> GameState {
    let mut state = GameState::default();
    let mut parser = Parser::new();
    for frame in parser.push_bytes(wire) {
        state.apply(&frame);
    }
    state
}

/// `n` occurrences of an unmodelled tag, as the wire would send them.
fn unknown_tags(n: usize) -> Vec<u8> {
    use std::fmt::Write as _;
    let mut wire = String::new();
    for i in 0..n {
        let _ = writeln!(wire, "<newThing seq='{i}'/>");
    }
    wire.into_bytes()
}

#[test]
fn an_unknown_tag_still_survives_to_display() {
    // Criterion 8 first: the bound must not become an excuse to drop the thing
    // the field exists to keep.
    let state = fold(b"<newThing id='1'/>\n");
    assert_eq!(state.unknown_tags.len(), 1);
    assert_eq!(state.unknown_tags[0].name, "newThing");
    assert!(
        state.unknown_tags[0].raw.contains("id='1'"),
        "the raw bytes ARE the diagnostic and must survive verbatim"
    );
}

#[test]
fn the_list_stops_growing_at_the_bound() {
    let state = fold(&unknown_tags(cena_model::MAX_UNKNOWN_TAGS * 3));
    assert_eq!(
        state.unknown_tags.len(),
        cena_model::MAX_UNKNOWN_TAGS,
        "the list grew past its bound; this is the production path and a tag \
         that rides every prompt would grow it all day"
    );
}

#[test]
fn the_first_occurrences_are_the_ones_kept() {
    // A census, not a transcript. The first sighting of a tag is the diagnostic;
    // the ten-thousandth copy teaches nothing new.
    let state = fold(&unknown_tags(cena_model::MAX_UNKNOWN_TAGS * 3));
    assert!(
        state.unknown_tags[0].raw.contains("seq='0'"),
        "the FIRST sighting was evicted: {:?}",
        state.unknown_tags[0].raw
    );
}

#[test]
fn the_counts_keep_rising_after_the_list_is_full() {
    // "How often" must survive even when "every instance" does not -- otherwise
    // the bound silently turns a rate into a fixed number.
    let n = cena_model::MAX_UNKNOWN_TAGS * 3;
    let state = fold(&unknown_tags(n));
    assert_eq!(
        state.unknown_tag_count("newThing"),
        n as u64,
        "the per-name count stopped at the bound, so a tag on every prompt \
         would look no more common than a tag seen twice"
    );
}

#[test]
fn a_name_never_seen_counts_zero() {
    let state = fold(b"<newThing/>\n");
    assert_eq!(state.unknown_tag_count("somethingElse"), 0);
}

#[test]
fn several_names_are_counted_separately() {
    let state = fold(b"<alpha/>\n<beta/>\n<alpha/>\n");
    assert_eq!(state.unknown_tag_count("alpha"), 2);
    assert_eq!(state.unknown_tag_count("beta"), 1);
}

#[test]
fn a_reconnect_keeps_them_because_they_are_about_the_protocol() {
    // Deliberately RETAINED, unlike everything else `invalidate_for_reconnect`
    // touches. `reconnect.rs` states the reason: an unmodelled tag is a fact
    // about the PROTOCOL, not about the character, and it is most useful across
    // a reconnect rather than least. Bounding it is what makes keeping it safe.
    let mut state = fold(b"<newThing/>\n");
    assert_eq!(state.unknown_tags.len(), 1);

    state.invalidate_for_reconnect();

    assert_eq!(
        state.unknown_tags.len(),
        1,
        "criterion 8's evidence was discarded on reconnect"
    );
    assert_eq!(state.unknown_tag_count("newThing"), 1);
}
