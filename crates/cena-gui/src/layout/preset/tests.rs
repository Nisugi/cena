//! Presets (`plan/49` Stage A step 7), Hydra's and a player's library.

use std::collections::HashSet;

use egui::Vec2;

use super::*;
use crate::layout::{Custom, Holds, Layout, Placed};
use crate::widget::Widget;

/// The widgets of a custom window, in its cells' order.
fn in_custom(custom: &Custom) -> Vec<Widget> {
    custom
        .cells
        .iter()
        .flat_map(|cell| cell.tabs.iter().map(|tab| tab.widget.clone()))
        .collect()
}

/// Hydra's presets: each named once, the vitals row four bars side by side.
#[test]
fn hydras_presets_are_put_together() {
    let presets = Preset::hydras();
    let names: HashSet<&str> = presets.iter().map(|preset| preset.name.as_str()).collect();
    assert_eq!(names.len(), presets.len());
    let mut layout = Layout::fitted(Vec2::new(900.0, 600.0));
    let row = presets
        .iter()
        .find(|preset| preset.name == "Vitals row")
        .expect("a vitals row");
    let placed = layout.add_preset(row, None);
    let Some(Holds::Custom(custom)) = layout.holder(placed).map(|holder| &holder.holds) else {
        panic!("a custom window");
    };
    assert_eq!(
        in_custom(custom),
        [
            Widget::Health,
            Widget::Mana,
            Widget::Stamina,
            Widget::Spirit
        ]
    );
    let tops: HashSet<u32> = custom
        .cells
        .iter()
        .map(|cell| cell.rect().min.y.to_bits())
        .collect();
    assert_eq!(tops.len(), 1, "one row");
}

/// Placing a preset places a copy: its widgets get ids of their own, each
/// copy apart, and saving over the preset later changes no copy.
#[test]
fn a_preset_placed_is_a_copy() {
    let mut layout = Layout::fitted(Vec2::new(900.0, 600.0));
    let vitals = Preset::hydras().remove(0);
    let first = layout.add_preset(&vitals, None);
    let second = layout.add_preset(&vitals, None);
    assert_ne!(first, second);
    let mut ids = HashSet::new();
    for holder in &layout.holders {
        let placed: Vec<u32> = match &holder.holds {
            Holds::One(placed) => vec![placed.id],
            Holds::Custom(custom) => custom
                .cells
                .iter()
                .flat_map(|cell| cell.tabs.iter().map(|tab| tab.id))
                .collect(),
        };
        for id in placed {
            assert!(ids.insert(id), "widget {id} twice");
        }
        assert!(ids.insert(holder.id), "window {}", holder.id);
    }
    let mut library = Library::load(None);
    library.keep(vitals.clone());
    let Some(Holds::Custom(custom)) = layout.holder(first).map(|holder| &holder.holds) else {
        panic!("a custom window");
    };
    library.keep(Preset::of(
        "Vitals",
        &Custom::empty("Vitals", Vec2::new(10.0, 10.0)),
    ));
    let Some(Holds::Custom(after)) = layout.holder(first).map(|holder| &holder.holds) else {
        panic!("still a custom window");
    };
    assert_eq!(after, custom, "the copy placed is untouched");
    assert_eq!(library.presets().len(), 1, "kept over the same name");
}

/// A preset placed for another character: each widget follows that one,
/// but a story, which stays its own window's.
#[test]
fn a_preset_placed_for_another_follows_but_its_story() {
    let mut layout = Layout::fitted(Vec2::new(900.0, 600.0));
    let placed = |id, widget| Placed { id, widget };
    let custom = Custom::rows(
        "Mixed",
        vec![
            vec![placed(0, Widget::Story)],
            vec![placed(0, Widget::Health)],
        ],
        Vec2::new(200.0, 200.0),
    );
    let window = layout.add_preset(&Preset::of("Mixed", &custom), Some("Baelor"));
    let Some(Holds::Custom(custom)) = layout.holder(window).map(|holder| &holder.holds) else {
        panic!("a custom window");
    };
    for tab in custom.cells.iter().flat_map(|cell| cell.tabs.iter()) {
        let follows = layout.follows.get(&tab.id).map(String::as_str);
        match tab.widget {
            Widget::Story => assert_eq!(follows, None),
            _ => assert_eq!(follows, Some("Baelor")),
        }
    }
}

/// The library is kept in its file: what is kept is there when it is read
/// again, one of the same name kept over, a forgotten one gone; no file, or
/// one of another version, is an empty library.
#[test]
fn the_library_is_kept_in_its_file() {
    let dir = std::env::temp_dir().join(format!("cena-presets-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        Library::load(Some(dir.clone())).presets().is_empty(),
        "no file"
    );
    let mut library = Library::load(Some(dir.clone()));
    let mut hydras = Preset::hydras().into_iter();
    let (Some(vitals), Some(row)) = (hydras.next(), hydras.next()) else {
        panic!("Hydra's presets");
    };
    library.keep(vitals);
    library.keep(row);
    assert_eq!(
        Library::load(Some(dir.clone())).presets(),
        library.presets()
    );
    library.forget("Vitals");
    let read = Library::load(Some(dir.clone()));
    let names: Vec<&str> = read
        .presets()
        .iter()
        .map(|preset| preset.name.as_str())
        .collect();
    assert_eq!(names, ["Vitals row"]);
    let file = dir.join("presets.json");
    let text = std::fs::read_to_string(&file).expect("written");
    std::fs::write(&file, text.replace("\"version\": 1", "\"version\": 2")).expect("written");
    assert!(
        Library::load(Some(dir.clone())).presets().is_empty(),
        "another version"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Hydra's Indicators preset holds all nineteen, three a row; its Effects
/// preset is one tab stack of the four lists, Active Spells showing.
#[test]
fn the_indicators_and_effects_presets_are_whole() {
    let presets = Preset::hydras();
    let found = |name: &str| {
        presets
            .iter()
            .find(|preset| preset.name == name)
            .cloned()
            .expect(name)
    };
    let mut layout = Layout::fitted(Vec2::new(900.0, 600.0));
    let indicators = layout.add_preset(&found("Indicators"), None);
    let Some(Holds::Custom(custom)) = layout.holder(indicators).map(|holder| &holder.holds) else {
        panic!("a custom window");
    };
    assert_eq!(custom.cells.len(), 19);
    let effects = layout.add_preset(&found("Effects"), None);
    let Some(Holds::Custom(custom)) = layout.holder(effects).map(|holder| &holder.holds) else {
        panic!("a custom window");
    };
    assert_eq!(custom.cells.len(), 1, "one tab stack");
    assert_eq!(custom.cells[0].tabs.len(), 4);
    assert_eq!(
        custom.cells[0].shown().map(|shown| shown.widget.clone()),
        Some(Widget::Effects(crate::widget::Category::ActiveSpells))
    );
}

/// Hydra's Streams preset is one tab stack of the streams most read.
#[test]
fn the_streams_preset_stacks_the_streams_most_read() {
    let streams = Preset::hydras()
        .into_iter()
        .find(|preset| preset.name == "Streams")
        .expect("a streams preset");
    let mut layout = Layout::fitted(Vec2::new(900.0, 600.0));
    let window = layout.add_preset(&streams, None);
    let Some(Holds::Custom(custom)) = layout.holder(window).map(|holder| &holder.holds) else {
        panic!("a custom window");
    };
    assert_eq!(custom.cells.len(), 1);
    let names: Vec<String> = custom.cells[0]
        .tabs
        .iter()
        .map(|tab| tab.widget.name().into_owned())
        .collect();
    assert_eq!(
        names,
        ["Thoughts", "Speech", "Arrivals", "Deaths", "Announcements"]
    );
    assert_eq!(layout.streams().len(), 5, "each an open stream");
}
