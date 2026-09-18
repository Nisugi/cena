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
    CritTables::load()
        .or_else(|_| CritTables::from_entries(Vec::new()))
        .unwrap_or_default()
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
    for entry in tables.entries() {
        let Some(line) = synthesise_matching_line(&entry.pattern) else {
            continue; // pattern too complex to literalise; other tests cover it
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
