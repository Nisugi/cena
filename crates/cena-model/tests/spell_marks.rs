//! bigshot's 703 and 1614 lists, kept per creature: Corrupt Essence's blood
//! red haze and Aura of the Arkati's rebuke (`state/creatures/prose.rs`).
//!
//! Every wire line here is SYNTHETIC, built from bigshot's own patterns
//! (`reference/scripts/scripts/bigshot.lic:2767-2774`): each is the creature's
//! bold link, as bigshot's `<pushBold/>...<a exist=...>` capture requires, and
//! the phrase. No committed fixture carries one of these lines.

use cena_model::{GameState, SpellMark};
use cena_protocol::Parser;

fn feed(state: &mut GameState, wire: &str) {
    let mut parser = Parser::new();
    for frame in parser.push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
}

/// A `room objs` listing one bolded hostile creature with its status tag.
fn listed(id: i64, noun: &str, name: &str) -> String {
    format!(
        "<component id='room objs'>  You also see<crtrStatus exist=\"{id}\" hostile=\"1\"/><b> <pushBold/>a <a exist=\"{id}\" noun=\"{noun}\">{name}</a><popBold/></b>.</component>\n"
    )
}

/// A creature's link as a line carries it: bolded.
fn link(id: i64, noun: &str, name: &str) -> String {
    format!("<pushBold/><a exist=\"{id}\" noun=\"{noun}\">{name}</a><popBold/>")
}

fn marked(state: &GameState, id: i64, mark: SpellMark) -> Option<bool> {
    state.creatures().get(id).map(|c| c.marked(mark))
}

#[test]
fn the_blood_red_haze_is_laid_and_lifted_by_the_creatures_own_link() {
    let mut state = GameState::default();
    feed(&mut state, &listed(9200, "kobold", "kobold"));
    assert_eq!(marked(&state, 9200, SpellMark::BloodRedHaze), Some(false));

    // bigshot.lic:2767
    let kobold = link(9200, "kobold", "kobold");
    feed(
        &mut state,
        &format!("The {kobold} is suddenly surrounded by a blood red haze.\n"),
    );
    assert_eq!(marked(&state, 9200, SpellMark::BloodRedHaze), Some(true));
    assert_eq!(
        marked(&state, 9200, SpellMark::Rebuked),
        Some(false),
        "one mark says nothing of the other"
    );

    // bigshot.lic:2769: the link comes AFTER the phrase here.
    feed(
        &mut state,
        &format!("The blood red haze dissipates from around the {kobold}.\n"),
    );
    assert_eq!(marked(&state, 9200, SpellMark::BloodRedHaze), Some(false));
}

#[test]
fn both_rebuke_lines_lay_the_mark_and_recovery_lifts_it() {
    let mut state = GameState::default();
    feed(&mut state, &listed(9210, "troll", "troll"));
    feed(&mut state, &listed(9211, "orc", "orc"));
    let troll = link(9210, "troll", "troll");
    let orc = link(9211, "orc", "orc");

    // bigshot.lic:2771's two alternatives.
    feed(
        &mut state,
        &format!("The {troll} is visibly struggling against your radiant aura!\n"),
    );
    feed(
        &mut state,
        &format!("The {orc} is in awe of your radiant aura!\n"),
    );
    assert_eq!(marked(&state, 9210, SpellMark::Rebuked), Some(true));
    assert_eq!(marked(&state, 9211, SpellMark::Rebuked), Some(true));

    // bigshot.lic:2773
    feed(
        &mut state,
        &format!("The {troll} recovers from being rebuked.\n"),
    );
    assert_eq!(marked(&state, 9210, SpellMark::Rebuked), Some(false));
    assert_eq!(
        marked(&state, 9211, SpellMark::Rebuked),
        Some(true),
        "the orc was not the one that recovered"
    );
}

#[test]
fn the_phrase_is_matched_in_any_case_as_bigshots_patterns_are() {
    let mut state = GameState::default();
    feed(&mut state, &listed(9220, "kobold", "kobold"));
    let kobold = link(9220, "kobold", "kobold");
    feed(
        &mut state,
        &format!("The {kobold} Is Suddenly Surrounded By A Blood Red Haze.\n"),
    );
    assert_eq!(marked(&state, 9220, SpellMark::BloodRedHaze), Some(true));
}

#[test]
fn a_line_with_no_creature_link_marks_nothing() {
    let mut state = GameState::default();
    feed(&mut state, &listed(9230, "kobold", "kobold"));
    // The same words with the creature unlinked, and the caster's own ending
    // message from `data/spell_extras.tsv:161`, which names no creature.
    feed(
        &mut state,
        "The kobold is suddenly surrounded by a blood red haze.\n",
    );
    assert_eq!(marked(&state, 9230, SpellMark::BloodRedHaze), Some(false));
    feed(
        &mut state,
        &format!(
            "The {} is suddenly surrounded by a blood red haze.\n",
            link(9230, "kobold", "kobold")
        ),
    );
    feed(
        &mut state,
        "The blood red haze dissipates from around you.\n",
    );
    assert_eq!(
        marked(&state, 9230, SpellMark::BloodRedHaze),
        Some(true),
        "your own haze ending is not the kobold's"
    );
}

#[test]
fn leaving_the_room_forgets_every_mark_as_bigshot_empties_its_lists() {
    let mut state = GameState::default();
    feed(&mut state, "<nav rm='300'/>\n");
    feed(&mut state, &listed(9240, "kobold", "kobold"));
    let kobold = link(9240, "kobold", "kobold");
    feed(
        &mut state,
        &format!("The {kobold} is suddenly surrounded by a blood red haze.\n"),
    );
    // A re-declaration of the same room is not a move (`room.rs`, `arrive`).
    feed(&mut state, "<nav rm='300'/>\n");
    assert_eq!(marked(&state, 9240, SpellMark::BloodRedHaze), Some(true));

    feed(&mut state, "<nav rm='301'/>\n");
    assert_eq!(
        marked(&state, 9240, SpellMark::BloodRedHaze),
        Some(false),
        "a haze that lifts out of sight is never seen to lift"
    );
}

#[test]
fn each_mark_names_its_spell() {
    assert_eq!(SpellMark::BloodRedHaze.spell(), 703);
    assert_eq!(SpellMark::Rebuked.spell(), 1614);
}
