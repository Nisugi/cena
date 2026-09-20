//! Status onset and expiry lines, including the one lookbehind in the data.
//!
//! `statuses.rb:118` is the single pattern of 954 that Rust's `regex` cannot
//! compile as written: a `kneeling` onset guarded by two negative lookbehinds,
//! so that *"X, dazed and is knocked to his knees"* does not read `X, dazed
//! and` as the target. Ported as a positive match plus a line-level veto
//! (`defs::HAND_PORTED`), the shape `crit/match_index.rs` uses for the one
//! lookahead in the crit tables.

use cena_model::state::chunks::ChunkLine;
use cena_model::{GameState, StatusAction, StatusLine, StatusName};
use cena_protocol::Parser;

fn line_of(wire: &str) -> ChunkLine {
    let mut parser = Parser::new();
    let mut state = GameState::default();
    for frame in parser.push_bytes(format!("{wire}\n").as_bytes()) {
        state.apply(&frame);
    }
    state
        .open_chunk()
        .lines()
        .first()
        .cloned()
        .unwrap_or_default()
}

fn bolded(id: i64, noun: &str, name: &str) -> String {
    format!("<pushBold/><a exist=\"{id}\" noun=\"{noun}\">{name}</a><popBold/>")
}

/// **The kneeling onset, and the lookbehind's veto.**
#[test]
fn a_knockdown_to_the_knees_is_kneeling_unless_the_veto_fires() {
    let line = line_of(&format!(
        "{} is knocked to his knees!",
        bolded(7, "kobold", "a kobold")
    ));
    let status = StatusLine::classify(&line).expect("a status");
    assert_eq!(status.status, StatusName::Kneeling);
    assert_eq!(status.action, StatusAction::Add);
    assert_eq!(status.target.and_then(|a| a.id), Some(7));

    // The lookbehind's case: the words before ` is knocked` are a status
    // rider, not a name. Lich refuses the match; so does the veto.
    let vetoed = line_of("A kobold, dazed and is knocked to his knees!");
    assert_ne!(
        StatusLine::classify(&vetoed).map(|s| s.status),
        Some(StatusName::Kneeling),
        "the veto must refuse the line the lookbehind refused"
    );
    let vetoed = line_of("A kobold, paralyzed and is driven to its knees!");
    assert_ne!(
        StatusLine::classify(&vetoed).map(|s| s.status),
        Some(StatusName::Kneeling)
    );
}

/// A status line about us has no target.
#[test]
fn a_status_about_us_has_no_target() {
    let status = StatusLine::classify(&line_of("You are blinded!")).expect("a status");
    assert_eq!(status.status, StatusName::Blind);
    assert_eq!(status.action, StatusAction::Add);
    assert!(status.is_self());
    assert_eq!(status.target, None);
}

/// Onset and expiry are paired per status.
#[test]
fn an_expiry_line_removes_the_status() {
    let line = line_of(&format!(
        "{} vision clears.",
        bolded(9, "orc", "A greater orc's")
    ));
    let status = StatusLine::classify(&line).expect("a status");
    assert_eq!(status.status, StatusName::Blind);
    assert_eq!(status.action, StatusAction::Remove);
    assert_eq!(status.target.as_ref().and_then(|a| a.id), Some(9));
    assert_eq!(
        status.target.as_ref().map(|a| a.name.as_str()),
        Some("A greater orc"),
        "the possessive inside the link is stripped"
    );
}

/// The floor positions are one channel.
#[test]
fn the_floor_positions_are_mutually_exclusive() {
    for s in [StatusName::Prone, StatusName::Sitting, StatusName::Kneeling] {
        assert!(s.is_position(), "{s:?}");
    }
    for s in [StatusName::Stunned, StatusName::Blind, StatusName::Webbed] {
        assert!(!s.is_position(), "{s:?}");
    }
}

/// A knockdown line is prone.
#[test]
fn a_knockdown_is_prone() {
    let line = line_of(&format!(
        "{} is knocked to the ground!",
        bolded(7, "kobold", "a kobold")
    ));
    let status = StatusLine::classify(&line).expect("a status");
    assert_eq!(status.status, StatusName::Prone);
    assert_eq!(status.action, StatusAction::Add);
}
