//! Tier 1: goldens over the committed fixtures. Run on every build.
//!
//! The fixtures are real wire traffic, cut from the corpus and scrubbed
//! (`tests/FIXTURES.md` records provenance; `tests/fixtures_are_scrubbed.rs`
//! proves the redaction held). They are 12 KB in total, inside the ~20-40 KB
//! budget, so this tier costs nothing to run every time.
//!
//! # These assert meaning, not a snapshot
//!
//! `plan/05` §0 asks what makes a test go RED. A snapshot of the whole frame
//! stream goes *different* on any change and gets regenerated; Vellum's
//! parser golden is regenerated with `UPDATE_PARSER_GOLDEN=1` and a reviewer
//! waves the diff through. So each test here names the fact it protects --
//! the room id, the vitals numerator, the unknown tag surviving -- and fails
//! with that fact in the message.

use cena_protocol::Parser;
use cena_protocol::frame::{Amount, Frame};

/// Parse a committed fixture into frames, through the byte-level read
/// boundary so the fixtures exercise reassembly too.
fn parse_fixture(name: &str) -> Vec<Frame> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    // No `unwrap`/`expect`/`panic!` here: clippy.toml's allow-*-in-tests
    // covers `#[test]` functions, not helpers, and the workspace denies all
    // three. An unreadable fixture yields no bytes and therefore no frames,
    // which every caller already asserts against -- `assert!(!frames.
    // is_empty())` and the specific-fact assertions all fail loudly.
    let bytes = std::fs::read(&path).unwrap_or_default();
    let mut parser = Parser::new();
    let mut frames = parser.push_bytes(&bytes);
    // A fixture need not end in a newline; flush whatever is pending.
    frames.extend(parser.push_bytes(b"\n"));
    frames
}

// ---------------------------------------------------------------------------
// Prompt
// ---------------------------------------------------------------------------

#[test]
fn every_prompt_in_the_fixture_is_typed_with_its_time() {
    let frames = parse_fixture("prompt.xml");
    let times: Vec<&str> = frames
        .iter()
        .filter_map(|f| match f {
            Frame::Prompt { time, .. } => Some(time.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        times,
        vec!["1764475407", "1764475408", "1764475408", "1764475757"],
        "prompt.xml has four prompts and their times are the session clock"
    );
}

#[test]
fn a_prompt_decodes_its_entity_body() {
    let frames = parse_fixture("prompt.xml");
    let text = frames
        .iter()
        .find_map(|f| match f {
            Frame::Prompt { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .expect("prompt.xml carries a prompt");
    assert_eq!(text, ">", "`&gt;` must reach the caller decoded, not raw");
}

#[test]
fn a_prompt_closes_streams_nobody_popped() {
    // The resync barrier. Without it, an unbalanced pushStream leaks the
    // stream onto every later line -- a torn room buffer becomes a wrong room.
    let mut parser = Parser::new();
    let frames = parser.parse_line("<pushStream id='familiar'/>text<prompt time='1'>&gt;</prompt>");
    assert!(
        frames
            .iter()
            .any(|f| matches!(f, Frame::StreamPopForced { id } if id == "familiar")),
        "an open stream must be force-closed at the prompt; got {frames:#?}"
    );
    // And the next line is back on the main stream, not still in `familiar`.
    let after = parser.parse_line("ordinary prose");
    assert!(
        after
            .iter()
            .any(|f| matches!(f, Frame::Text(t) if t.stream.is_empty())),
        "after a prompt, text belongs to the main window again; got {after:#?}"
    );
}

#[test]
fn a_prompt_drops_orphaned_bold() {
    let mut parser = Parser::new();
    let _ = parser.parse_line("<pushBold/>hostile<prompt time='1'>&gt;</prompt>");
    let after = parser.parse_line("ordinary prose");
    let bold = after.iter().find_map(|f| match f {
        Frame::Text(t) => Some(t.style.bold_depth),
        _ => None,
    });
    assert_eq!(
        bold,
        Some(0),
        "orphaned <pushBold> must not leak past the prompt onto later lines"
    );
}

// ---------------------------------------------------------------------------
// Vitals -- and the parse trap the corpus findings flagged
// ---------------------------------------------------------------------------

#[test]
fn vitals_numbers_come_from_text_not_from_the_percentage() {
    // THE bug worth a golden: `value=` is a percent, the numbers are in
    // `text=`. A parser reading value= would report 99 health out of nothing.
    let frames = parse_fixture("vitals.xml");
    let health = frames
        .iter()
        .find_map(|f| match f {
            Frame::ProgressBar(bar) if bar.id == "health" => Some(bar),
            _ => None,
        })
        .expect("vitals.xml carries a health bar");
    assert_eq!(health.percent, 99, "value= is the percentage");
    assert_eq!(
        health.amount,
        Some(Amount {
            current: 225,
            max: 226
        }),
        "the real numbers live in text='health 225/226'"
    );
    assert_ne!(
        i32::try_from(health.percent).ok(),
        health.amount.map(|a| a.current),
        "if these were equal the fixture could not distinguish the two \
         readings and this test would be decoration"
    );
}

#[test]
fn the_spirit_bar_distinguishes_value_from_text_most_sharply() {
    // text='spirit 8/9' with value='88'. Reading value= gives 88, not 8.
    let frames = parse_fixture("vitals.xml");
    let spirit = frames
        .iter()
        .filter_map(|f| match f {
            Frame::ProgressBar(bar) if bar.id == "spirit" => Some(bar),
            _ => None,
        })
        .find(|bar| bar.percent == 88)
        .expect("vitals.xml carries a spirit bar at 88%");
    assert_eq!(
        spirit.amount,
        Some(Amount { current: 8, max: 9 }),
        "8/9 is 88%, so a parser reading value= reports 88 spirit"
    );
}

#[test]
fn every_vitals_bar_knows_which_dialog_it_arrived_in() {
    // `health` in minivitals and `health2` in injuries carry the same numbers;
    // the enclosing dialogData id is what tells them apart.
    let frames = parse_fixture("vitals.xml");
    let minivitals: Vec<&str> = frames
        .iter()
        .filter_map(|f| match f {
            Frame::ProgressBar(bar) if bar.dialog.as_deref() == Some("minivitals") => {
                Some(bar.id.as_str())
            }
            _ => None,
        })
        .collect();
    for want in ["health", "mana", "stamina", "spirit"] {
        assert!(
            minivitals.contains(&want),
            "{want} must be attributed to the minivitals dialog; got {minivitals:?}"
        );
    }
    assert!(
        frames.iter().any(|f| matches!(
            f,
            Frame::ProgressBar(bar) if bar.id == "health2" && bar.dialog.as_deref() == Some("injuries")
        )),
        "health2 belongs to the injuries dialog, not minivitals"
    );
}

#[test]
fn a_label_only_bar_reports_no_amount_rather_than_a_fabricated_max() {
    let frames = parse_fixture("vitals.xml");
    let mind = frames
        .iter()
        .find_map(|f| match f {
            Frame::ProgressBar(bar) if bar.id == "mindState" => Some(bar),
            _ => None,
        })
        .expect("vitals.xml carries a mindState bar");
    assert_eq!(mind.text, "numbed");
    assert_eq!(
        mind.amount, None,
        "Vellum returns (90, 100) here, inventing a maximum the wire never \
         sent and which a caller cannot tell from a real one"
    );
}

#[test]
fn the_secondary_vitals_fixture_parses_without_unknown_tags() {
    let frames = parse_fixture("vitals_secondary.xml");
    let unknown: Vec<&Frame> = frames
        .iter()
        .filter(|f| matches!(f, Frame::UnknownTag { .. } | Frame::MalformedTag { .. }))
        .collect();
    assert!(
        unknown.is_empty(),
        "vitals_secondary.xml is real wire traffic and every tag in it is in \
         KNOWN_WIRE_TAGS, so an unknown here means the port lost a tag: \
         {unknown:#?}"
    );
    assert!(
        frames.iter().any(|f| matches!(f, Frame::RoundTime { .. })),
        "the fixture carries a roundTime"
    );
}

#[test]
fn no_committed_fixture_produces_an_unknown_or_malformed_frame() {
    // The whole Tier 1 corpus at once. These are real, unmodified wire lines:
    // anything unknown means the ported vocabulary has a hole.
    for name in [
        "room.xml",
        "prompt.xml",
        "vitals.xml",
        "vitals_secondary.xml",
    ] {
        let frames = parse_fixture(name);
        assert!(!frames.is_empty(), "{name} produced no frames at all");
        for frame in &frames {
            assert!(
                !matches!(frame, Frame::UnknownTag { .. } | Frame::MalformedTag { .. }),
                "{name}: {frame:#?}"
            );
        }
    }
}
