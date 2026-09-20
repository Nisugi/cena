//! The block machine, driven by the real `info` blob.
//!
//! M3 step 4. The machine is a **consumer** (`plan/12` §3a): it remembers which
//! multi-line report is open, so the stateless classifiers can be handed lines
//! that would mean nothing alone.

use cena_model::state::character::blocks::{Block, Blocks, InfoReport};
use cena_model::{Stat, StatKind, StatValue};
use cena_protocol::Parser;
use cena_protocol::frame::Frame;

/// Reassembled lines of the `info` fixture, each with its bold fragments.
///
/// **This is the reassembly `route_text` already performs** (`state/streams.rs:95`):
/// runs accumulate until `ends_line`. Doing it here proves the machine's input
/// contract, and carries the bold set because bold lives in the frames rather
/// than in the joined text.
///
/// No `unwrap`/`expect`/`panic!`: the workspace denies all three and
/// `clippy.toml`'s allowance covers `#[test]` functions only.
fn info_blob() -> Vec<(String, Vec<String>)> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../cena-protocol/tests/fixtures/character_info.xml");
    let bytes = std::fs::read(&path).unwrap_or_default();
    let mut parser = Parser::new();
    let mut frames = parser.push_bytes(&bytes);
    frames.extend(parser.push_bytes(b"\n"));

    let mut out = Vec::new();
    let mut line = String::new();
    let mut bold: Vec<String> = Vec::new();
    for frame in &frames {
        if let Frame::Text(text) = frame {
            line.push_str(&text.content);
            if text.style.bold_depth > 0 {
                bold.push(text.content.clone());
            }
            if text.ends_line {
                out.push((std::mem::take(&mut line), std::mem::take(&mut bold)));
            }
        }
    }
    out
}

/// Feed the whole blob through a fresh machine and end it, as a prompt would.
fn run_blob() -> (Blocks, Option<InfoReport>) {
    let mut blocks = Blocks::default();
    for (line, bold) in info_blob() {
        let refs: Vec<&str> = bold.iter().map(String::as_str).collect();
        blocks.offer_with_bold(&line, &refs);
    }
    let report = blocks.end();
    (blocks, report)
}

#[test]
fn the_blob_opens_a_block_and_the_prompt_closes_it() {
    let mut blocks = Blocks::default();
    assert_eq!(
        blocks.open(),
        Block::None,
        "nothing is open before any line arrives"
    );

    let lines = info_blob();
    assert!(
        lines.len() >= 12,
        "guard against vacuity: an unreadable fixture yields no lines and every \
         assertion here would pass over nothing; got {}",
        lines.len()
    );

    // The `Name:` line opens it.
    let header = lines
        .iter()
        .find(|(l, _)| l.starts_with("Name: "))
        .expect("the fixture carries an info header");
    assert!(blocks.offer(&header.0), "the header is recognised");
    assert_eq!(blocks.open(), Block::Info, "and it opens the info block");

    // And ending returns what accumulated, leaving nothing open.
    let _ = blocks.end();
    assert_eq!(
        blocks.open(),
        Block::None,
        "ending a block closes it, so the next line is ordinary text again"
    );
}

#[test]
fn all_ten_stats_reach_the_report() {
    let (_, report) = run_blob();
    let report = report.expect("the blob produces a report");
    let kinds: Vec<StatKind> = report.stats.iter().map(|(k, _, _)| *k).collect();
    assert_eq!(
        kinds,
        StatKind::ALL.to_vec(),
        "all ten stats, in the wire's print order"
    );
}

#[test]
fn the_identity_line_gives_race_and_profession() {
    let (_, report) = run_blob();
    let report = report.expect("the blob produces a report");
    assert_eq!(report.identity.race.as_deref(), Some("Half-Elf"));
    assert_eq!(
        report.identity.profession.as_deref(),
        Some("Ranger"),
        "the `(shown as: Hero)` title is stripped -- it is a title, not a \
         profession, and Lich strips it too"
    );
    assert_eq!(report.identity.gender.as_deref(), Some("Male"));
    assert_eq!(report.identity.age, Some(36));
}

#[test]
fn the_character_name_is_not_taken_from_the_info_line() {
    // `parser.rb:237`: "name captured here, but do not rely on it - use XML
    // instead". `<playerID>` and the `<a exist>` link are authoritative, and the
    // `info` line is a snapshot that Shroud of Deception can also lie about.
    //
    // `Identity` has no name field at all, which is the enforcement: there is
    // nowhere for a careless port to put it.
    let (_, report) = run_blob();
    let report = report.expect("the blob produces a report");
    let debug = format!("{:?}", report.identity);
    assert!(
        !debug.contains("Ashryn"),
        "the identity must not carry the name from `info`: {debug}"
    );
}

#[test]
fn only_the_bolded_stats_are_marked_enhanced() {
    // **The signal this step threads through.** Bold lives in the frames, so a
    // machine handed only reassembled text could never set this -- which is what
    // `enhanced_is_bolded` was before `classify_with_bold` existed: a documented
    // field that was always false.
    let (_, report) = run_blob();
    let report = report.expect("the blob produces a report");

    let bolded: Vec<StatKind> = report
        .stats
        .iter()
        .filter(|(_, _, b)| *b)
        .map(|(k, _, _)| *k)
        .collect();
    assert_eq!(
        bolded,
        vec![StatKind::Intuition, StatKind::Wisdom],
        "exactly Intuition and Wisdom are enhanced in this capture. If this is \
         empty the bold is being lost; if it is all ten, it is being attributed \
         to every line."
    );
}

#[test]
fn a_bolded_stat_folds_into_an_enhanced_flag() {
    let (_, report) = run_blob();
    let report = report.expect("the blob produces a report");
    let (_, line, bolded) = report
        .stats
        .iter()
        .find(|(k, _, _)| *k == StatKind::Intuition)
        .expect("Intuition is in the report");
    let folded = InfoReport::merge_into(line, *bolded, Stat::default());
    assert!(
        folded.enhanced_is_bolded,
        "the wire bolded Intuition's enhanced column, so the fold must say so"
    );
    assert_eq!(
        folded.enhanced,
        Some(StatValue {
            value: 106,
            bonus: 28
        })
    );
    assert_eq!(
        folded.ascended,
        Some(StatValue {
            value: 98,
            bonus: 24
        })
    );
    assert_eq!(
        folded.normal, None,
        "`info` sends two columns, so the base value stays unknown"
    );
}

#[test]
fn a_later_info_does_not_erase_what_info_full_taught() {
    // **The difference between "unknown" and "unchanged".** `info full` teaches
    // the base value; a plain `info` afterwards never mentions it. Replacing
    // rather than merging would erase it on every subsequent `info`.
    let full = cena_model::StatLine::classify(
        "    Strength (STR):   110 (30)    ...  115 (32)    ...  120 (35)",
    )
    .expect("the three-column form classifies");
    let after_full = InfoReport::merge_into(&full, false, Stat::default());
    assert_eq!(
        after_full.normal,
        Some(StatValue {
            value: 110,
            bonus: 30
        }),
        "`info full` taught the base value"
    );

    let plain = cena_model::StatLine::classify("    Strength (STR):   115 (32)    ...  120 (35)")
        .expect("the two-column form classifies");
    let after_plain = InfoReport::merge_into(&plain, false, after_full);
    assert_eq!(
        after_plain.normal,
        Some(StatValue {
            value: 110,
            bonus: 30
        }),
        "and a plain `info` afterwards must NOT erase it -- the line said \
         nothing about the base value, which is not the same as saying it is \
         unknown"
    );
}

#[test]
fn an_expired_enhancive_stops_being_marked() {
    // The opposite direction, and the reason `enhanced_is_bolded` is replaced
    // rather than `or`-ed: an enhancive that ran out stops arriving bolded, and
    // carrying the old `true` forward would make it permanent.
    let line = cena_model::StatLine::classify("   Intuition (INT):    98 (24)    ...   98 (24)")
        .expect("it classifies");
    let previously_enhanced = Stat {
        enhanced_is_bolded: true,
        ..Stat::default()
    };
    let now = InfoReport::merge_into(&line, false, previously_enhanced);
    assert!(
        !now.enhanced_is_bolded,
        "the wire stopped bolding it, so the enhancive is gone"
    );
}

#[test]
fn a_stat_line_outside_a_block_is_ignored() {
    // **A player can say anything** -- the rule `state.rs:306-309` records for
    // the idle warning. Someone typing a stat-shaped line into a chat channel
    // must not rewrite the character sheet.
    let mut blocks = Blocks::default();
    assert!(
        !blocks.offer("    Strength (STR):   999 (99)    ...  999 (99)"),
        "a stat line with no open block is not part of a report"
    );
    assert_eq!(blocks.open(), Block::None);
    assert!(
        blocks.end().is_none(),
        "and nothing accumulated, so there is nothing to apply"
    );
}

#[test]
fn a_block_that_never_terminates_applies_nothing() {
    // Lich's failure mode, handled explicitly. A disconnect mid-`info` leaves
    // its mutex locked and its hold array dirty; the next block's start silently
    // discards the rows and unlocks one level too shallow
    // (`infomon.rb:54`, `parser.rb:239`).
    let mut blocks = Blocks::default();
    let lines = info_blob();
    // Feed the header and three stat lines, then abandon -- a drop mid-report.
    for (line, _) in lines.iter().take(6) {
        blocks.offer(line);
    }
    assert_eq!(blocks.open(), Block::Info, "the block is open");
    blocks.abandon();
    assert_eq!(blocks.open(), Block::None, "and abandoning closes it");
    assert!(
        blocks.end().is_none(),
        "a block abandoned on reconnect applies NOTHING -- a truncated `info` \
         must not leave four stats updated and six stale"
    );
}

#[test]
fn abandoning_leaves_nothing_behind_even_without_an_end() {
    // **The reconnect path, and the mutation that first survived.** Removing
    // `abandon`'s clearing left all 12 tests GREEN, because every other route to
    // the accumulator either drains it (`end` uses `mem::take`) or resets it (the
    // header clears on open). So the clearing looked redundant.
    //
    // It is not. MEASURED with the clearing removed: after `abandon()` with no
    // `end()` -- which is exactly what a reconnect does -- the struct still held
    // the previous session's `Strength` line and its race and profession:
    //
    //   Blocks { open: None, stats: [(Strength, ...)], identity: { race: ... } }
    //
    // That is a stale belief nobody chose to keep, the thing
    // `invalidate_for_reconnect` exists to prevent (`state/reconnect.rs:88-95`).
    // Comparing against a default is what catches it, because the leak is
    // invisible through the public accessors.
    let mut blocks = Blocks::default();
    blocks.offer("Name: X Race: Half-Elf  Profession: Ranger");
    blocks.offer("    Strength (STR):   115 (32)    ...  115 (32)");
    blocks.abandon();
    assert_eq!(
        blocks,
        Blocks::default(),
        "a reconnect must leave the machine indistinguishable from a fresh one"
    );
}

#[test]
fn ending_with_no_block_open_yields_nothing() {
    // Every prompt will call `end()`, and most prompts have no block open. It
    // must be cheap and silent rather than producing an empty report a caller
    // would dutifully apply.
    let mut blocks = Blocks::default();
    assert!(blocks.end().is_none());
    assert!(blocks.end().is_none(), "and it is idempotent");
}

#[test]
fn a_second_report_replaces_the_first() {
    // Two `info` runs with no prompt between them -- possible when commands are
    // typed quickly. The second header must reset, not append, or the report
    // carries twenty stat lines.
    let mut blocks = Blocks::default();
    for (line, _) in info_blob() {
        blocks.offer(&line);
    }
    for (line, _) in info_blob() {
        blocks.offer(&line);
    }
    let report = blocks.end().expect("a report");
    assert_eq!(
        report.stats.len(),
        10,
        "ten stats, not twenty: the second header resets the accumulator"
    );
}
