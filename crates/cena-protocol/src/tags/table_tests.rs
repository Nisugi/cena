//! Tests for the wire-tag table.
//!
//! Split from `tags.rs` under Rule 4.1 (`plan/05:352-353`) -- move code down,
//! do not raise the cap. Beside `arms.rs`, which is the same split for the
//! handler-arm coverage test.

use super::*;

#[test]
fn the_table_is_sorted_and_has_no_duplicates() {
    // Goes RED on a misplaced insert: appending "aaa" fails on the first
    // pair that straddles it. Strict `<` also catches a duplicate.
    for pair in KNOWN_WIRE_TAGS.windows(2) {
        assert!(
            pair[0] < pair[1],
            "KNOWN_WIRE_TAGS must stay ASCII-sorted and distinct: {:?} \
             does not precede {:?}. is_known() binary-searches this table, \
             so a mis-sort silently makes OTHER tags report unknown and \
             spray into the user's text stream.",
            pair[0],
            pair[1]
        );
    }
}

#[test]
fn the_table_holds_the_reference_116_plus_the_ten_added_since() {
    // 116 measured twice from the reference, plus five the Tier 2 replay
    // found in real traffic, plus two that
    // `every_tag_with_a_handler_arm_is_in_the_table` found had a handler
    // here and no table entry, plus three Saga 0.9.9 names. CLAUDE.md's
    // "~130" and the task brief's 117 are both wrong about the reference.
    assert_eq!(
        known_count(),
        126,
        "the table is the reference's 116 tags plus c, map, mapInfo, \
         streamBox and FEStart from the corpus, plus style and \
         clearDialogData from the handler-arm cross-check, plus \
         closeContainers, room and task from Saga 0.9.9. If a tag was \
         deliberately added or removed, change this number in the same \
         commit and say why."
    );
    for (tag, why) in [
        (
            "c",
            "the player's own typed command; 376 in one corpus file",
        ),
        ("map", "the automapper dialog, plain server output"),
        ("mapInfo", "automapper position and zoom"),
        ("streamBox", "a text box inside a dialog"),
        (
            "FEStart",
            "session start, sibling of the FEVersion already listed",
        ),
        (
            "style",
            "the room-name/room-desc preset switch. 338,516 occurrences in                  a 272-file sample, handled here as markup since the first                  commit, and absent from the reference's table too -- an                  inherited gap, found by the handler-arm cross-check rather                  than by the corpus replay, which cannot see a tag that has                  its own dispatch arm",
        ),
        (
            "clearDialogData",
            "has a dispatch arm and had no entry; absent from a 272-file                  sample, so listed on the handler's authority rather than the                  corpus's",
        ),
        (
            "closeContainers",
            "named by Saga 0.9.1 and 0.9.9; 0 corpus hits, so listed on                  Simutronics' own client's authority",
        ),
        (
            "room",
            "named by Saga 0.9.1 and 0.9.9; 0 corpus hits. Distinct from                  roomDesc",
        ),
        (
            "task",
            "added by Saga between 0.9.1 and 0.9.9 with action and                  reward; an <objective> child carrying quest progress",
        ),
    ] {
        assert!(is_known(tag), "`{tag}` is {why}");
    }
}

#[test]
fn every_tag_with_a_handler_arm_is_in_the_table() {
    use super::arms::{include_str_of_sibling, match_arm_names};
    // The table and the dispatch match arms are TWO SPELLINGS OF ONE
    // FACT, and nothing kept them in step. An attack on this crate's
    // tests deleted `c`, `map`, `mapInfo`, `castTime`, `nav`, `roundTime`
    // and `indicator` from the table one at a time and the gated corpus
    // replay stayed green on all seven.
    //
    // The mechanism: the replay only counts
    // `UnknownTag { name } if !is_known(name)`, but `is_known` is
    // consulted at the END of dispatch, after every explicit name arm. A
    // tag with its own arm never reaches the check, so its table entry is
    // dead weight the replay cannot see -- and 87 of the entries have
    // such an arm.
    //
    // This test closes that by reading the arms out of the source. It is
    // lexical, which is the same known limit the architecture tests
    // record: a name built by a macro or assembled from pieces is
    // invisible. That is acceptable here because the failure it guards --
    // someone edits the table and not the dispatcher, or vice versa -- is
    // a hand edit to a literal, which is exactly what a literal scan sees.
    //
    // Goes RED on deleting any of those seven entries (VERIFIED).
    // **Every file holding a name arm, not just the two it started with.**
    //
    // `markup.rs` was split OUT of `dispatch.rs` and took `a`, `d`, `preset`,
    // `style`, `pushBold`, `popBold` and `output` with it; `parser.rs` holds
    // `is_paired`. Neither was added here, so this cross-check -- the test
    // that found the `style` gap back when those arms lived in `dispatch.rs`
    // -- had been blind to them ever since (review PR-6).
    //
    // A split moves code out from under a lexical scan without changing a
    // line of it. Nothing fails; the scan simply stops seeing it. When a file
    // in this list is split, its children belong here.
    let sources = [
        include_str_of_sibling("parser/dispatch.rs"),
        include_str_of_sibling("parser/thin.rs"),
        include_str_of_sibling("parser/markup.rs"),
        include_str_of_sibling("parser.rs"),
    ];
    let mut missing = Vec::new();
    for source in &sources {
        for name in match_arm_names(source) {
            if !is_known(&name) {
                missing.push(name);
            }
        }
    }
    missing.sort_unstable();
    missing.dedup();
    assert!(
        missing.is_empty(),
        "these tag names have a dispatch arm but are absent from              KNOWN_WIRE_TAGS, so `is_known` disagrees with what the parser              actually handles: {missing:?}"
    );
}

#[test]
fn membership_works_at_both_ends_and_rejects_novelty() {
    // Ends, because a binary search that is off by one fails there first.
    assert!(is_known("FEVersion"));
    assert!(is_known("worldEvent"));
    assert!(is_known("prompt"));
    assert!(is_known("compDef"));
    // Case matters: the wire sends both `closeDialog` and `closedialog`
    // and they are separate entries.
    assert!(is_known("closeDialog"));
    assert!(is_known("closedialog"));
    assert!(!is_known("unknownFutureTag"));
    assert!(!is_known(""));
    // The client-injected tags Cena does not speak. If one ever arrives it
    // must reach Frame::UnknownTag, not a handler.
    assert!(!is_known("vellumImg"));
    assert!(!is_known("vellumCmd"));
    assert!(!is_known("vellumTimer"));
}
