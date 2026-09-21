//! `profile`, typed: what it shows that nothing else holds.
//!
//! `character_profile.xml` is a real `profile` run, cut 2026-09-21 from a live
//! log. Every assertion here was read off that capture, not imagined: the
//! bracketed age, the `Master of` line that carries no number, and the
//! achievement labels are all the game's own wording.
//!
//! **It arrives in the MAIN window**, not in the `charprofile` stream its own
//! `streamWindow` declares -- see `character/profile.rs`, which records the
//! measurement, and `FIXTURES.md`. These tests were written against the stream
//! and all eleven failed; that is what found it.

use cena_model::GameState;
use cena_model::Society;
use cena_protocol::Parser;

const PROFILE: &str = include_str!("../../cena-protocol/tests/fixtures/character_profile.xml");

fn fed(wire: &str) -> GameState {
    let mut state = GameState::default();
    for frame in Parser::new().push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
    state
}

#[test]
fn a_real_profile_teaches_what_nothing_else_holds() {
    let p = &fed(PROFILE).character.profile;
    assert!(p.is_stated());
    assert_eq!(p.title.as_deref(), Some("Hero"));
    assert_eq!(p.birthday.as_deref(), Some("8/28/5090"));
    assert_eq!(p.age, Some(36), "the number inside the brackets");
}

#[test]
fn the_description_lines_are_kept_in_wire_order() {
    let p = &fed(PROFILE).character.profile;
    assert_eq!(p.description.len(), 5, "{:?}", p.description);
    assert!(p.description[0].starts_with("He is taller than average"));
    assert!(
        p.description.last().expect("five lines").contains("spider"),
        "the tattoo line is last: {:?}",
        p.description
    );
}

#[test]
fn every_affiliation_is_kept_even_the_ones_standing_cannot_hold() {
    let p = &fed(PROFILE).character.profile;
    assert_eq!(p.affiliations.len(), 7, "{:?}", p.affiliations);
    // The three that have no field anywhere else, which is why the section is
    // kept whole rather than only routed into `Standing`.
    assert!(p.affiliations.iter().any(|a| a == "Follower of Zelia"));
    assert!(
        p.affiliations
            .iter()
            .any(|a| a == "Attuned to the Element of Earth")
    );
    assert!(p.affiliations.iter().any(|a| a == "No Guild affiliation"));
}

#[test]
fn achievements_are_labelled_pairs() {
    let p = &fed(PROFILE).character.profile;
    assert_eq!(p.achievement("Number of warcamps destroyed"), Some("1021"));
    assert_eq!(
        p.achievement("Strongest foe vanquished"),
        Some("a leopard ogre (level 140)"),
        "the value keeps its own brackets"
    );
    assert_eq!(p.achievement("Most difficult lock picked"), Some("None"));
}

#[test]
fn the_history_section_is_read() {
    let p = &fed(PROFILE).character.profile;
    assert_eq!(p.history, vec!["Formerly known as Ibbo.".to_owned()]);
}

#[test]
fn the_society_and_citizenship_reach_standing_not_a_second_copy() {
    // Rule 2.2a. `profile` states both, and `Standing` owns both -- so a
    // caller asking "what society" gets rank 20 from the SAME place whether
    // the fact came from a chunk or from this stream.
    let state = fed(PROFILE);
    assert_eq!(
        state.character.standing.society,
        Some(Some(Society::GuardiansOfSunfist))
    );
    assert_eq!(
        state.character.standing.society_rank,
        Some(20),
        "`Master of` carries no number; the society's max rank fills it"
    );
    assert_eq!(
        state.character.standing.citizenship,
        Some(Some("Kraken's Fall".to_owned()))
    );
    assert_eq!(state.character.identity.age, Some(36), "routed to Identity");

    // **The rank, from the profile's own line ALONE.** MEASURED by mutation:
    // the capture also contains the indented `society` report, which fills the
    // rank by itself -- so the assertion above passed with the profile
    // reader's rank deleted.
    use cena_model::state::character::standing::{Affiliation, SocietyEvent, profile_affiliation};
    assert_eq!(
        profile_affiliation("Master of the Guardians of Sunfist"),
        Some(Affiliation::Society(SocietyEvent::Report {
            society: Some(Society::GuardiansOfSunfist),
            rank: Some(20),
            master: true,
        }))
    );
    // A member's rank is NOT on this line and must stay unknown rather than
    // becoming 1 (`plan/dazzling` bug 3).
    assert_eq!(
        profile_affiliation("Member of the Order of Voln"),
        Some(Affiliation::Society(SocietyEvent::Report {
            society: Some(Society::OrderOfVoln),
            rank: None,
            master: false,
        }))
    );
}

#[test]
fn what_it_teaches_is_marked_for_the_store() {
    let mut state = fed(PROFILE);
    let taught = state.character.take_taught();
    assert!(
        taught.contains(&cena_model::state::character::snapshot::Group::Standing),
        "{taught:?}"
    );
}

#[test]
fn a_chunk_that_is_not_a_profile_changes_nothing() {
    // The reader runs on EVERY chunk, so it must recognise a profile rather
    // than assume one.
    //
    // **The lines must be ones a profile WOULD read**, or this passes with the
    // header check deleted: MEASURED by mutation. `Title:` and an achievement
    // pair, with no `PERSONAL INFORMATION` above them -- which is also a real
    // hazard, since a player can type either into a channel.
    let state = fed(
        "Title: Hero\nStrongest foe vanquished: a rat (level 1)\n<prompt time=\"1\">&gt;</prompt>\n",
    );
    assert!(!state.character.profile.is_stated());
    assert_eq!(state.character.profile.title, None);
    assert!(state.character.profile.achievements.is_empty());
}

#[test]
fn a_second_run_replaces_the_first() {
    // `profile` prints everything every time, so a title that is no longer
    // shown must not survive from the last run.
    let mut wire = PROFILE.to_owned();
    // In the main window, as the capture shows -- a second run with no title
    // and no achievements.
    wire.push_str(
        "<pushBold/>PERSONAL INFORMATION\n\
          <popBold/>Name: Nisugi\n<prompt time=\"2\">&gt;</prompt>\n",
    );
    let state = fed(&wire);
    assert!(state.character.profile.is_stated());
    assert_eq!(
        state.character.profile.title, None,
        "the old title survived"
    );
    assert!(state.character.profile.achievements.is_empty());
}

#[test]
fn a_reconnect_forgets_it_because_nothing_resends_it() {
    let mut state = fed(PROFILE);
    state.invalidate_for_reconnect();
    assert!(!state.character.profile.is_stated());
    assert_eq!(state.character.profile.title, None);
}

#[test]
fn the_level_on_the_profession_line_is_not_read_as_anything() {
    // `blocks.rs:155`: Lich discards `Level:` here and says "do not rely on
    // it - use XML". The `expr` dialog owns the level.
    let state = fed(PROFILE);
    assert_eq!(state.character.experience.level, None);
}
