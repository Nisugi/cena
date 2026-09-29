//! The drawers (`plan/49` Stage E) as a player meets them: a push drawer
//! moving the main area's windows over, a window dragged into a drawer and
//! kept there, a drawer's edge dragged to size it, and a press on a
//! drawer's bare backdrop stopped while it hides what is under it and let
//! through once it hides nothing (`plan/28` §7d.5).

use super::links::{click_at, heard, rat};
use super::*;
use crate::layout::{Mode, Zone};

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

/// The left drawer opened `size` across, pushing or clipping, as opaque as
/// `opacity`.
fn open_left(harness: &mut Harness<'_, Scene>, size: f32, mode: Mode, opacity: f32) {
    if let Some(layout) = harness.state_mut().play.layout.as_mut() {
        layout.drawers.left.open = true;
        layout.drawers.left.size = size;
        layout.drawers.left.mode = mode;
        layout.drawers.left.opacity = opacity;
    }
    harness.run();
}

/// Where the play area's top left is on screen: the Story window as drawn,
/// less where its zone puts it.
fn corner(harness: &Harness<'_, Scene>) -> egui::Vec2 {
    let play = &harness.state().play;
    let story = layout(harness).titled("Story").map(|holder| holder.id);
    let shown = story
        .and_then(|story| layout(harness).shown(story, &play.zones))
        .unwrap_or(egui::Rect::NOTHING);
    harness.get_by_label("Story").rect().min - shown.min
}

/// Opened to push, the left drawer moves the main area's windows over by
/// its width, and they are kept where they were: shut again, they are back.
#[test]
fn a_push_drawer_moves_the_main_area_over() {
    let mut harness = harness();
    harness.run();
    let before = harness.get_by_label("Story").rect();
    let kept_before = kept(&harness, "Story");
    open_left(&mut harness, 200.0, Mode::Push, 1.0);
    let after = harness.get_by_label("Story").rect();
    assert!(
        (after.min.x - before.min.x - 200.0).abs() < 0.5,
        "moved over: {before:?} -> {after:?}"
    );
    assert!(
        after.width() < before.width(),
        "and drawn narrower, inside what is left"
    );
    assert_eq!(kept(&harness, "Story"), kept_before, "kept as it was");
    if let Some(layout) = harness.state_mut().play.layout.as_mut() {
        layout.drawers.left.open = false;
    }
    harness.run();
    assert_eq!(
        harness.get_by_label("Story").rect(),
        before,
        "back when shut"
    );
}

/// A clip drawer lies over the main area, which does not move.
#[test]
fn a_clip_drawer_leaves_the_main_area_where_it_is() {
    let mut harness = harness();
    harness.run();
    let before = harness.get_by_label("Story").rect();
    open_left(&mut harness, 200.0, Mode::Clip, 1.0);
    assert_eq!(harness.get_by_label("Story").rect(), before);
}

/// A window dragged by its title into an open drawer lives there: made to
/// fit it, kept from its corner, gone while the drawer is shut and back
/// when it opens, and all of it saved with the layout.
#[test]
fn a_window_dragged_into_a_drawer_lives_there() {
    let dir = std::env::temp_dir().join(format!("cena-play-drawers-{}", std::process::id()));
    let mut harness = saving(&dir);
    open_left(&mut harness, 300.0, Mode::Clip, 1.0);
    let window = harness.get_by_label("Hunt").rect();
    let grip = egui::pos2(window.center().x, window.min.y + 12.0);
    let to = corner(&harness).to_pos2() + egui::vec2(150.0, 300.0);
    drag(&mut harness, grip, to);
    let hunt = layout(&harness).titled("Hunt").expect("still laid out");
    assert_eq!(hunt.zone, Zone::Left, "in the drawer it was let go over");
    let (id, kept_rect) = (hunt.id, hunt.rect());
    assert!(
        kept_rect.width() <= 300.0 + 0.01,
        "made to fit: {kept_rect:?}"
    );
    assert!(
        kept_rect.min.x >= -0.01,
        "from the drawer's corner: {kept_rect:?}"
    );
    let drawer = harness.state().play.zones.rect(Zone::Left);
    let shown = layout(&harness).shown(id, &harness.state().play.zones);
    assert!(
        drawer
            .zip(shown)
            .is_some_and(|(drawer, shown)| drawer.contains_rect(shown)),
        "{drawer:?} holds {shown:?}"
    );

    if let Some(layout) = harness.state_mut().play.layout.as_mut() {
        layout.drawers.left.open = false;
    }
    harness.run();
    assert!(
        harness.query_by_label("Hunt").is_none(),
        "shut away with its drawer"
    );

    let reopened = Play::new(0, "Ashryn", None, Some(dir.clone()));
    let saved = reopened.layout.as_ref().expect("saved");
    assert_eq!(saved.titled("Hunt").map(|hunt| hunt.zone), Some(Zone::Left));
    assert!(saved.drawers.left.open, "saved as it was when last kept");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A play window over Ashryn whose layout is kept in `dir`, emptied first.
fn saving<'a>(dir: &std::path::Path) -> Harness<'a, Scene> {
    let _ = std::fs::remove_dir_all(dir);
    let mut scene = Scene::new();
    scene.play = Play::new(0, "Ashryn", None, Some(dir.to_path_buf()));
    let mut harness = Harness::builder()
        .with_size((1000.0, 700.0))
        .build_ui_state(|ui, scene: &mut Scene| scene.draw(ui), scene);
    harness.run();
    harness
}

/// A drawer's edge dragged toward the main area makes it larger by as
/// much, and saved so; locked, the edge is not there to drag.
#[test]
fn a_drawers_edge_sizes_it() {
    let dir = std::env::temp_dir().join(format!("cena-play-drawer-edge-{}", std::process::id()));
    let mut harness = saving(&dir);
    open_left(&mut harness, 200.0, Mode::Push, 1.0);
    let edge = corner(&harness).to_pos2() + egui::vec2(200.0, 300.0);
    drag(&mut harness, edge, edge + egui::vec2(60.0, 0.0));
    let size = layout(&harness).drawers.left.size;
    assert!((size - 260.0).abs() < 1.0, "{size}");
    let reopened = Play::new(0, "Ashryn", None, Some(dir.clone()));
    let saved = reopened.layout.map(|layout| layout.drawers.left.size);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        saved.is_some_and(|saved| (saved - size).abs() < 0.01),
        "{saved:?}"
    );

    if let Some(layout) = harness.state_mut().play.layout.as_mut() {
        layout.locked = true;
    }
    harness.run();
    let edge = corner(&harness).to_pos2() + egui::vec2(260.0, 300.0);
    drag(&mut harness, edge, edge + egui::vec2(60.0, 0.0));
    let locked = layout(&harness).drawers.left.size;
    assert!((locked - 260.0).abs() < 1.0, "locked: {locked}");
}

/// Over the story's rat, an opaque clip drawer takes the click on its bare
/// backdrop: the player cannot see what they would be clicking. Made
/// clear, the drawer hides nothing, and the click reaches the rat.
#[test]
fn a_press_passes_a_drawer_only_when_it_hides_nothing() {
    let mut harness = harness();
    heard(harness.state_mut(), &[("a grey rat", Some(rat()))]);
    harness.run();
    let at = harness.get_by_label("a grey rat").rect().center();
    open_left(&mut harness, 300.0, Mode::Clip, 1.0);
    click_at(&mut harness, at);
    assert!(harness.state().asked.is_empty(), "stopped by the drawer");

    open_left(&mut harness, 300.0, Mode::Clip, 0.1);
    click_at(&mut harness, at);
    assert_eq!(
        harness.state().asked,
        [Asked::Quietly("_menu #456 1".to_owned())],
        "through a drawer that hides nothing"
    );
}
