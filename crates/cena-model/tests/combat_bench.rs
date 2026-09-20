//! How much the combat classifiers cost per line, measured here.
//!
//! `plan/06` §1.9: *"Never port a performance conclusion; port the discipline
//! of measuring, then measure here."* Lich's `PatternGate` exists because
//! Ruby scans a big alternation linearly and 35µs/line was the number worth
//! saving. This is the number for the in-order `Vec<Regex>` design, over the
//! 71 fixtures' real feed lines, with every family tried on every line -- the
//! worst case, since a consumer would stop at the first family that answered.
//!
//! It prints and does not assert a figure: a timing assertion is a flake on a
//! loaded CI box. The one bound it keeps is absurdly loose, so a regression
//! of a hundredfold still fails. Run with `--nocapture` to read the numbers:
//!
//! ```text
//! cargo test -p cena-model --test combat_bench -- --nocapture
//! ```

use std::path::PathBuf;
use std::time::Instant;

use cena_model::state::chunks::ChunkLine;
use cena_model::state::combat::bracket::{
    assault_end, assault_start, sequence_end, sequence_start,
};
use cena_model::state::combat::defs::defs;
use cena_model::{
    AmbushPrefix, AttackLine, DamageLine, FlareLine, GameState, Outcome, Resolution, SpellLoss,
    StatusLine, UcsLine,
};
use cena_protocol::Parser;

/// Every fixture line through the parser, one file per chunk.
///
/// Per file rather than all at once: the open chunk is capped at
/// `MAX_CHUNK_LINES` (200) and drops the oldest, and the fixtures together are
/// several times that. No single fixture file is.
fn fixture_lines() -> Vec<ChunkLine> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/combat");
    let mut lines = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        let mut paths: Vec<PathBuf> = rd.filter_map(Result::ok).map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            let mut wire = String::new();
            for raw in std::fs::read_to_string(&path).unwrap_or_default().lines() {
                if !raw.starts_with('#') && !raw.trim().is_empty() {
                    wire.push_str(raw);
                    wire.push('\n');
                }
            }
            let mut parser = Parser::new();
            let mut state = GameState::default();
            for frame in parser.push_bytes(wire.as_bytes()) {
                state.apply(&frame);
            }
            assert!(
                !state.open_chunk().is_truncated(),
                "{} exceeds one chunk; split it",
                path.display()
            );
            lines.extend_from_slice(state.open_chunk().lines());
        }
    }
    lines
}

/// Every classifier, on one line. What a consumer that asked everything of
/// every line would pay.
fn classify_all(line: &ChunkLine) -> usize {
    usize::from(AttackLine::classify(line).is_some())
        + usize::from(AmbushPrefix::classify(line).is_some())
        + usize::from(DamageLine::classify(line).is_some())
        + usize::from(Resolution::classify(line).is_some())
        + usize::from(Outcome::classify(line).is_some())
        + usize::from(FlareLine::classify(line).is_some())
        + usize::from(StatusLine::classify(line).is_some())
        + usize::from(SpellLoss::classify(line).is_some())
        + usize::from(UcsLine::classify(line).is_some())
        + usize::from(assault_start(line).is_some())
        + usize::from(assault_end(line).is_some())
        + usize::from(sequence_start(line).is_some())
        + usize::from(sequence_end(line).is_some())
}

#[test]
fn the_classifiers_cost_this_much_per_line() {
    // First touch: compiling 946 patterns.
    let t0 = Instant::now();
    let table = defs();
    let first_touch = t0.elapsed();
    let patterns: usize = table.families().map(|f| table.family(f).len()).sum();

    let lines = fixture_lines();
    assert!(
        lines.len() > 500,
        "guard: only {} lines loaded; the fixtures did not read",
        lines.len()
    );
    let rounds = 20;
    let t1 = Instant::now();
    let mut hits = 0usize;
    for _ in 0..rounds {
        for line in &lines {
            hits += classify_all(line);
        }
    }
    let total = t1.elapsed();
    let per_line = total / u32::try_from(lines.len() * rounds).unwrap_or(1);

    eprintln!("combat classifiers: {patterns} patterns compiled in {first_touch:?}");
    eprintln!(
        "combat classifiers: {} lines x {rounds} rounds = {total:?}, {per_line:?}/line, every family on every line, {} classifications",
        lines.len(),
        hits / rounds
    );
    assert!(
        per_line.as_micros() < 5_000,
        "{per_line:?}/line is a hundredfold regression on the measured figure"
    );
}
