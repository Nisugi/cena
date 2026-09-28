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
    assert!(
        harness.query_by_label("PRESETS").is_none(),
        "no preset is called so"
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
    // The first layout shows sixteen kinds: all but the room's description
    // of the seventeen it was made from.
    assert_eq!(harness.query_all_by_label("shown").count(), 16);
    search(&mut harness, "description");
    assert_eq!(harness.query_all_by_label("shown").count(), 0);
    harness
        .get_by_role_and_label(Role::Button, "Room description")
        .click();
    harness.run();
    assert_eq!(harness.query_all_by_label("shown").count(), 1);
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
        .flat_map(|cell| cell.tabs.iter().map(|tab| tab.widget.clone()))
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

/// The list's presets come first: a click places a copy, and with Advanced
/// showing another character, its widgets follow that one.
#[test]
fn the_list_places_a_preset() {
    let mut harness = with_baelor();
    open_the_list(&mut harness);
    assert!(harness.query_by_label("PRESETS").is_some());
    harness
        .get_by_role_and_label(Role::Button, "Vitals row")
        .click();
    harness.run();
    assert!(matches!(
        holds(&harness, "Vitals row"),
        Some(Holds::Custom(custom)) if custom.cells.len() == 4
    ));
    search(&mut harness, "loadout");
    harness.get_by_label("Advanced").click();
    harness.run();
    harness.get_by_label("Baelor").click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Loadout")
        .click();
    harness.run();
    assert!(
        harness
            .query_by_label("Baelor Left: a steel broadsword")
            .is_some()
    );
}

/// A custom window saved as a preset from its menu asks the app to keep it,
/// under the name typed; a preset saved before can be forgotten from the
/// list.
#[test]
fn a_custom_window_is_saved_as_a_preset_and_forgotten() {
    let mut harness = with_baelor();
    harness
        .get_by_label("Left: a steel broadsword")
        .click_secondary();
    harness.run();
    harness.get_by_label("Save as preset...").click();
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.run();
    harness.key_press(egui::Key::Backspace);
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert!(harness.state().asked.is_empty(), "a preset needs a name");
    typed(&mut harness, "Kit");
    harness.key_press(egui::Key::Enter);
    harness.run();
    let saved = harness.state().asked.iter().find_map(|asked| match asked {
        Asked::SavePreset(preset) => Some(preset.clone()),
        _ => None,
    });
    let saved = saved.expect("asked to keep it");
    assert_eq!(saved.name, "Kit");
    harness.state_mut().presets = {
        let mut library = crate::layout::Library::default();
        library.keep(saved);
        library
    };
    open_the_list(&mut harness);
    harness
        .get_by_role_and_label(Role::Button, "Forget")
        .click();
    harness.run();
    assert!(
        harness
            .state()
            .asked
            .contains(&Asked::ForgetPreset("Kit".to_owned()))
    );
}

/// A library whose file cannot be written says so in the list.
#[test]
fn presets_not_saved_say_so() {
    let mut harness = with_baelor();
    let blocked = std::env::temp_dir().join(format!("cena-presets-blocked-{}", std::process::id()));
    std::fs::write(&blocked, "a file, not a folder").expect("written");
    let mut library = crate::layout::Library::load(Some(blocked.clone()));
    library.keep(crate::layout::Preset::hydras().remove(0));
    harness.state_mut().presets = library;
    open_the_list(&mut harness);
    assert!(
        harness
            .query_by_label_contains("Presets not saved")
            .is_some()
    );
    let _ = std::fs::remove_file(&blocked);
}

/// A compass added to a play window moves its character when clicked, as
/// if the direction were typed.
#[test]
fn a_compass_moves_its_character() {
    let mut harness = with_baelor();
    if let Some(layout) = harness.state_mut().play.layout.as_mut() {
        layout.add_widget(Widget::Compass, None);
    }
    harness.run();
    harness.get_by_role_and_label(Role::Button, "north").click();
    harness.run();
    assert!(
        harness
            .state()
            .asked
            .contains(&Asked::Send("north".to_owned()))
    );
}

/// A line on `stream`, as the feed hands the story one.
fn heard(stream: &str, text: &str) -> cena_session::ObservedEvent {
    cena_session::ObservedEvent {
        session: cena_session::SessionId::FIRST,
        generation: cena_session::Generation::FIRST,
        cursor: 9,
        event: cena_session::Event::Line(Arc::new(cena_session::Line::new(
            stream,
            cena_session::ChunkLine::plain(text).runs,
        ))),
    }
}

/// A thought reaches the story while no widget shows thoughts; with a
/// Thoughts widget open, it is there, once, and not in the story.
#[test]
fn a_streams_widget_takes_its_lines_from_the_story() {
    let mut harness = with_baelor();
    harness
        .state_mut()
        .story
        .hear(&heard("thoughts", "[General] anyone hunting?"), None);
    harness.run();
    assert_eq!(
        harness
            .query_all_by_label("[General] anyone hunting?")
            .count(),
        1,
        "in the story"
    );
    if let Some(layout) = harness.state_mut().play.layout.as_mut() {
        layout.add_widget(Widget::Stream("thoughts".to_owned()), None);
    }
    harness.run();
    assert!(holds(&harness, "Thoughts").is_some());
    assert_eq!(
        harness
            .query_all_by_label("[General] anyone hunting?")
            .count(),
        1,
        "in the Thoughts window alone"
    );
}

/// The list offers every stream the character has received, known by name
/// or not: one that comes with a position the game gives, too.
#[test]
fn the_list_offers_a_stream_the_character_received() {
    let mut harness = with_baelor();
    harness
        .state_mut()
        .story
        .hear(&heard("mentor", "[Mentor] welcome"), None);
    harness.run();
    open_the_list(&mut harness);
    search(&mut harness, "mentor");
    harness
        .get_by_role_and_label(Role::Button, "Mentor")
        .click();
    harness.run();
    assert!(matches!(
        holds(&harness, "Mentor"),
        Some(Holds::One(placed)) if placed.widget == Widget::Stream("mentor".to_owned())
    ));
    assert!(harness.query_by_label("[Mentor] welcome").is_some());
}

/// A stream's menu has no Advanced entry: a stream is its character's
/// story, and never follows another.
#[test]
fn a_streams_menu_offers_no_following() {
    let mut harness = with_baelor();
    if let Some(layout) = harness.state_mut().play.layout.as_mut() {
        layout.add_widget(Widget::Stream("thoughts".to_owned()), None);
    }
    harness.run();
    harness.get_by_label("Nothing yet.").click_secondary();
    harness.run();
    assert!(harness.query_by_label("Remove").is_some(), "a menu");
    assert!(harness.query_by_label("Advanced").is_none());
}
