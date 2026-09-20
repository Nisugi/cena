//! Reading an `info` report out of a prompt-bounded chunk.
//!
//! M3 step 4. The chunker is shared (`state/chunks.rs`) and `InfoReport::read`
//! is a pure function of one chunk, so these tests drive the **whole path**:
//! bytes -> frames -> `GameState::apply` -> chunk -> report -> typed stats.

use cena_model::state::chunks::{Chunk, ChunkLine};
use cena_model::{GameState, InfoReport, Stat, StatKind, StatValue};
use cena_protocol::Parser;

/// Fold the `info` fixture through the real parser and the real `GameState`.
///
/// No `unwrap`/`expect`/`panic!`: the workspace denies all three and
/// `clippy.toml`'s allowance covers `#[test]` functions only. An unreadable
/// fixture yields no frames, which every assertion below fails on.
fn state_from_fixture() -> GameState {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../cena-protocol/tests/fixtures/character_info.xml");
    let bytes = std::fs::read(&path).unwrap_or_default();
    let mut parser = Parser::new();
    let mut state = GameState::default();
    for frame in parser.push_bytes(&bytes) {
        state.apply(&frame);
    }
    for frame in parser.push_bytes(b"\n") {
        state.apply(&frame);
    }
    state
}

/// A chunk built from plain lines, for the chunk-level tests.
fn chunk_of(lines: &[&str]) -> Chunk {
    let mut chunk = Chunk::default();
    for text in lines {
        chunk.push_line(ChunkLine {
            text: (*text).to_owned(),
            bold: Vec::new(),
        });
    }
    chunk
}

#[test]
fn the_whole_path_produces_ten_typed_stats() {
    // The end-to-end assertion: the fixture's prompt closes the chunk, the
    // chunk is read, and the stats land in `GameState`.
    let state = state_from_fixture();
    let kinds: Vec<StatKind> = state.character.stats.keys().copied().collect();
    assert_eq!(
        kinds,
        StatKind::ALL.to_vec(),
        "all ten stats reach GameState. If this is empty the chunk never \
         closed; the prompt is the only thing that closes it."
    );
}

#[test]
fn the_identity_reaches_game_state() {
    let state = state_from_fixture();
    let id = &state.character.identity;
    assert_eq!(id.race.as_deref(), Some("Half-Elf"));
    assert_eq!(
        id.profession.as_deref(),
        Some("Ranger"),
        "the `(shown as: Hero)` title is stripped -- it is a title, not a \
         profession, and Lich strips it too"
    );
    assert_eq!(id.gender.as_deref(), Some("Male"));
    assert_eq!(id.age, Some(36));
}

#[test]
fn only_the_bolded_stats_are_marked_enhanced() {
    // Bold lives in the FRAMES, so this passing at all proves the signal
    // survived reassembly into the chunk.
    let state = state_from_fixture();
    let bolded: Vec<StatKind> = state
        .character
        .stats
        .iter()
        .filter(|(_, s)| s.enhanced_is_bolded)
        .map(|(k, _)| *k)
        .collect();
    assert_eq!(
        bolded,
        vec![StatKind::Intuition, StatKind::Wisdom],
        "exactly Intuition and Wisdom are enhanced in this capture. Empty means \
         the bold was lost; all ten means it was attributed to every line."
    );
}

#[test]
fn an_enhanced_stat_keeps_both_of_its_columns() {
    let state = state_from_fixture();
    let intuition = state
        .character
        .stats
        .get(&StatKind::Intuition)
        .copied()
        .unwrap_or_default();
    assert_eq!(
        intuition.ascended,
        Some(StatValue {
            value: 98,
            bonus: 24
        })
    );
    assert_eq!(
        intuition.enhanced,
        Some(StatValue {
            value: 106,
            bonus: 28
        })
    );
    assert_eq!(
        intuition.normal, None,
        "`info` sends two columns, so the base value stays unknown"
    );
}

#[test]
fn the_character_name_is_not_taken_from_the_info_line() {
    // `infomon/parser.rb:237`: "name captured here, but do not rely on it - use
    // XML instead". `Identity` has no name field at all, which is the
    // enforcement -- there is nowhere for a careless port to put it.
    let state = state_from_fixture();
    let debug = format!("{:?}", state.character.identity);
    assert!(
        !debug.contains("Ashryn"),
        "identity must not carry the name from `info`: {debug}"
    );
}

#[test]
fn a_chunk_without_the_header_is_not_a_report() {
    // **A player can say anything** -- `state.rs:306-309`'s rule for the idle
    // warning. A stat-shaped line typed into a channel must not rewrite the
    // character sheet.
    let chunk = chunk_of(&["    Strength (STR):   999 (99)    ...  999 (99)"]);
    assert!(
        InfoReport::read(&chunk).is_none(),
        "without `Name: ... Race: ... Profession:` this is not an info report"
    );
}

#[test]
fn a_header_with_no_stats_yields_nothing() {
    // A caller must not be handed an empty update to apply.
    let chunk = chunk_of(&["Name: X Race: Half-Elf  Profession: Ranger"]);
    assert!(InfoReport::read(&chunk).is_none());
}

#[test]
fn an_empty_chunk_yields_nothing() {
    assert!(InfoReport::read(&Chunk::default()).is_none());
}

#[test]
fn a_second_header_in_one_chunk_replaces_the_first() {
    // Two `info` runs with no prompt between them. The later wins, as it would
    // if they had arrived in separate chunks.
    let chunk = chunk_of(&[
        "Name: X Race: Half-Elf  Profession: Ranger",
        "    Strength (STR):   115 (32)    ...  115 (32)",
        "Name: X Race: Half-Elf  Profession: Ranger",
        "        Aura (AUR):    98 (24)    ...   98 (24)",
    ]);
    let report = InfoReport::read(&chunk).expect("a report");
    assert_eq!(
        report.stats.len(),
        1,
        "one stat, not two: the second header resets the accumulator"
    );
    assert_eq!(report.stats[0].0, StatKind::Aura);
}

#[test]
fn a_chunk_is_bounded_and_says_when_it_truncated() {
    // Lich caps its buffer at 200 and drops the oldest (`combat/tracker.rb`).
    // A report interrupted by a disconnect must not grow without limit, and a
    // truncated chunk is a fact a consumer may refuse to act on (Rule 2.2).
    let mut chunk = Chunk::default();
    for i in 0..(cena_model::MAX_CHUNK_LINES + 5) {
        chunk.push_line(ChunkLine {
            text: format!("line {i}"),
            bold: Vec::new(),
        });
    }
    assert_eq!(chunk.lines().len(), cena_model::MAX_CHUNK_LINES);
    assert_eq!(chunk.dropped(), 5);
    assert!(chunk.is_truncated());
    assert_eq!(
        chunk.lines()[0].text,
        "line 5",
        "the OLDEST are dropped: the newest lines are the ones a terminator \
         would have applied"
    );
}

#[test]
fn a_reconnect_drops_a_chunk_in_progress() {
    // A command whose output was interrupted will never see its terminating
    // prompt, so the lines it carried describe a report that cannot complete.
    //
    // This is the failure Lich handles by accident: its accumulator is guarded
    // only by a mutex, so a disconnect mid-`info` leaves it locked and dirty and
    // the NEXT block silently discards the rows (`infomon.rb:54`).
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../cena-protocol/tests/fixtures/character_info.xml");
    let bytes = std::fs::read(&path).unwrap_or_default();
    let mut parser = Parser::new();
    let mut state = GameState::default();
    // Feed everything EXCEPT the terminating prompt, so a chunk stays open.
    let frames = parser.push_bytes(&bytes);
    for frame in frames
        .iter()
        .filter(|f| !matches!(f, cena_protocol::frame::Frame::Prompt { .. }))
    {
        state.apply(frame);
    }
    assert!(
        state.character.stats.is_empty(),
        "guard: with no prompt, nothing should have been applied yet"
    );

    // Guard: the chunk really did accumulate, or this test passes vacuously.
    assert!(
        state.open_chunk_len() > 0,
        "with no prompt the lines should still be sitting in the open chunk"
    );

    state.invalidate_for_reconnect();

    // Comparing whole states would be wrong here: `vitals` are deliberately
    // KEPT across a reconnect (`state/reconnect.rs`), so the fixture's bars
    // survive and a fresh state has none. The property under test is narrower.
    assert_eq!(
        state.open_chunk_len(),
        0,
        "the interrupted chunk must be dropped, not carried into the next \
         generation where a later prompt would commit it"
    );
}

#[test]
fn a_line_from_another_stream_cannot_join_the_report() {
    // **The injection this guard exists for**, and the mutation that first
    // survived without it.
    //
    // A report is prose in the main window. A `thoughts` or `bounty` stream
    // carries someone ELSE'S words -- and a player can type anything, including
    // a stat-shaped line. Lich gates the same way, refusing lines while
    // `XMLData.in_stream` is true (`combat/tracker.rb:481`).
    //
    // MEASURED with the guard removed: the ESP line overwrote the real stat,
    // giving Strength 999 (99) instead of 115 (32). Ordering matters -- the
    // injected line has to arrive AFTER the real one to win, which is why this
    // test puts it there.
    let wire = concat!(
        "Name: X Race: Half-Elf  Profession: Ranger
",
        "    Strength (STR):   115 (32)    ...  115 (32)
",
        "<pushStream id='thoughts'/>    Strength (STR):   999 (99)    ...  999 (99)<popStream/>
",
        "<prompt time='1'>&gt;</prompt>
",
    );
    let mut parser = Parser::new();
    let mut state = GameState::default();
    for frame in parser.push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
    let strength = state
        .character
        .stats
        .get(&StatKind::Strength)
        .copied()
        .unwrap_or_default();
    assert_eq!(
        strength.ascended,
        Some(StatValue {
            value: 115,
            bonus: 32
        }),
        "a stat-shaped line on the `thoughts` stream must not reach the report;          without the gate this reads 999 (99)"
    );
}

#[test]
fn a_later_info_does_not_erase_what_info_full_taught() {
    // **The difference between "unknown" and "unchanged".** `info full` teaches
    // the base value; a plain `info` afterwards never mentions it.
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
        })
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
        "a plain `info` must NOT erase it -- the line said nothing about the \
         base value, which is not the same as saying it is unknown"
    );
}

#[test]
fn an_expired_enhancive_stops_being_marked() {
    // Bold is a per-report observation: an enhancive that ran out stops
    // arriving bolded, and carrying the old `true` forward would make it
    // permanent.
    let line = cena_model::StatLine::classify("   Intuition (INT):    98 (24)    ...   98 (24)")
        .expect("it classifies");
    let previously = Stat {
        enhanced_is_bolded: true,
        ..Stat::default()
    };
    assert!(!InfoReport::merge_into(&line, false, previously).enhanced_is_bolded);
}

#[test]
fn a_shrouded_character_keeps_its_real_identity() {
    // **Shroud of Deception (spell 1212) falsifies race, profession, gender and
    // age in `info` output.** Lich refuses to store those four while it is
    // active (`infomon/parser.rb:243`, `:249`) and force-STOPs the spell before
    // syncing (`infomon/cli.rb:11-18`).
    //
    // Persisting a shrouded identity is permanent damage: nothing later says
    // "that was a lie".
    let mut state = state_from_fixture();
    let real = state.character.identity.clone();
    assert_eq!(
        real.race.as_deref(),
        Some("Half-Elf"),
        "guard: learned once"
    );

    state.character.shrouded = true;
    let chunk = chunk_of(&[
        "Name: X Race: Burghal Gnome  Profession: Bard",
        "    Strength (STR):   200 (99)    ...  200 (99)",
    ]);
    if let Some(report) = InfoReport::read(&chunk) {
        state.character.apply_info(&report);
    }

    assert_eq!(
        state.character.identity.race.as_deref(),
        Some("Half-Elf"),
        "the shroud's false race must NOT overwrite the real one"
    );
    assert_eq!(
        state.character.identity.profession.as_deref(),
        Some("Ranger"),
        "nor the profession"
    );
    // **But the numbers ARE taken**, because the shroud does not touch them.
    let strength = state
        .character
        .stats
        .get(&StatKind::Strength)
        .copied()
        .unwrap_or_default();
    assert_eq!(
        strength.ascended,
        Some(StatValue {
            value: 200,
            bonus: 99
        }),
        "stats are stored while shrouded -- this is a partial refusal, not a \
         dropped report"
    );
}
