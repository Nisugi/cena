//! Arranging custom windows (`plan/49` Stage A step 4) as a player does it:
//! Arrange from the Layout menu, then drags, found as a screen reader finds
//! things.

use super::*;
use crate::layout::{Holds, Layout};
use crate::widget::Widget;

/// Press at `from`, move to `to` frame by frame, and let go.
fn drag(harness: &mut Harness<'_, Scene>, from: egui::Pos2, to: egui::Pos2) {
    harness.hover_at(from);
    harness.step();
    harness.drag_at(from);
    harness.step();
    for step in 1..=4u8 {
        harness.hover_at(from + (to - from) * (f32::from(step) / 4.0));
        harness.step();
    }
    harness.drop_at(to);
    harness.step();
    harness.step();
}

fn layout<'a>(harness: &'a Harness<'_, Scene>) -> &'a Layout {
    harness.state().play.layout.as_ref().expect("laid out")
}

/// The widgets in the window titled `title`, in its cells' order.
fn widgets_in(harness: &Harness<'_, Scene>, title: &str) -> Vec<Widget> {
    match layout(harness).titled(title).map(|holder| &holder.holds) {
        Some(Holds::Custom(custom)) => custom
            .cells
            .iter()
            .flat_map(|cell| cell.tabs.iter().map(|placed| placed.widget))
            .collect(),
        Some(Holds::One(placed)) => vec![placed.widget],
        None => Vec::new(),
    }
}

/// A harness drawn once, with Arrange on.
fn arranging<'a>() -> Harness<'a, Scene> {
    let mut harness = harness();
    harness.run();
    harness.state_mut().play.arranging = true;
    harness.run();
    harness
}

/// Arrange is in the Layout menu; on, each cell of a custom window shows
/// its widget's name, and off, none does.
#[test]
fn arrange_from_the_menu_shows_each_cell_by_name() {
    let mut harness = harness();
    harness.run();
    assert!(
        harness.query_by_label("Right hand").is_none(),
        "off at first"
    );
    harness.get_by_label("Layout").click();
    harness.run();
    harness.get_by_label("Arrange").click();
    harness.run();
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(harness.state().play.arranging);
    for name in ["Health", "Right hand", "Roundtime", "Exits"] {
        assert!(harness.query_by_label(name).is_some(), "{name}");
    }
}

/// With Arrange off, dragging where a widget is takes no widget anywhere.
#[test]
fn with_arrange_off_no_widget_leaves_its_window() {
    let mut harness = harness();
    harness.run();
    let exits = harness
        .get_by_label("Obvious exits: north, out")
        .rect()
        .center();
    drag(&mut harness, exits, egui::pos2(200.0, 300.0));
    assert!(widgets_in(&harness, "Room").contains(&Widget::Exits));
    assert!(layout(&harness).titled("Exits").is_none());
}

/// A cell dragged inside its window lands snapped: taken to the bottom, it
/// sits on the inside's bottom edge.
#[test]
fn a_cell_moves_and_snaps_inside_its_window() {
    let mut harness = arranging();
    let right = harness.get_by_label("Right hand").rect();
    drag(
        &mut harness,
        right.center(),
        right.center() + egui::vec2(0.0, 44.0),
    );
    let Some(Holds::Custom(loadout)) = layout(&harness)
        .titled("Loadout")
        .map(|holder| &holder.holds)
    else {
        panic!("a custom window");
    };
    let cell = loadout
        .cells
        .iter()
        .find(|cell| {
            cell.shown()
                .is_some_and(|shown| shown.widget == Widget::RightHand)
        })
        .map(crate::layout::Cell::rect)
        .expect("still there");
    assert!(
        (cell.max.y - loadout.inside().y).abs() < 0.01,
        "on the bottom edge: {cell:?} in {:?}",
        loadout.inside()
    );
    assert!(
        (cell.height() - crate::widget::LINE).abs() < 0.01,
        "a move keeps its size"
    );
}

/// A cell moved partway past its window's edge, the pointer still inside,
/// is slid back in whole.
#[test]
fn a_cell_moved_partway_out_is_slid_back_in() {
    let mut harness = arranging();
    let right = harness.get_by_label("Right hand").rect();
    drag(
        &mut harness,
        right.center(),
        right.center() + egui::vec2(40.0, 0.0),
    );
    let Some(Holds::Custom(loadout)) = layout(&harness)
        .titled("Loadout")
        .map(|holder| &holder.holds)
    else {
        panic!("a custom window");
    };
    let cell = loadout
        .cells
        .iter()
        .find(|cell| {
            cell.shown()
                .is_some_and(|shown| shown.widget == Widget::RightHand)
        })
        .map(crate::layout::Cell::rect)
        .expect("still there");
    assert!(cell.max.x <= loadout.inside().x + 0.01, "{cell:?}");
    assert!(
        cell.min.x.abs() < 0.01,
        "as wide as the inside, so back at its left: {cell:?}"
    );
}

/// A cell's edge dragged moves that edge only.
#[test]
fn a_cells_edge_resizes_it() {
    let mut harness = arranging();
    let clock = harness.get_by_label("Roundtime").rect();
    let before = clock;
    drag(
        &mut harness,
        egui::pos2(clock.max.x - 2.0, clock.center().y),
        egui::pos2(clock.max.x - 42.0, clock.center().y),
    );
    let after = harness.get_by_label("Roundtime").rect();
    assert!(
        after.width() < before.width() - 20.0,
        "{before:?} -> {after:?}"
    );
    assert!(
        (after.min.x - before.min.x).abs() < 0.5,
        "the left edge stayed"
    );
}

/// A widget dragged out of its custom window into the open becomes a
/// standalone window there; the rest of its window stays.
#[test]
fn a_widget_dragged_out_gets_a_window_of_its_own() {
    let mut harness = arranging();
    let exits = harness.get_by_label("Exits").rect().center();
    drag(&mut harness, exits, egui::pos2(200.0, 300.0));
    assert_eq!(widgets_in(&harness, "Exits"), [Widget::Exits]);
    assert!(!widgets_in(&harness, "Room").contains(&Widget::Exits));
    assert!(widgets_in(&harness, "Room").contains(&Widget::Creatures));
}

/// With Arrange on, a standalone window dropped on a custom window joins
/// it; with it off, the same drop leaves it a window of its own.
#[test]
fn a_window_dropped_on_a_custom_window_joins_it_only_when_arranging() {
    for arrange in [false, true] {
        let mut harness = harness();
        harness.run();
        harness.state_mut().play.arranging = arrange;
        harness.run();
        let hunt = harness.get_by_label("Hunt").rect();
        let grip = egui::pos2(hunt.center().x, hunt.min.y + 12.0);
        let room = harness.get_by_label("a kobold").rect().center();
        drag(&mut harness, grip, room);
        let joined = widgets_in(&harness, "Room").contains(&Widget::Hunt);
        assert_eq!(joined, arrange, "arranging: {arrange}");
        assert_eq!(layout(&harness).titled("Hunt").is_some(), !arrange);
    }
}

/// A new custom window comes empty, saying how to fill it, with Arrange on.
#[test]
fn a_new_custom_window_comes_empty_and_arranging() {
    let mut harness = harness();
    harness.run();
    harness.get_by_label("Layout").click();
    harness.run();
    harness.get_by_label("New custom window").click();
    harness.run();
    harness.run();
    assert!(harness.state().play.arranging);
    assert!(widgets_in(&harness, "Custom window").is_empty());
    assert!(layout(&harness).titled("Custom window").is_some());
    assert!(
        harness
            .query_by_label_contains("drop widgets here")
            .is_some()
    );
}

/// Resized over a custom window, a standalone window stays its own: only a
/// window carried there joins. Hunt is put in open space first, level with
/// the room, so its right edge is its own to drag.
#[test]
fn a_window_resized_over_a_custom_window_stays_its_own() {
    let mut harness = arranging();
    let room = harness.get_by_label("Creatures").rect().center();
    let hunt = layout(&harness)
        .titled("Hunt")
        .map(|holder| holder.id)
        .expect("a hunt");
    let area_top = harness.get_by_label("Story").rect().min.y;
    if let Some(layout) = harness.state_mut().play.layout.as_mut() {
        let at = egui::pos2(100.0, room.y - area_top - 60.0);
        layout.set(
            hunt,
            egui::Rect::from_min_size(at, egui::vec2(260.0, 110.0)),
        );
    }
    // egui takes a new size a frame after it is given.
    harness.run();
    harness.step();
    harness.step();
    let window = harness.get_by_label("Hunt").rect();
    let before = kept(&harness, "Hunt").expect("laid out");
    let edge = egui::pos2(window.max.x - 2.0, room.y);
    drag(&mut harness, edge, egui::pos2(room.x, room.y));
    let after = kept(&harness, "Hunt").expect("still its own");
    assert!(
        after.width() > before.width() + 100.0,
        "{before:?} -> {after:?}"
    );
    assert_eq!(widgets_in(&harness, "Hunt"), [Widget::Hunt]);
    assert!(!widgets_in(&harness, "Room").contains(&Widget::Hunt));
}

/// A cell's edge dragged past its window stops at the window's edge, and
/// the cell stays in its window: only a moved widget leaves.
#[test]
fn a_cell_resized_past_its_window_stays_in_it() {
    let mut harness = arranging();
    let clock = harness.get_by_label("Roundtime").rect();
    drag(
        &mut harness,
        egui::pos2(clock.center().x, clock.max.y - 2.0),
        egui::pos2(clock.center().x, clock.max.y + 200.0),
    );
    assert!(widgets_in(&harness, "Loadout").contains(&Widget::Roundtime));
    let Some(Holds::Custom(loadout)) = layout(&harness)
        .titled("Loadout")
        .map(|holder| &holder.holds)
    else {
        panic!("a custom window");
    };
    for cell in &loadout.cells {
        assert!(
            cell.rect().max.y <= loadout.inside().y + 0.01,
            "{:?}",
            cell.rect()
        );
    }
}

/// What arranging did is kept under the character's name.
#[test]
fn an_arranged_layout_is_kept_by_name() {
    let dir = std::env::temp_dir().join(format!("cena-arrange-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut scene = Scene::new();
    scene.play = Play::new(0, "Ashryn", Some(dir.clone()));
    let mut harness = Harness::builder()
        .with_size((1000.0, 700.0))
        .build_ui_state(|ui, scene: &mut Scene| scene.draw(ui), scene);
    harness.run();
    harness.state_mut().play.arranging = true;
    harness.run();
    let exits = harness.get_by_label("Exits").rect().center();
    drag(&mut harness, exits, egui::pos2(200.0, 300.0));
    let reopened = Play::new(0, "Ashryn", Some(dir.clone()));
    let kept = reopened
        .layout
        .as_ref()
        .and_then(|layout| layout.titled("Exits"));
    assert!(matches!(
        kept.map(|holder| &holder.holds),
        Some(Holds::One(_))
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

/// While a widget is dragged out of its window, its name follows the
/// pointer, where it would land.
#[test]
fn a_widget_dragged_out_shows_where_it_goes() {
    let mut scene = Scene::new();
    scene.snapshot.state.roundtime_ends = None;
    let mut harness = Harness::builder()
        .with_size((1000.0, 700.0))
        .wgpu()
        .build_ui_state(|ui, scene: &mut Scene| scene.draw(ui), scene);
    harness.run();
    harness.state_mut().play.arranging = true;
    harness.run();
    let exits = harness.get_by_label("Exits").rect().center();
    let to = egui::pos2(300.0, 300.0);
    harness.hover_at(exits);
    harness.step();
    harness.drag_at(exits);
    harness.step();
    for step in 1..=4u8 {
        harness.hover_at(exits + (to - exits) * (f32::from(step) / 4.0));
        harness.step();
    }
    harness.snapshot("arrange");
    harness.drop_at(to);
    harness.step();
}
