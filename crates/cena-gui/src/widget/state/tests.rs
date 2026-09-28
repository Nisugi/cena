use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::{Line, found, game_state, tree};
use crate::fixture::snapshot;

/// A print as `{:#?}` writes one, cut down to its shapes.
const PRINTED: &str = "GameState {
    room: Room {
        id: Some(
            \"8213304\",
        ),
        exits: [],
        players: [
            RoomItem {
                noun: \"Aldric\",
                id: \"-1\",
            },
        ],
    },
    roundtime_ends: None,
}";

fn leaf(text: &str) -> Line {
    Line::Leaf(text.to_owned())
}

/// The print read as a tree: a struct or a list opens a branch, a value is a
/// line of its own, and a branch holding one value is that value on one
/// line. A print cut short still reads.
#[test]
fn the_print_reads_as_a_tree() {
    let players = Line::Branch(
        "players:".to_owned(),
        vec![Line::Branch(
            "RoomItem".to_owned(),
            vec![leaf("noun: \"Aldric\","), leaf("id: \"-1\",")],
        )],
    );
    assert_eq!(
        tree(PRINTED),
        [Line::Branch(
            "GameState".to_owned(),
            vec![
                Line::Branch(
                    "room: Room".to_owned(),
                    vec![leaf("id: Some(\"8213304\"),"), leaf("exits: [],"), players],
                ),
                leaf("roundtime_ends: None,"),
            ],
        )]
    );
    assert_eq!(
        tree("GameState {\n    hands: Hands {\n        left: 1,\n        right: 2,"),
        [Line::Branch(
            "GameState".to_owned(),
            vec![Line::Branch(
                "hands: Hands".to_owned(),
                vec![leaf("left: 1,"), leaf("right: 2,")]
            )]
        )]
    );
}

/// The filter finds a line whatever its case, and says where it is.
#[test]
fn the_filter_says_where_each_line_is() {
    assert_eq!(
        found(&tree(PRINTED), "ALDRIC"),
        ["GameState › room: Room › players: › RoomItem › noun: \"Aldric\","]
    );
    assert_eq!(
        found(&tree(PRINTED), "room: room"),
        ["GameState › room: Room"],
        "a branch's own line is found too"
    );
    assert!(found(&tree(PRINTED), "nowhere").is_empty());
}

/// The widget shows the model's own fields, each closed until opened; the
/// filter finds what is inside; nothing seen says so; and Copy takes the
/// whole print.
#[test]
fn the_widget_shows_everything_the_model_holds() {
    let seen = snapshot();
    let mut harness = Harness::builder()
        .with_size((520.0, 640.0))
        .build_ui(move |ui| game_state(ui, Some(&seen.state), None, egui::Id::new("state")));
    harness.run();
    assert!(
        harness.query_by_label("room: Room").is_some(),
        "a top field"
    );
    assert!(
        harness
            .query_by_label_contains("steel broadsword")
            .is_none(),
        "closed until opened"
    );
    match harness.query_all_by_label_contains("left_hand").next() {
        Some(hands) => hands.click(),
        None => panic!("the left hand's field"),
    }
    harness.run();
    assert!(
        harness
            .query_by_label_contains("steel broadsword")
            .is_some()
    );

    harness
        .get_by_role(egui::accesskit::Role::TextInput)
        .focus();
    harness.run();
    harness
        .get_by_role(egui::accesskit::Role::TextInput)
        .type_text("BROADSWORD");
    harness.run();
    assert!(
        harness
            .query_by_label_contains("left_hand: Holding › name")
            .is_some(),
        "found, with where it is"
    );

    harness.get_by_label("Copy").click();
    harness.step();
    let copied = harness
        .output()
        .platform_output
        .commands
        .iter()
        .any(|command| matches!(command, egui::OutputCommand::CopyText(text) if text.starts_with("GameState {")));
    assert!(copied, "the whole print");

    let nothing = Harness::new_ui(|ui| game_state(ui, None, None, egui::Id::new("state")));
    assert!(nothing.query_by_label("Nothing seen yet.").is_some());
}
