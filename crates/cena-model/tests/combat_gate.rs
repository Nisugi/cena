//! The matching gate returns what the linear scan returns.
//!
//! `defs.rs` and `crit/match_index.rs` each put a `RegexSet` in front of
//! their patterns. A gate is an optimisation, and the only thing that makes
//! an optimisation safe is agreeing with the slow path -- so the slow path is
//! rebuilt here, from the defs' own public `regex` and `veto` and from the
//! crit entries' own pattern text, and compared on every real line the
//! fixtures hold, in every family.
//!
//! The rate test at the bottom is `plan/06` §1.9's discipline: the number in
//! `defs.rs`'s header is printed by a command, not remembered.

use std::path::PathBuf;

use cena_model::GameState;
use cena_model::crit::CritTables;
use cena_model::state::combat::defs::{Def, Role, defs};
use cena_protocol::Parser;

/// Every combat fixture line as the classifiers see it: through the parser
/// and the chunk, de-tagged. One blob at a time, because an open chunk is
/// capped and a single feed of everything keeps only its tail.
fn chunk_texts() -> Vec<String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut out = Vec::new();
    for dir in ["combat", "combat_raw"] {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(root.join(dir))
            .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).collect())
            .unwrap_or_default();
        paths.sort();
        for path in paths {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            // a header or a blank line ends a blob
            for blob in text.split("\n\n") {
                let wire: String = blob
                    .lines()
                    .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
                    .flat_map(|l| [l, "\n"])
                    .collect();
                let mut parser = Parser::new();
                let mut state = GameState::default();
                for frame in parser.push_bytes(wire.as_bytes()) {
                    state.apply(&frame);
                }
                out.extend(
                    state
                        .open_chunk()
                        .lines()
                        .iter()
                        .map(cena_model::state::chunks::ChunkLine::text),
                );
            }
        }
    }
    out
}

/// Does this def match, by its own regex and veto and nothing else?
fn accepts(def: &Def, text: &str) -> bool {
    def.regex.as_ref().is_some_and(|re| re.is_match(text))
        && !def.veto.as_ref().is_some_and(|v| v.is_match(text))
}

/// The first def a linear scan finds, as `(order, name)`.
fn linear(family: &str, role: Option<Role>, text: &str) -> Option<(u32, String)> {
    defs()
        .family(family)
        .iter()
        .filter(|d| role.is_none_or(|r| d.role == r))
        .find(|d| accepts(d, text))
        .map(|d| (d.order, d.name.clone()))
}

#[test]
fn every_family_has_a_gate() {
    let ungated: Vec<&str> = defs().ungated().collect();
    assert!(
        ungated.is_empty(),
        "these families fell back to the linear scan: {ungated:?}"
    );
    let tables = CritTables::load().unwrap_or_else(|_| CritTables::empty());
    assert!(
        tables.residual_is_gated(),
        "the crit residual set did not build"
    );
}

#[test]
fn the_gate_finds_the_def_a_linear_scan_finds() {
    let texts = chunk_texts();
    assert!(
        texts.len() > 700,
        "only {} lines: the fixtures moved",
        texts.len()
    );
    let families: Vec<&str> = defs().families().collect();
    let mut matched = 0usize;
    for text in &texts {
        for family in &families {
            let want = linear(family, None, text);
            let got = defs()
                .first_match(family, text)
                .map(|(d, _)| (d.order, d.name.clone()));
            assert_eq!(got, want, "family {family} on {text:?}");
            assert_eq!(defs().any_match(family, text), want.is_some());
            matched += usize::from(want.is_some());
            for role in [Role::Add, Role::Remove, Role::Start, Role::End] {
                let want = linear(family, Some(role), text);
                let got = defs()
                    .first_match_with_role(family, role, text)
                    .map(|(d, _)| (d.order, d.name.clone()));
                assert_eq!(got, want, "family {family} role {role:?} on {text:?}");
            }
        }
    }
    // guard: a comparison of nothing with nothing proves nothing
    assert!(matched > 400, "only {matched} matches were compared");
}

#[test]
fn the_crit_index_finds_what_a_full_scan_finds_on_real_lines() {
    let Ok(tables) = CritTables::load() else {
        return;
    };
    // The oracle is compiled from each entry's own pattern text, not from the
    // index's copy. The one lookahead is handled as `match_index.rs` does.
    let lookahead = "(?!.*removes skull.)";
    let veto = regex::Regex::new(".*removes skull.").ok();
    let oracle: Vec<(Option<regex::Regex>, bool)> = tables
        .entries()
        .iter()
        .map(|e| {
            let vetoed = e.pattern.contains(lookahead);
            (
                regex::Regex::new(&e.pattern.replace(lookahead, "")).ok(),
                vetoed,
            )
        })
        .collect();
    let mut hits = 0usize;
    for text in chunk_texts() {
        let line = text.trim();
        let mut want: Vec<_> = tables
            .entries()
            .iter()
            .zip(&oracle)
            .filter(|(_, (re, vetoed))| {
                re.as_ref().is_some_and(|re| re.is_match(line))
                    && !(*vetoed && veto.as_ref().is_some_and(|v| v.is_match(line)))
            })
            .map(|(e, _)| e.key())
            .collect();
        let mut got: Vec<_> = tables.parse(&text).iter().map(|e| e.key()).collect();
        want.sort_unstable();
        got.sort_unstable();
        assert_eq!(got, want, "on {text:?}");
        hits += usize::from(!want.is_empty());
    }
    assert!(hits > 20, "only {hits} crit lines were compared");
}

/// MEASURED, printed under `--release -- --nocapture`. Caches are warmed
/// first: a `regex` builds its DFA lazily, and the first pass pays for it.
#[test]
#[allow(clippy::cast_precision_loss)] // a line count, printed as a rate
fn the_gated_rate_is_printed() {
    const REPS: u32 = 20;
    let texts = chunk_texts();
    let families: Vec<&str> = defs().families().collect();
    let pass = |scan: &dyn Fn(&str, &str) -> bool| {
        let started = std::time::Instant::now();
        let mut hits = 0usize;
        for _ in 0..REPS {
            for text in &texts {
                for family in &families {
                    hits += usize::from(scan(family, text));
                }
            }
        }
        let per = started.elapsed().as_secs_f64() * 1e6 / (texts.len() as f64 * f64::from(REPS));
        (per, hits)
    };
    let gated_scan = |f: &str, t: &str| defs().first_match(f, t).is_some();
    let linear_scan = |f: &str, t: &str| linear(f, None, t).is_some();
    let _ = (pass(&gated_scan), pass(&linear_scan)); // warm
    let (gated, a) = pass(&gated_scan);
    let (scanned, b) = pass(&linear_scan);
    println!(
        "{} lines x {} families: linear {scanned:.1}us/line, gated {gated:.1}us/line",
        texts.len(),
        families.len()
    );
    assert_eq!(a, b);
}
