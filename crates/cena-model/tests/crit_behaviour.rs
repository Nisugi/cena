//! Behaviour of the ported crit tables: matching, lookup, and loading.
//!
//! Split from `crit_parity.rs` under Rule 4.1 (`plan/05:352-353`) -- move code
//! down, do not raise the cap. The seam is what each file asserts about:
//! `crit_parity.rs` holds the tables to the Ruby they were cut from (counts,
//! digest, the anomalous entries); this file holds the *API* to what the data
//! says (which lines match, what `get` returns, what the loader rejects).
//!
//! Read `crit_parity.rs`'s module header first: its account of the two-stage
//! extraction, and of why no Ruby runs at test time, governs both files.

use cena_model::crit::types::{DamageType, Location, Position, WoundLocation};
use cena_model::crit::{CritEntry, CritTables, STUN_UNKNOWN, load};

/// The shipped tables, or an empty table if they would not load.
///
/// No `unwrap`/`expect`/`panic!` here: clippy.toml's `allow-*-in-tests` covers
/// `#[test]` functions, not helpers beside them, and the workspace denies all
/// three. A failure therefore degrades to an empty table, and every caller
/// asserts against a count, a digest or a key, so it still fails loudly.
///
/// # The error is PRINTED, not swallowed
///
/// This used to end `.unwrap_or_default()`, discarding a `LoadError` that
/// names the offending line and its reason. The claim beside it was that a
/// later assertion gives a better message; it does not. "0 vs 2394" says a
/// table did not load and nothing about why, while the error says which row
/// broke and how -- exactly what someone regenerating the TSV needs
/// (review MO-8).
///
/// `eprintln!` rather than a panic because of the lint scoping above, and
/// because a printed cause plus a failing assertion is strictly more than the
/// assertion alone.
fn tables() -> CritTables {
    match CritTables::load() {
        Ok(tables) => tables,
        Err(e) => {
            eprintln!(
                "crit tables did not load: {e}\n\
                 The assertions below will fail against an EMPTY table, which \
                 reports a count of 0 and says nothing about the cause. The \
                 line above is the cause."
            );
            CritTables::from_entries(Vec::new()).unwrap_or_default()
        }
    }
}

#[test]
fn the_rewritten_lookahead_still_excludes_removes_skull_lines() {
    // The specific entry, driven both ways. Proven equivalent to the Ruby
    // regex on 10 adversarial lines (0 disagreements) before it was written;
    // this keeps it that way.
    let tables = tables();
    let slash_head = tables
        .get(DamageType::Slash, Location::Head, 3)
        .expect("slash/head/3 must exist");
    assert!(
        slash_head.pattern.contains("(?!"),
        "slash/head/3 is the lookahead entry; its stored pattern should still \
         be Lich's source form, not the rewrite: {:?}",
        slash_head.pattern
    );

    let matched = |line: &str| -> Vec<String> {
        tables
            .parse(line)
            .into_iter()
            .map(|e| format!("{}/{}/{}", e.damage_type, e.location, e.rank))
            .collect()
    };

    // Matches, because it is a plain blow to the head.
    for line in [
        "Blow to head.",
        "Blow to headache.",
        "Blow to head is deflected.",
    ] {
        assert!(
            matched(line).contains(&"slash/head/3".to_owned()),
            "slash/head/3 should match {line:?}, got {:?}",
            matched(line)
        );
    }

    // Does NOT match: this is impact/head's message, which is the entire
    // reason the lookahead exists.
    for line in [
        "Blow to head removes skull.",
        "Blow to head. The blow removes skull.",
        "Blow to head removes skullcap.",
    ] {
        let hits = matched(line);
        assert!(
            !hits.contains(&"slash/head/3".to_owned()),
            "slash/head/3 must NOT match {line:?} -- that is impact/head's \
             message (impact_critical_table.rb:205) and the negative lookahead \
             exists to keep them apart. Got {hits:?}"
        );
    }

    // And the line it was protecting still finds its own entry.
    //
    // **Named exactly, not "something".** This was
    // `contains("impact/head/9") || !is_empty()`, where the second disjunct
    // subsumes the first: any match at all satisfied it, including the
    // `slash/head/3` the veto exists to prevent. The assertion could not fail
    // for the reason it was written (review MO-8).
    //
    // The TSV carries exactly two entries whose pattern mentions this line:
    //
    // ```text
    // $ grep 'removes skull' crates/cena-model/data/crit_tables.tsv
    // impact/head/9  ^Blow to head removes skull.
    // slash/head/3   ^(?!.*removes skull.)Blow to head.
    // ```
    //
    // so the right answer is knowable and singular.
    let hits = matched("Blow to head removes skull.");
    assert!(
        hits.contains(&"impact/head/9".to_owned()),
        "impact's own 'removes skull' entry must match its own message: {hits:?}"
    );
    assert!(
        !hits.contains(&"slash/head/3".to_owned()),
        "slash/head/3's negative lookahead exists precisely to keep it OFF          this line; a match here means the veto is not applied: {hits:?}"
    );
}

#[test]
fn ambiguous_lines_return_every_entry_they_match() {
    // `parse` returns a Vec because the game data is genuinely ambiguous.
    // These are measured pairs, and they are the reason the API is not
    // `Option<&CritEntry>`.
    let tables = tables();
    let keys = |line: &str| -> Vec<String> {
        let mut k: Vec<String> = tables
            .parse(line)
            .into_iter()
            .map(|e| format!("{}/{}/{}", e.damage_type, e.location, e.rank))
            .collect();
        k.sort();
        k
    };

    assert_eq!(
        keys("A kobold right leg jerks momentarily."),
        vec![
            "disruption/right_leg/1".to_owned(),
            "unbalance/right_leg/1".to_owned()
        ],
        "two damage types share this message and it is indistinguishable from \
         the line alone; collapsing to one entry would be inventing certainty"
    );
    assert_eq!(
        keys("Steam billows around a kobold."),
        vec!["steam/chest/0".to_owned(), "steam/head/0".to_owned()],
        "one damage type, two locations, same message"
    );
}

#[test]
fn every_key_is_unique_and_sorted() {
    // What makes `get`'s binary search correct.
    let tables = tables();
    let keys: Vec<_> = tables.entries().iter().map(CritEntry::key).collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(
        keys, sorted,
        "entries must be sorted by key for binary search"
    );
    let mut deduped = sorted.clone();
    deduped.dedup();
    assert_eq!(
        deduped.len(),
        keys.len(),
        "(damage_type, location, rank) must be unique, or `get` is ambiguous"
    );
}

#[test]
fn get_finds_every_entry_it_should() {
    // Drives the binary search over every key rather than a sample, including
    // the sparse ranks that are the reason it is not a dense index.
    let tables = tables();
    for entry in tables.entries() {
        let found = tables.get(entry.damage_type, entry.location, entry.rank);
        assert_eq!(
            found,
            Some(entry),
            "get({}, {}, {}) did not return its own entry",
            entry.damage_type,
            entry.location,
            entry.rank
        );
    }
    assert!(
        tables
            .get(DamageType::Acid, Location::RightEye, 0)
            .is_none(),
        "ranks are sparse -- acid/right_eye holds [4, 7, 8] -- so a missing \
         rank must be None rather than a neighbouring entry"
    );
}

#[test]
fn the_loader_rejects_a_malformed_row_rather_than_guessing() {
    // The data file is generated, so these are reachable only via a bad
    // regeneration -- which is exactly when a message naming the line matters.
    let good = "type\tlocation\trank\tdamage\tposition\tfatal\tstunned\tamputated\t\
                crippled\tsleeping\tdazed\tlimb_favored\troundtime\tsilenced\tslowed\t\
                wound_rank\tsecondary_location\tsecondary_wound_rank\tpattern\n\
                acid\thead\t0\t0\t\t0\t0\t0\t0\t0\t0\t0\t0\t0\t0\t0\t\t\t^Ouch.\n";
    assert_eq!(
        load::parse_tsv(good).map(|e| e.len()),
        Ok(1),
        "the reference row must parse"
    );

    let short = good.replace("\t\t\t^Ouch.", "\t^Ouch.");
    assert!(
        load::parse_tsv(&short).is_err(),
        "a row with the wrong field count must fail, not silently shift fields"
    );

    let bad_bool = good.replace("acid\thead\t0\t0\t\t0", "acid\thead\t0\t0\t\ttrue");
    assert!(
        load::parse_tsv(&bad_bool).is_err(),
        "a boolean that is not 0 or 1 must fail: a permissive reader would let \
         a shifted column read as a plausible value"
    );

    let bad_type = good.replace("acid\thead", "acidic\thead");
    assert!(
        load::parse_tsv(&bad_type).is_err(),
        "an unknown damage type must fail rather than be dropped"
    );

    let reordered = good.replace("type\tlocation", "location\ttype");
    assert!(
        load::parse_tsv(&reordered).is_err(),
        "a reordered header must fail: otherwise a regeneration that changed \
         column order would shift every field by one and still parse"
    );
}

#[test]
fn the_measured_domains_still_hold() {
    // Every integer width in types.rs rests on one of these. If a Lich update
    // widens a domain, the field silently truncates -- so the ranges are
    // asserted rather than trusted to a comment.
    let tables = tables();
    let entries = tables.entries();

    let max = |f: fn(&cena_model::crit::CritEntry) -> u32| entries.iter().map(f).max().unwrap_or(0);
    assert_eq!(max(|e| u32::from(e.rank)), 11, "rank was measured 0..=11");
    assert_eq!(
        max(|e| u32::from(e.damage)),
        88,
        "damage was measured 0..=88"
    );
    assert_eq!(
        max(|e| u32::from(e.roundtime)),
        20,
        "roundtime was measured in {{0, 2, 5, 10, 20}}"
    );
    assert_eq!(
        max(|e| u32::from(e.wound_rank)),
        3,
        "wound_rank measured 0..=3"
    );

    assert_eq!(
        entries.iter().filter(|e| e.stunned == STUN_UNKNOWN).count(),
        1,
        "exactly one entry uses the 999 'unknown' sentinel"
    );
    assert_eq!(
        entries
            .iter()
            .filter(|e| e.stunned != STUN_UNKNOWN)
            .map(|e| e.stunned)
            .max(),
        Some(20),
        "real stun durations were measured 0..=20"
    );

    // crippled is false in every entry. Asserted because types.rs says so.
    assert!(
        entries.iter().all(|e| !e.crippled),
        "`crippled` was measured false in all 2,394 entries; if that has \
         changed, the field now carries information and types.rs's note saying \
         it does not is stale"
    );

    let positions = |p: Position| entries.iter().filter(|e| e.position == Some(p)).count();
    assert_eq!(positions(Position::Prone), 319);
    assert_eq!(positions(Position::Kneeling), 13);
    assert_eq!(positions(Position::Sitting), 11);
    assert_eq!(
        entries.iter().filter(|e| e.position.is_none()).count(),
        2051,
        "2,051 entries force no position"
    );

    assert_eq!(
        entries
            .iter()
            .filter(|e| e.secondary_wound.is_some())
            .count(),
        156,
        "156 entries carry a secondary wound"
    );
}

#[test]
fn the_both_eyes_secondary_wound_survived() {
    // The data fact that forced WoundLocation to be its own enum: `both eyes`
    // is a secondary wound location and not a primary location key.
    let tables = tables();
    let both_eyes: Vec<_> = tables
        .entries()
        .iter()
        .filter(|e| {
            e.secondary_wound
                .is_some_and(|w| w.location == WoundLocation::BothEyes)
        })
        .collect();
    assert_eq!(
        both_eyes.len(),
        1,
        "exactly one entry carries a `both eyes` secondary wound. It is why \
         WoundLocation is not Location -- reusing Location would mean either \
         inventing a BothEyes primary location no table has, or dropping this \
         entry silently."
    );
    assert!(
        WoundLocation::BothEyes.as_location().is_none(),
        "BothEyes has no primary Location, which is the whole point"
    );
}

#[test]
fn the_most_specific_entry_comes_first() {
    // Callers take `parse(...).first()`, following Lich's `.values.first`, so
    // the order IS the answer. Four pairs in the tables are literal-prefix
    // subsumptions where key order returned the weaker entry; assert the
    // stronger one now leads.
    let tables = tables();
    let line = "Burst of flames to chest toasts skin nicely.";
    let hits = tables.parse(line);
    assert!(
        hits.len() >= 2,
        "{line:?} must still match both fire/chest entries, got {:?}",
        hits.iter().map(|hit| hit.key()).collect::<Vec<_>>()
    );
    let first = hits[0];
    assert_eq!(
        (first.damage_type, first.location, first.rank),
        (DamageType::Fire, Location::Chest, 2),
        "the longer, more specific pattern must win: key order would put \
         fire/chest/0 (damage 0) ahead of fire/chest/2 (damage 10, wound 1)"
    );
    assert!(
        hits.windows(2)
            .all(|pair| pair[0].pattern.len() >= pair[1].pattern.len()),
        "results must be ordered by descending pattern length"
    );
}
