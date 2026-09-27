//! The Add-a-widget list and the right-click menu (`plan/49` Stage A step
//! 6), as a player uses them.

use std::sync::Arc;

use super::*;
use crate::layout::Holds;
use crate::widget::{Character, Widget};
use egui::accesskit::Role;

/// Ashryn's play window with Baelor running too.
fn with_baelor<'a>() -> Harness<'a, Scene> {
    let mut scene = Scene::new();
    scene.others = vec![Character {
        name: "Baelor".to_owned(),
        snapshot: Some(Arc::new(crate::fixture::snapshot())),
        hunt: None,
    }];
    let mut harness = Harness::builder()
        .with_size((1000.0, 700.0))
        .build_ui_state(|ui, scene: &mut Scene| scene.draw(ui), scene);
    harness.run();
    harness
}

fn open_the_list(harness: &mut Harness<'_, Scene>) {
    harness.get_by_label("Layout").click();
    harness.run();
    harness.get_by_label("Add a widget...").click();
    harness.run();
}

/// Type `text` into the list's search, as a player does: a click into it,
/// then the keys.
fn search(harness: &mut Harness<'_, Scene>, text: &str) {
    harness
        .query_all_by_role(Role::TextInput)
        .find(|input| !input.is_focused())
        .expect("the search")
        .click();
    harness.run();
    typed(harness, text);
}

/// Type `text` into the input that has the keyboard.
fn typed(harness: &mut Harness<'_, Scene>, text: &str) {
    harness
        .query_all_by_role(Role::TextInput)
        .find(egui_kittest::Node::is_focused)
        .expect("an input with the keyboard")
        .type_text(text);
    harness.run();
}

fn holds<'a>(harness: &'a Harness<'_, Scene>, title: &str) -> Option<&'a Holds> {
    harness
        .state()
        .play
        .layout
        .as_ref()
        .and_then(|layout| layout.titled(title))
        .map(|holder| &holder.holds)
}

/// The list finds a widget by what is typed, and a click adds it in a
/// window of its own.
#[test]
fn the_list_adds_a_widget_found_by_typing() {
    let mut harness = with_baelor();
    open_the_list(&mut harness);
    search(&mut harness, "cast");
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Health")
            .is_none()
    );
    harness
        .get_by_role_and_label(Role::Button, "Cast time")
        .click();
    harness.run();
    assert!(matches!(
        holds(&harness, "Cast time"),
        Some(Holds::One(placed)) if placed.widget == Widget::CastTime
    ));
}

/// The list's groups are §3's, and each kind already shown says so.
#[test]
fn the_list_says_what_is_shown() {
    let mut harness = with_baelor();
    open_the_list(&mut harness);
    for heading in ["STREAMS", "GRAPHICS", "HYDRA'S OWN"] {
        assert!(harness.query_by_label(heading).is_some(), "{heading}");
    }
    // The first layout shows every kind but the room's description.
    assert_eq!(
        harness.query_all_by_label("shown").count(),
        Widget::ALL.len() - 1
    );
    harness
        .get_by_role_and_label(Role::Button, "Room description")
        .click();
    harness.run();
    assert_eq!(
        harness.query_all_by_label("shown").count(),
        Widget::ALL.len()
    );
}

/// The Advanced place is closed until opened; there, a widget added follows
/// another character, named on it, and a story cannot be. The list is
/// narrowed first, as a player would, so its bottom is in view.
#[test]
fn advanced_adds_a_widget_for_another_character() {
    let mut harness = with_baelor();
    open_the_list(&mut harness);
    search(&mut harness, "st");
    assert!(harness.query_by_label("Show for").is_none(), "closed");
    harness.get_by_label("Advanced").click();
    harness.run();
    harness.get_by_label("Baelor").click();
    harness.run();
    let windows = |harness: &Harness<'_, Scene>| {
        harness
            .state()
            .play
            .layout
            .as_ref()
            .map_or(0, |layout| layout.holders.len())
    };
    let before = windows(&harness);
    harness.get_by_role_and_label(Role::Button, "Story").click();
    harness.run();
    assert_eq!(
        windows(&harness),
        before,
        "a story stays its own character's"
    );
    harness
        .get_by_role_and_label(Role::Button, "Stamina")
        .click();
    harness.run();
    assert!(harness.query_by_label("Stamina (Baelor)").is_some());
    assert!(harness.query_by_label("Baelor SP ?").is_some());
}

/// A right-click on a standalone window removes it.
#[test]
fn a_right_click_removes_a_window_of_one() {
    let mut harness = with_baelor();
    harness.get_by_label("Hunt").click_secondary();
    harness.run();
    harness.get_by_label("Remove").click();
    harness.run();
    assert!(holds(&harness, "Hunt").is_none());
}

/// A right-click on a widget in a custom window removes that widget only.
#[test]
fn a_right_click_removes_one_widget_of_a_custom_window() {
    let mut harness = with_baelor();
    harness
        .get_by_label("Obvious exits: north, out")
        .click_secondary();
    harness.run();
    harness.get_by_label("Remove").click();
    harness.run();
    let Some(Holds::Custom(room)) = holds(&harness, "Room") else {
        panic!("still a custom window");
    };
    let widgets: Vec<Widget> = room
        .cells
        .iter()
        .flat_map(|cell| cell.tabs.iter().map(|tab| tab.widget))
        .collect();
    assert_eq!(widgets.len(), 4);
    assert!(!widgets.contains(&Widget::Exits));
}

/// A custom window is renamed and removed from its menu.
#[test]
fn a_custom_window_is_renamed_and_removed() {
    let mut harness = with_baelor();
    harness
        .get_by_label("Left: a steel broadsword")
        .click_secondary();
    harness.run();
    harness.get_by_label("Rename...").click();
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.run();
    typed(&mut harness, "Hands");
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert!(holds(&harness, "Hands").is_some(), "renamed");
    harness
        .get_by_label("Left: a steel broadsword")
        .click_secondary();
    harness.run();
    harness.get_by_label("Remove window").click();
    harness.run();
    assert!(holds(&harness, "Hands").is_none());
    assert!(harness.query_by_label("Left: a steel broadsword").is_none());
}

/// The menu's Advanced entry, closed, makes a widget follow another
/// character; a story's menu has none.
#[test]
fn the_menus_advanced_entry_follows_another_character() {
    let mut harness = with_baelor();
    harness
        .get_by_label("You swing a steel broadsword at a kobold!")
        .click_secondary();
    harness.run();
    assert!(harness.query_by_label("Remove").is_some(), "a menu");
    assert!(
        harness.query_by_label("Advanced").is_none(),
        "none for a story"
    );
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(
        harness.query_by_label("Remove").is_none(),
        "Escape closes it"
    );
    harness.get_by_label("HP 348/400 87%").click_secondary();
    harness.run();
    harness.get_by_label("Advanced").click();
    harness.run();
    harness.get_by_label("Baelor").click();
    harness.run();
    assert!(harness.query_by_label("Baelor HP 348/400 87%").is_some());
}

/// A widget following a character who is not running says so.
#[test]
fn a_widget_following_someone_not_running_says_so() {
    let mut harness = with_baelor();
    let health = {
        let layout = harness.state().play.layout.as_ref().expect("laid out");
        let Some(Holds::Custom(vitals)) = layout.titled("Vitals").map(|holder| &holder.holds)
        else {
            panic!("a custom window");
        };
        vitals.cells[0].tabs[0].id
    };
    if let Some(layout) = harness.state_mut().play.layout.as_mut() {
        layout.follow(health, Some("Lorwyn".to_owned()));
    }
    harness.run();
    assert!(harness.query_by_label("Lorwyn is not running.").is_some());
}

/// A click outside an open menu closes it, doing nothing.
#[test]
fn a_click_outside_closes_the_menu() {
    let mut harness = with_baelor();
    harness.get_by_label("Hunt").click_secondary();
    harness.run();
    assert!(harness.query_by_label("Remove").is_some());
    harness
        .get_by_label("You swing a steel broadsword at a kobold!")
        .click();
    harness.run();
    assert!(harness.query_by_label("Remove").is_none());
    assert!(holds(&harness, "Hunt").is_some());
}

/// A story never follows another character, whatever a layout file says:
/// it shows its own window's lines, even named after one not running.
#[test]
fn a_story_stays_its_own_whatever_the_file_says() {
    let mut harness = with_baelor();
    let story = match holds(&harness, "Story") {
        Some(Holds::One(placed)) => placed.id,
        _ => panic!("a story window"),
    };
    if let Some(layout) = harness.state_mut().play.layout.as_mut() {
        layout.follow(story, Some("Lorwyn".to_owned()));
    }
    harness.run();
    assert!(
        harness.query_by_label("Story").is_some(),
        "not Story (Baelor)"
    );
    assert!(
        harness
            .query_by_label("You swing a steel broadsword at a kobold!")
            .is_some()
    );
}

/// A widget of several lines following another character says whose, on a
/// line of its own above what it shows.
#[test]
fn a_widget_of_lines_names_whose_it_is() {
    let mut harness = with_baelor();
    let hunt = match holds(&harness, "Hunt") {
        Some(Holds::One(placed)) => placed.id,
        _ => panic!("a hunt window"),
    };
    if let Some(layout) = harness.state_mut().play.layout.as_mut() {
        layout.follow(hunt, Some("Baelor".to_owned()));
    }
    harness.run();
    assert!(harness.query_by_label("Hunt (Baelor)").is_some());
    assert!(harness.query_by_label("Baelor").is_some());
    assert!(
        harness.query_by_label("No hunt running.").is_some(),
        "Baelor's"
    );
}
