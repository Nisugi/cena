//! The match index: its shape, and that every entry is reachable through it.
//!
//! Split from `crit_behaviour.rs` under Rule 4.1 (`plan/05:352-353`) -- move
//! code down, do not raise the cap. The seam: `crit_behaviour.rs` asserts what
//! the *data* says (which lines match, what `get` returns); this file asserts
//! that the *index over that data* is a faithful optimisation -- correct shape,
//! no unreachable entry, and no whitespace-dependent match.
//!
//! Both properties here were found by adversarial review after all 15 original
//! tests passed against a broken index. Read `crit_parity.rs`'s module header
//! first for the extraction that produced the data.

use cena_model::crit::CritTables;

/// The shipped tables, or an empty table if they would not load.
///
/// See `crit_behaviour.rs`'s copy of this helper for why it does not panic.
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
            CritTables::empty()
        }
    }
}

#[test]
fn the_match_index_has_its_measured_shape() {
    // The index is an optimisation, and an optimisation whose shape nobody
    // checks is one that can quietly become a full scan.
    let tables = tables();
    let (buckets, residual, largest) = tables.index_shape();
    assert_eq!(buckets, 454, "measured: 454 first-word buckets");
    assert_eq!(
        residual, 195,
        "measured: 195 patterns with no fixed first word"
    );
    assert_eq!(
        largest, 72,
        "measured: the largest bucket holds 72 patterns"
    );
    assert!(
        residual < tables.entries().len() / 4,
        "if most patterns fall into the residual list the index has stopped \
         indexing and every line pays a near-full scan"
    );
}

#[test]
fn every_entry_is_reachable_through_the_index() {
    // The shape test above checks the index's size, not whether its buckets
    // can ever be hit. `^Scaldl?ing ...` (steam/back/6) hashed to the key
    // "scaldl", which neither spelling of that message starts with, so the
    // entry was dead -- with all 15 original tests passing. That `l?` is a
    // deliberate hedge around a game-side typo (see `match_index.rs`), which
    // is exactly why losing it silently mattered: it is the entry designed to
    // survive a change in the wire text.
    //
    // Synthesise a line each pattern accepts and require the index to find it.
    // This is the end-to-end property; where a pattern lives (bucket or
    // residual) is the index's business, not the test's.
    let tables = tables();
    // **Counted, not merely skipped.**
    //
    // The skip used to be a bare `continue` with the comment "other tests
    // cover it", which was not true: nothing else matched these patterns
    // against a line. 46 of 2,394 entries were silently outside the only test
    // that checks reachability, and one more becoming unliteralisable would
    // have widened that gap with no signal (review MO-5).
    //
    // MEASURED, and asserted below so it cannot drift:
    //
    // ```text
    // $ awk -F'\t' 'NR>1 {p=$NF; if (p ~ /[()|{}$]/ || p !~ /^\^/ || \
    //       p ~ /^\^\(\?!/) c++} END {print c}' crates/cena-model/data/crit_tables.tsv
    // 46
    // ```
    let mut skipped = 0usize;
    for entry in tables.entries() {
        let Some(line) = synthesise_matching_line(&entry.pattern) else {
            // Not literalisable: the pattern's language is not a single
            // literal string. Compiling it is still checked by the loader,
            // and every field is still covered by `crit_parity.rs`'s digest.
            skipped += 1;
            continue;
        };
        let hits = tables.parse(&line);
        assert!(
            hits.iter().any(|hit| hit.key() == entry.key()),
            "{:?}/{:?}/{} is unreachable through the index: pattern {:?}              accepts {line:?}, but parse returned {:?}",
            entry.damage_type,
            entry.location,
            entry.rank,
            entry.pattern,
            hits.iter().map(|hit| hit.key()).collect::<Vec<_>>()
        );
    }

    assert_eq!(
        skipped, EXPECTED_SKIPS,
        "the number of patterns this test cannot synthesise a line for has          changed. That is not necessarily wrong -- a new pattern with a          group or an alternation is legitimate -- but it must be a VISIBLE          change, because each one is an entry outside the only test that          checks reachability. Update EXPECTED_SKIPS with the measurement in          its doc comment."
    );
}

/// Patterns `synthesise_matching_line` cannot literalise.
///
/// A pattern with an UNESCAPED `(`, `)`, `|`, `{`, `}` or `$`, or one that is
/// not `^`-anchored, has no single literal line that exercises it.
///
/// # The count is 45, and a plausible one-liner says 46
///
/// The review reported 46, from
///
/// ```text
/// awk over the TSV's last column, counting any pattern that holds one
/// of `()|{}$` or lacks a `^` anchor
/// ```
///
/// which counts `^Burn exposes the spine \(from the front\).` -- where the
/// parentheses are **escaped**, so they are literal text and the synthesiser
/// handles them through its `\` arm. A character-class scan cannot see the
/// escape; the synthesiser can.
///
/// This constant is therefore pinned to what the FUNCTION does, measured by
/// running it, rather than to a regex over the data that approximates it. The
/// approximation is what was wrong, which is the point: when a test and a
/// one-liner disagree about the code's behaviour, the code is the oracle.
const EXPECTED_SKIPS: usize = 45;

/// Does `pattern` accept `line`, ignoring the index entirely?
///
/// Reproduces `match_index.rs`'s one documented exclusion -- Lich's single
/// negative lookahead, which `regex` does not support and which the index
/// folds into a match-then-veto pair. Two lines, for one entry out of 2,394,
/// and restating them here is what keeps this an INDEPENDENT oracle: an
/// oracle that called the index's own predicate would agree with it by
/// construction.
fn compile_oracle(pattern: &str) -> (regex::Regex, Option<regex::Regex>) {
    const LOOKAHEAD: &str = "(?!.*removes skull.)";
    const EXCLUDED: &str = ".*removes skull.";

    let (source, veto) = if pattern.contains(LOOKAHEAD) {
        (
            pattern.replace(LOOKAHEAD, ""),
            regex::Regex::new(EXCLUDED).ok(),
        )
    } else {
        (pattern.to_owned(), None)
    };
    // `Regex::new` cannot fail here -- `CritTables::load` compiled every one
    // of these already, and this test only runs on tables that loaded. An
    // empty alternation is the inert stand-in for the impossible case:
    // matching nothing makes a hypothetical failure show up as a DISAGREEMENT
    // with the index rather than as a panic in a helper, which is the more
    // useful failure. (A `panic!` here is also unavailable: `clippy.toml`
    // scopes that lint away from `#[test]` functions, and this is a helper.)
    let regex = regex::Regex::new(&source).unwrap_or_else(|_| {
        regex::Regex::new("$.^").unwrap_or_else(|_| unreachable!("matches nothing"))
    });
    (regex, veto)
}

/// Does this compiled pair accept `line`?
fn accepts(compiled: &(regex::Regex, Option<regex::Regex>), line: &str) -> bool {
    let line = line.trim();
    compiled.0.is_match(line) && compiled.1.as_ref().is_none_or(|veto| !veto.is_match(line))
}

/// **The index agrees with a full scan.**
///
/// `match_index.rs` claimed this was "VERIFIED by exhaustive comparison ...
/// 0 disagreements / 2394", and no such comparison existed in the repo: the
/// claim described a one-off exercise in the voice of a standing guarantee
/// (review MO-5). `crit_index.rs:9-11` also records that the index WAS once
/// broken -- steam/back/6 -- "while all 15 original tests passed", which is
/// the reason a standing comparison is worth its runtime.
///
/// The oracle is a linear scan over every entry, run against every line this
/// test can synthesise. Bucketing is an optimisation, so the only thing that
/// makes it safe is that it returns what the slow path would.
#[test]
fn the_index_returns_what_a_full_scan_returns() {
    let tables = tables();
    // Compiled ONCE, not once per line. The oracle is 2,394 patterns and there
    // are ~2,348 lines to try, so recompiling inside the loop is ~5.6 million
    // regex compilations -- minutes of test time for no extra coverage.
    let oracle: Vec<_> = tables
        .entries()
        .iter()
        .map(|e| compile_oracle(&e.pattern))
        .collect();
    let mut compared = 0usize;

    for entry in tables.entries() {
        let Some(line) = synthesise_matching_line(&entry.pattern) else {
            continue;
        };
        compared += 1;

        let mut indexed: Vec<_> = tables.parse(&line).iter().map(|e| e.key()).collect();
        // The full scan, built from the ENTRY's own pattern rather than from
        // the index's compiled copy -- otherwise the oracle and the thing
        // under test share the same compilation and the comparison is
        // circular.
        let mut scanned: Vec<_> = tables
            .entries()
            .iter()
            .zip(&oracle)
            .filter(|(_, compiled)| accepts(compiled, &line))
            .map(|(candidate, _)| candidate.key())
            .collect();
        indexed.sort_unstable();
        scanned.sort_unstable();

        assert_eq!(
            indexed, scanned,
            "the index and a full scan disagree on {line:?}. Bucketing is an              optimisation; the only thing that makes it safe is returning              what the slow path returns. A first-word bucket that drops a              match is exactly the steam/back/6 defect this file's header              records, which 15 passing tests did not catch."
        );
    }

    assert!(
        compared > 2_000,
        "only {compared} lines were compared; the synthesiser has stopped          producing lines and this test is passing by doing nothing"
    );
}

/// A line the pattern matches, built by literalising the constructs the crit
/// tables actually use. Returns `None` for anything else, so a future pattern
/// shape is skipped rather than silently asserted against a wrong line.
fn synthesise_matching_line(pattern: &str) -> Option<String> {
    let body = pattern.strip_prefix('^')?;
    // The one negative lookahead is an exclusion; its entry is covered by
    // `the_negative_lookahead_excludes_only_what_it_names`.
    if body.starts_with("(?!") {
        return None;
    }
    let mut out = String::new();
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            // `.*?` / `.+?` stand in for a creature name.
            '.' => {
                if matches!(chars.peek(), Some('*' | '+')) {
                    chars.next();
                    if chars.peek() == Some(&'?') {
                        chars.next();
                    }
                    out.push_str("a kobold");
                } else {
                    out.push('.'); // a literal-ish `.`: any char, so `.` works
                }
            }
            // `l?` -- take the optional letter; `[Kk]` -- take the first.
            '[' => {
                let first = chars.next()?;
                for skip in chars.by_ref() {
                    if skip == ']' {
                        break;
                    }
                }
                out.push(first);
            }
            '\\' => out.push(chars.next()?),
            '?' | '*' | '+' => {} // the preceding char is already emitted
            '(' | ')' | '|' | '{' | '}' | '$' => return None,
            other => out.push(other),
        }
    }
    Some(out)
}

#[test]
fn trailing_whitespace_does_not_create_a_match() {
    // Lich matches `line.strip` -- both ends. 2,379 of the 2,394 patterns end
    // in an unescaped `.`, which is regex "any character", so a trailing space
    // satisfies it. Trimming only the front made `"Blow to head "` match where
    // Ruby returns nil, on 2,271 of those patterns.
    let tables = tables();
    for line in [
        "Blow to head",
        "Blow to head.",
        "Blow to head removes skull.",
    ] {
        let bare = tables.parse(line);
        for padded in [
            format!("{line} "),
            format!("{line}\t"),
            format!(" {line}  "),
        ] {
            let got = tables.parse(&padded);
            assert_eq!(
                bare.iter().map(|e| e.key()).collect::<Vec<_>>(),
                got.iter().map(|e| e.key()).collect::<Vec<_>>(),
                "{padded:?} must match exactly what {line:?} matches"
            );
        }
    }
}
