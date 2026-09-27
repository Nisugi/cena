//! Each widget draws what it shows, found as a screen reader finds it.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use super::*;
use crate::fixture::{snapshot, story};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

/// Every kind, each in a strip of its own height, over `snapshot`.
fn drawn<'a>(snapshot: Option<Snapshot>, hunt: Option<HuntView>) -> Harness<'a, ()> {
    let story = story();
    Harness::builder()
        .with_size((520.0, 2400.0))
        .build_ui(move |ui| {
            let seen = Seen {
                snapshot: snapshot.as_ref(),
                story: &story,
                hunt: hunt.as_ref(),
                who: None,
            };
            for widget in Widget::ALL {
                let height = widget.size().y.min(120.0);
                ui.allocate_ui(egui::vec2(500.0, height), |ui| {
                    widget.draw(ui, &seen, Id::new(("widget-test", widget)));
                });
            }
        })
}

#[test]
fn every_kind_is_listed_once_under_a_name_of_its_own() {
    let kinds: HashSet<Widget> = Widget::ALL.into_iter().collect();
    assert_eq!(kinds.len(), Widget::ALL.len());
    let names: HashSet<&str> = Widget::ALL.iter().map(|widget| widget.name()).collect();
    assert_eq!(names.len(), Widget::ALL.len());
}

#[test]
fn each_widget_draws_what_it_shows() {
    let mut ashryn = snapshot();
    // The clocks count on the wall clock; a test does not.
    ashryn.state.roundtime_ends = None;
    let hunt = HuntView {
        running: "ojandhaart".to_owned(),
        phase: "resting (out of mana)".to_owned(),
        doing: "waiting 5s".to_owned(),
        target: None,
        waiting: Some("mana 30%, wants 50%".to_owned()),
    };
    let harness = drawn(Some(ashryn), Some(hunt));
    for label in [
        "You swing a steel broadsword at a kobold!",
        "> look",
        "HP 348/400 87%",
        "MP 48/120 40%",
        "SP ?",
        "Sp ?",
        "Right: empty",
        "Left: a steel broadsword",
        "RT —",
        "CT —",
        "[Rawknuckle's, Watering Hole]",
        "Description unknown",
        "a kobold",
        "Also here:",
        "Maravel",
        "Obvious exits: north, out",
        "Hunt: resting until mana is 50%.",
        "ojandhaart",
        "Waiting: mana 30%, wants 50%",
        "Prepared: none",
    ] {
        assert!(harness.query_by_label(label).is_some(), "{label}");
    }
}

/// Before the game has said anything, each widget says what it does not
/// know rather than drawing nothing.
#[test]
fn a_widget_says_what_it_does_not_know() {
    let harness = drawn(None, None);
    for (label, count) in [
        ("HP ?", 1),
        ("Right: ?", 1),
        ("Left: ?", 1),
        ("RT —", 1),
        ("Room unknown", 1),
        ("Description unknown", 1),
        ("unknown", 3),
        ("Exits unknown", 1),
        ("No hunt running.", 1),
    ] {
        assert_eq!(harness.query_all_by_label(label).count(), count, "{label}");
    }
}

/// A one-line widget stays one line in a narrow cell, however long what it
/// says: cut short, never wrapped onto a line its cell has not got.
#[test]
fn a_one_line_widget_stays_one_line() {
    let mut ashryn = snapshot();
    ashryn.state.room.title =
        Some("[Wehnimer's Landing, Town Square Central, beside the old fountain]".to_owned());
    ashryn.state.room.exits = Some(
        [
            "north",
            "northeast",
            "east",
            "southeast",
            "south",
            "southwest",
            "west",
            "out",
        ]
        .map(str::to_owned)
        .to_vec(),
    );
    ashryn.state.apply(&cena_session::Frame::LeftHand {
        item: "a gleaming vultite greatsword etched with twisting runes".to_owned(),
        link: None,
    });
    let heights = Arc::new(Mutex::new(Vec::new()));
    let seen_heights = Arc::clone(&heights);
    let story = story();
    let mut harness = Harness::builder()
        .with_size((400.0, 400.0))
        .build_ui(move |ui| {
            let seen = Seen {
                snapshot: Some(&ashryn),
                story: &story,
                hunt: None,
                who: None,
            };
            let mut heights = seen_heights
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            heights.clear();
            for widget in [Widget::RoomTitle, Widget::Exits, Widget::LeftHand] {
                let used = ui
                    .allocate_ui(egui::vec2(120.0, LINE), |ui| {
                        widget.draw(ui, &seen, Id::new(("one-line", widget)));
                    })
                    .response
                    .rect
                    .height();
                heights.push((widget, used));
            }
        });
    harness.run();
    let heights = heights
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(heights.len(), 3);
    for (widget, used) in heights.iter() {
        assert!(*used <= LINE, "{widget:?} took {used}");
    }
}

/// What the character is and has, each widget saying it as the game did.
#[test]
fn the_characters_widgets_say_what_it_is() {
    let mut ashryn = snapshot();
    let character = &mut ashryn.state.character;
    character.stance = Some("defensive (100%)".to_owned());
    character.stance_percent = Some(100);
    character.encumbrance = Some("Light".to_owned());
    character.encumbrance_percent = Some(20);
    character.encumbrance_detail = Some("You feel slightly weighed down.".to_owned());
    let experience = &mut character.experience;
    experience.mind_state = Some("clear as a bell".to_owned());
    experience.mind_percent = Some(0);
    experience.next_level = Some("11999265 experience".to_owned());
    experience.next_level_percent = Some(40);
    experience.level = Some("Level 100".to_owned());
    experience.physical_training = Some(3673);
    experience.mental_training = Some(0);
    experience.total_experience = Some(68_809_898);
    experience.field_experience = Some(12);
    experience.field_experience_max = Some(1403);
    experience.ascension_experience = Some(24_904_977);
    let standing = &mut character.standing;
    standing.society = Some(Some(cena_session::Society::CouncilOfLight));
    standing.society_rank = Some(20);
    standing.covert_arts_charges = Some(120);
    standing.suffused = Some(1_500);
    standing.shadow_essence = Some(3);
    ashryn.state.prepared = Some("Spirit Warding I".to_owned());
    let harness = drawn(Some(ashryn), None);
    for label in [
        "Stance: defensive (100%)",
        "Encumbrance: Light 20%",
        "You feel slightly weighed down.",
        "Mind: clear as a bell 0%",
        "Next level: 11999265 experience 40%",
        "Level 100",
        "PTPs 3,673 · MTPs 0",
        "Total 68,809,898",
        "Field 12/1,403",
        "Ascension 24,904,977",
        "Prepared: Spirit Warding I",
        "Council of Light, rank 20",
        "Covert Arts charges: 120/200",
        "Suffused: 1,500",
        "Shadow essence: 3/5",
        "No objectives",
    ] {
        assert!(harness.query_by_label(label).is_some(), "{label}");
    }
}

/// Before the game has told them, the character's widgets say so.
#[test]
fn the_characters_widgets_say_what_is_not_known() {
    let harness = drawn(None, None);
    for label in [
        "Stance ?",
        "Encumbrance ?",
        "Encumbrance unknown",
        "Mind ?",
        "Level ?",
        "PTPs ? · MTPs ?",
        "Experience unknown",
        "Prepared: ?",
        "Society unknown",
        "Resources unknown",
        "Objectives unknown",
    ] {
        assert!(harness.query_by_label(label).is_some(), "{label}");
    }
}

/// Numbers are grouped by thousands, as the game writes them.
#[test]
fn numbers_are_grouped_as_the_game_writes_them() {
    for (n, said) in [
        (0, "0"),
        (999, "999"),
        (1_000, "1,000"),
        (68_809_898, "68,809,898"),
    ] {
        assert_eq!(character::grouped(n), said);
    }
}
