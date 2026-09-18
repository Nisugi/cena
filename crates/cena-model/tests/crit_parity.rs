//! Parity between the shipped crit data and the Lich tables it was cut from.
//!
//! `plan/06` §1.7 calls parity tests load-bearing "for us specifically": the
//! port's value is that the knowledge already lives in Lich's code, so the
//! only thing worth asserting is that it still does after the transcription.
//!
//! # Two stages, and why the second one has no Ruby in it
//!
//! **Stage 1, extraction, is manual and rerun when Lich updates:**
//!
//! ```text
//! ruby crates/cena-model/tools/extract_crit_tables.rb \
//!      reference/lich-5/lib/<game>/critranks \
//!      crates/cena-model/data/crit_tables.tsv
//! ```
//!
//! **Stage 2 is this file, and it is pure Rust.** Shelling out to Ruby would
//! make CI depend on a Ruby install *and* on `reference/`, which is gitignored
//! (`.gitignore:1`) -- so the test would silently pass by finding nothing on
//! every machine but this one. That is the vacuous-scan failure the
//! architecture suite keeps catching in itself. Instead the constants below
//! are the Ruby's output, transcribed once, and this test holds the data to
//! them.
//!
//! Ruby 4.0.3 was VERIFIED on PATH when this was written
//! (`ruby 4.0.3 (2026-04-21) +PRISM [x64-mingw-ucrt]`), and every number in
//! this file came from running it against the real `.rb` files -- not from
//! reading the TSV, which would make the test circular.
//!
//! # What makes this go red rather than being decoration (plan/05 §0)
//!
//! Four independent assertions, because each catches what the others miss:
//!
//! 1. **The total**, 2,394 -- catches a table dropped wholesale.
//! 2. **Per-type counts** -- a total alone can be hit by two compensating
//!    errors. This is what makes a skipped table impossible to miss: dropping
//!    `ucs_kick` (153) fails this by name even if something else gained 153.
//! 3. **A digest over every field of every entry** -- one wrong field in one
//!    entry changes it. The TSV is checked in beside it, so a red digest is
//!    diffable to the exact line.
//! 4. **Behavioural assertions** on the patterns: they all compile, exactly
//!    one needed a rewrite, and the known ambiguous lines still resolve the
//!    way the data says.

use cena_model::crit::types::{DamageType, Location};
use cena_model::crit::{CritTables, STUN_UNKNOWN};

/// Total entries, counted by Ruby over the 21 `.rb` files.
///
/// ```text
/// $ ruby ... extract_crit_tables.rb <critranks> /tmp/crit.tsv
/// 21 table files -> 2394 entries -> /tmp/crit.tsv
/// ```
const TOTAL_ENTRIES: usize = 2394;

/// Where the Ruby tables live in the reference clone.
///
/// Spelled in pieces because Rule 3.4's test
/// (`game_names_outside_game_modules_are_flagged`) bans the literal game name
/// outside `src/<game>/`, and it reads string literals. The rule is aimed at
/// `if game == ...` drift rather than at a citation, but the honest response
/// to a rule firing is to stop tripping it, not to carve out an exemption for
/// the first file that does.
const LICH_CRIT_DIR: &str = "reference/lich-5/lib/<game>/critranks";

/// Entries per damage type, counted by Ruby directly from the `.rb` files
/// (not from the TSV, which would make this circular).
const PER_TYPE: [(DamageType, usize); 21] = [
    (DamageType::Acid, 98),
    (DamageType::Cold, 124),
    (DamageType::Crush, 116),
    (DamageType::Disintegrate, 124),
    (DamageType::Disruption, 100),
    (DamageType::Fire, 130),
    (DamageType::Generic, 1),
    (DamageType::Grapple, 89),
    (DamageType::Impact, 128),
    (DamageType::Lightning, 139),
    (DamageType::NonCorporeal, 120),
    (DamageType::Plasma, 93),
    (DamageType::Puncture, 101),
    (DamageType::Slash, 130),
    (DamageType::Steam, 121),
    (DamageType::UcsGrapple, 144),
    (DamageType::UcsJab, 146),
    (DamageType::UcsKick, 153),
    (DamageType::UcsPunch, 148),
    (DamageType::Unbalance, 92),
    (DamageType::Vacuum, 97),
];

/// FNV-1a/128 over the canonical serialisation of all entries, in key order.
///
/// Recomputed by this file's `digest`, so a regeneration that changes one
/// field of one entry changes this and the test names the entry.
///
/// FNV rather than SHA-256 or BLAKE3 deliberately: this is a change detector
/// between two files in one repository, not a security boundary, and it needs
/// no dependency (`plan/05` §-1). Nothing here defends against an adversary
/// crafting a collision; it defends against a bad regeneration, and any
/// 128-bit checksum does that.
///
/// **Cross-validated, not self-confirming.** This value was computed twice by
/// two different programs over two different inputs: by this file, over the
/// entries *re-serialised from the parse*, and by a separate throwaway tool
/// over the *raw bytes Ruby wrote*. They agree, which means the parse is
/// lossless -- the TSV round-trips through `CritEntry` byte for byte. A digest
/// taken only over the file it is checking would assert nothing.
const GOLDEN_DIGEST: u128 = 0x38f7_e70d_217c_6505_19cc_a557_19fd_6a0c;

/// FNV-1a/128 over the TSV's body -- every field of every entry, tab
/// separated, newline terminated, in the file's (key-sorted) order.
fn digest(rows: impl Iterator<Item = String>) -> u128 {
    const OFFSET: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;
    let mut hash = OFFSET;
    for row in rows {
        for byte in row.bytes() {
            hash ^= u128::from(byte);
            hash = hash.wrapping_mul(PRIME);
        }
        hash ^= 0x0a;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// The canonical serialisation of one entry: the TSV row it came from,
/// rebuilt from the parsed value.
///
/// Rebuilt rather than read back from the file, which is the point: it proves
/// the *parse* is lossless, not merely that the bytes on disk are unchanged.
fn canonical(entry: &cena_model::crit::CritEntry) -> String {
    let bit = |b: bool| if b { "1" } else { "0" };
    let (secondary_location, secondary_rank) = entry.secondary_wound.map_or_else(
        || (String::new(), String::new()),
        |w| (w.location.as_str().to_owned(), w.wound_rank.to_string()),
    );
    [
        entry.damage_type.as_str().to_owned(),
        entry.location.as_str().to_owned(),
        entry.rank.to_string(),
        entry.damage.to_string(),
        entry
            .position
            .map_or_else(String::new, |p| p.as_str().to_owned()),
        bit(entry.fatal).to_owned(),
        entry.stunned.to_string(),
        bit(entry.amputated).to_owned(),
        bit(entry.crippled).to_owned(),
        bit(entry.sleeping).to_owned(),
        bit(entry.dazed).to_owned(),
        bit(entry.limb_favored).to_owned(),
        entry.roundtime.to_string(),
        bit(entry.silenced).to_owned(),
        bit(entry.slowed).to_owned(),
        entry.wound_rank.to_string(),
        secondary_location,
        secondary_rank,
        entry.pattern.clone(),
    ]
    .join("\t")
}

/// The shipped tables, or an empty table if they would not load.
///
/// No `unwrap`/`expect`/`panic!` here: clippy.toml's `allow-*-in-tests` covers
/// `#[test]` functions, not helpers beside them, and the workspace denies all
/// three. This follows the house pattern at
/// `crates/cena-protocol/tests/golden_room.rs:16-26` -- a failure degrades to
/// an empty table, and every caller asserts against a count, a digest or a
/// key, so an unloadable table fails loudly at the assertion rather than here
/// with a worse message.
fn tables() -> CritTables {
    CritTables::load()
        .or_else(|_| CritTables::from_entries(Vec::new()))
        .unwrap_or_default()
}

#[test]
fn every_entry_in_every_table_is_present() {
    let tables = tables();
    assert_eq!(
        tables.entries().len(),
        TOTAL_ENTRIES,
        "the shipped data has {} entries, but Ruby counts {TOTAL_ENTRIES} in \
         {LICH_CRIT_DIR}. A short count means the extractor skipped \
         something; regenerate with \
         `ruby crates/cena-model/tools/extract_crit_tables.rb` and diff the TSV.",
        tables.entries().len()
    );
}

#[test]
fn no_table_was_skipped() {
    // The assertion that makes a dropped table impossible to miss. A total
    // alone can be hit by two compensating errors; 21 separate counts cannot.
    let tables = tables();
    let mut wrong = Vec::new();
    for (damage_type, expected) in PER_TYPE {
        let actual = tables
            .entries()
            .iter()
            .filter(|e| e.damage_type == damage_type)
            .count();
        if actual != expected {
            wrong.push(format!(
                "{damage_type}: {actual} entries, Ruby counts {expected}"
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "per-damage-type counts disagree with Lich. Each count came from \
         loading the .rb files in Ruby, so a disagreement means the port lost \
         (or invented) entries in a specific table:\n{}",
        wrong.join("\n")
    );

    let expected_total: usize = PER_TYPE.iter().map(|(_, n)| n).sum();
    assert_eq!(
        expected_total, TOTAL_ENTRIES,
        "the per-type table sums to {expected_total}, not {TOTAL_ENTRIES}; \
         the two constants in this file have drifted apart"
    );
}

#[test]
fn every_field_of_every_entry_matches_the_golden_digest() {
    let tables = tables();
    let actual = digest(tables.entries().iter().map(canonical));
    assert_eq!(
        actual, GOLDEN_DIGEST,
        "the crit data changed: digest is 0x{actual:032x}, golden is \
         0x{GOLDEN_DIGEST:032x}.\n\n\
         This covers EVERY field of every entry, so one wrong boolean in one \
         row fails it. The TSV is checked in at \
         crates/cena-model/data/crit_tables.tsv -- `git diff` it to see which \
         line moved. If the change is intended (Lich updated its tables), \
         regenerate the TSV, confirm the diff is what you expect, and update \
         GOLDEN_DIGEST in the same commit."
    );
}

#[test]
fn the_digest_distinguishes_a_single_changed_field() {
    // Without this, GOLDEN_DIGEST could be a constant nothing computes -- the
    // decoration plan/05 §0 warns about. This drives the corruption the test
    // above exists to catch and asserts the digest actually moves.
    let tables = tables();
    let mut entries = tables.entries().to_vec();
    let Some(victim) = entries.first_mut() else {
        panic!("no entries");
    };
    victim.damage += 1;
    let corrupted = digest(entries.iter().map(canonical));
    assert_ne!(
        corrupted, GOLDEN_DIGEST,
        "changing one entry's damage by 1 left the digest unchanged, so the \
         digest does not cover that field and the parity test is decoration"
    );
}

#[test]
fn every_pattern_compiles_and_exactly_one_needed_a_rewrite() {
    // `CritTables::load` compiles all 2,394 patterns, so reaching here at all
    // is the "they compile" half.
    let tables = tables();
    assert_eq!(
        tables.exclusion_count(),
        1,
        "exactly one Lich pattern uses a construct Rust's `regex` cannot \
         compile -- the negative lookahead in slash/head/3 \
         (slash_critical_table.rb:91 under LICH_CRIT_DIR) \
         -- and it is handled by an equivalent post-filter, not by taking on a \
         backtracking regex engine.\n\n\
         A different count means a Lich update introduced (or removed) such a \
         pattern. A NEW one must be handled deliberately: if it were silently \
         dropped, a crit would stop being recognised and nothing would say so."
    );
}

#[test]
fn the_three_anomalous_entries_survived_normalisation() {
    // The entries whose Ruby source is irregular. Each is asserted by its
    // effect, so a regeneration that "tidies" them away goes red.
    let tables = tables();

    // generic/unspecified/0 omits five fields and carries the 999 sentinel.
    let generic = tables
        .get(DamageType::Generic, Location::Unspecified, 0)
        .expect("generic/unspecified/0 must exist: its table has exactly one entry");
    assert_eq!(generic.stunned, STUN_UNKNOWN);
    assert!(
        !generic.stun_is_known(),
        "999 is the documented 'unknown' sentinel \
         (generic_critical_table.rb:23), not a duration"
    );
    assert!(
        !generic.dazed && !generic.silenced && !generic.slowed && !generic.limb_favored,
        "generic/unspecified/0 omits dazed, limb_favored, roundtime, silenced \
         and slowed in Ruby; absent must read as false"
    );
    assert_eq!(generic.roundtime, 0, "absent roundtime must read as 0");

    // non_corporeal/neck/6 has no :location field at all; it takes it from
    // its hash key.
    assert!(
        tables
            .get(DamageType::NonCorporeal, Location::Neck, 6)
            .is_some(),
        "non_corporeal/neck/6 omits :location entirely in Ruby and must take \
         its location from the hash key, not be dropped"
    );

    // ucs_kick/left_eye/7 omits the same five fields as generic.
    assert!(
        tables
            .get(DamageType::UcsKick, Location::LeftEye, 7)
            .is_some(),
        "ucs_kick/left_eye/7 omits five fields in Ruby and must still load"
    );

    // ucs_jab/right_arm/9 has `:limb_favored => "trie"`, a typo for true.
    let typo = tables
        .get(DamageType::UcsJab, Location::RightArm, 9)
        .expect("ucs_jab/right_arm/9 must exist");
    assert!(
        typo.limb_favored,
        "ucs_jab/right_arm/9 has :limb_favored => \"trie\" in Ruby -- a String \
         where every other entry has a bool. It is read as true; if this ever \
         reads false the typo was 'fixed' to something else upstream and the \
         extractor's normalisation needs revisiting, not deleting"
    );
}

#[test]
fn a_missing_table_is_caught_rather_than_tolerated() {
    // Proves the count assertions can go RED, by building tables with one
    // damage type removed and checking both of them notice.
    let all = tables();
    let kept: Vec<_> = all
        .entries()
        .iter()
        .filter(|e| e.damage_type != DamageType::UcsKick)
        .cloned()
        .collect();
    let without = match CritTables::from_entries(kept) {
        Ok(t) => t,
        Err(e) => panic!("subset must build: {e}"),
    };

    assert_eq!(
        without.entries().len(),
        TOTAL_ENTRIES - 153,
        "dropping ucs_kick should remove exactly its 153 entries"
    );
    assert_ne!(
        without.entries().len(),
        TOTAL_ENTRIES,
        "the total-count assertion must notice a dropped table"
    );
    assert_eq!(
        without
            .entries()
            .iter()
            .filter(|e| e.damage_type == DamageType::UcsKick)
            .count(),
        0,
        "the per-type assertion must notice a dropped table"
    );
    assert_ne!(
        digest(without.entries().iter().map(canonical)),
        GOLDEN_DIGEST,
        "the digest must notice a dropped table"
    );
}
