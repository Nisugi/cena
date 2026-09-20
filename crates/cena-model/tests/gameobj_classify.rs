//! Type and sellable classification, over the committed table.
//!
//! `tests/gameobj_patterns.rs` already asserts every pattern **compiles**.
//! This asserts they **classify**, which is the part a caller depends on.
//!
//! # The objects are real
//!
//! The wire cases below are `noun`/`name` pairs taken from a live capture
//! (`cena_logs/2026-09-19/Nisugi-...bytes`) -- the author's own worn
//! inventory. A hand-invented name would have matched whatever pattern I had
//! in mind while writing it, which is how a classifier passes its tests and
//! fails on the wire.

use cena_model::state::gameobj::{Classification, ObjectTypes, categories, classify};

/// A noun-keyed category matches on the noun alone.
///
/// `lockpick`'s whole pattern is `^(lockpick)$` against the noun; the display
/// name is arbitrary.
#[test]
fn a_noun_keyed_category_matches_on_the_noun() {
    let types = classify("lockpick", "a heavy steel lockpick");
    assert!(types.is("lockpick"), "got {:?}", types.types);
}

/// Real objects from a live capture classify.
///
/// Not every object has a type -- most worn gear does not -- so this asserts
/// the ones the table covers and records the rest as deliberately
/// unclassified.
#[test]
fn real_worn_items_classify() {
    // Jewelry keys on the noun.
    let bracelet = classify("bracelet", "briar and thorn wrotwood bracelet");
    assert!(bracelet.is("jewelry"), "got {:?}", bracelet.types);

    let necklace = classify("necklace", "cloth necklace");
    assert!(necklace.is("jewelry"));

    let anklet = classify("anklet", "spiderweb-patterned nightshade anklet");
    assert!(anklet.is("jewelry"));

    // A bow is a weapon by noun.
    let bow = classify("bow", "scorched glowbark long bow");
    assert!(
        !bow.is_unclassified(),
        "a long bow should classify as something: {bow:?}"
    );
}

/// **An object can be several types at once.**
///
/// `matching_data_keys` selects every matching key rather than the first, and
/// Lich joins them with commas (`gameobj.rb:263`). Returning the set directly
/// is the same answer without the round trip through a string.
#[test]
fn an_object_can_hold_several_types() {
    // `doomstone` is in both `cursed` and `gem`.
    let types = classify("doomstone", "a jagged doomstone");
    assert!(types.is("cursed"), "got {:?}", types.types);
    assert!(types.is("gem"), "got {:?}", types.types);
    assert!(
        types.types.len() >= 2,
        "the set is the point: {:?}",
        types.types
    );
}

/// **The exclude veto reads the NAME even when the noun matched.**
///
/// `gameobj.rb:1509` tests `@name =~ entry[:exclude]` unconditionally, and the
/// data relies on it: `alchemy equipment` matches the noun `mortar` and
/// excludes the name `small blue clay mortar`.
///
/// This looks like an oversight in Lich and is not -- a veto that only applied
/// to name-matches would let the excluded mortar through.
#[test]
fn the_exclude_veto_reads_the_name_after_a_noun_match() {
    let ordinary = classify("mortar", "a granite mortar");
    assert!(ordinary.is("alchemy equipment"), "got {:?}", ordinary.types);

    let excluded = classify("mortar", "small blue clay mortar");
    assert!(
        !excluded.is("alchemy equipment"),
        "the exclude must veto a noun match too: {:?}",
        excluded.types
    );
}

/// An unknown thing classifies as nothing, rather than guessing.
#[test]
fn an_unknown_object_is_unclassified() {
    let types = classify("widget", "an entirely fictional widget");
    assert!(types.is_unclassified(), "got {types:?}");
    assert!(!types.is("gem"));
    assert!(!types.sells_to("gemshop"));
}

/// Sellable categories are separate from types.
///
/// An object can be a `skin` type and sell to the `furrier`, and a caller
/// asking "what is it" and "who buys it" wants different answers.
#[test]
fn sellable_is_its_own_classification() {
    let type_names = categories(Classification::Type);
    let sellable_names = categories(Classification::Sellable);

    assert!(!type_names.is_empty(), "the table declares types");
    assert!(!sellable_names.is_empty(), "and sellable categories");
    assert!(sellable_names.contains("furrier"), "got {sellable_names:?}");
}

/// The table loads without skipping anything unexpected.
///
/// **Two rows are skipped deliberately** -- the `suffix` field, which
/// `matching_data_keys` never reads (`gameobj.rb:1504-1512`) and which the
/// data declares twice, for `furrier` and `skin`.
///
/// Anything beyond those two means a pattern failed to compile under this
/// crate's regex dialect, and classification is running against a partial
/// table. Asserted rather than logged, because a silently partial table gives
/// confident wrong answers.
#[test]
fn only_the_dead_suffix_rows_are_skipped() {
    assert_eq!(
        ObjectTypes::skipped(),
        2,
        "expected exactly the two `suffix` rows; more means a pattern did \
         not compile and the table is partial"
    );
}

/// Every category the table declares is reachable.
///
/// A category whose only pattern failed to compile would vanish silently, and
/// `only_the_dead_suffix_rows_are_skipped` would catch that -- but a category
/// whose patterns compile and match nothing at all is a different failure, and
/// this is where it would show. Asserts the count rather than each name, since
/// the names are the game's and will grow.
#[test]
fn the_table_declares_the_categories_the_data_holds() {
    let tsv = include_str!("../data/gameobj-data.tsv");
    let mut declared: Vec<(&str, &str)> = tsv
        .lines()
        .skip(1)
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let mut cols = line.splitn(4, '\t');
            match (cols.next(), cols.next(), cols.next()) {
                // `suffix` rows are not loaded, so a category declared ONLY by
                // a suffix row would legitimately be absent.
                (Some(kind), Some(name), Some(field)) if field != "suffix" => Some((kind, name)),
                _ => None,
            }
        })
        .collect();
    declared.sort_unstable();
    declared.dedup();

    let loaded =
        categories(Classification::Type).len() + categories(Classification::Sellable).len();
    assert_eq!(
        loaded,
        declared.len(),
        "every declared category must load: {declared:?}"
    );
}
