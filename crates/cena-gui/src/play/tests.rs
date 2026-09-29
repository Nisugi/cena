//! The play window through `egui_kittest`, as a screen reader finds it; the
//! last test renders it and compares it with `tests/snapshots/play.png`.

use super::*;
use crate::fixture::{snapshot, story};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

/// A play window over Ashryn, and what it asked for.
struct Scene {
    play: Play,
    snapshot: Snapshot,
    story: Story,
    hunt: Option<cena_ui::HuntView>,
    /// The other characters running, which a widget may follow.
    others: Vec<crate::widget::Character>,
    /// The presets a player saved.
    presets: crate::layout::Library,
    asked: Vec<Asked>,
}

impl Scene {
    fn new() -> Self {
        Self {
            play: Play {
                room_parts: true,
                ..Play::new(0, "Ashryn", None, None)
            },
            snapshot: snapshot(),
            story: story(),
            hunt: Some(cena_ui::HuntView {
                running: "ojandhaart".to_owned(),
                phase: "resting (out of mana)".to_owned(),
                doing: "waiting 5s".to_owned(),
                target: None,
                waiting: Some("mana 30%, wants 50%".to_owned()),
            }),
            others: Vec::new(),
            presets: crate::layout::Library::default(),
            asked: Vec::new(),
        }
    }

    fn draw(&mut self, ui: &mut egui::Ui) {
        let view = PlayView {
            name: "Ashryn",
            lifecycle: &LifecycleView::Ready,
            snapshot: Some(&self.snapshot),
            story: &self.story,
            now: Instant::now(),
            hunt: self.hunt.as_ref(),
            numlock: None,
            set: 0,
            keys: &[],
            others: &self.others,
            presets: &self.presets,
            lich: false,
        };
        if let Some(asked) = self.play.show(ui, &view) {
            self.asked.push(asked);
        }
    }
}

/// Where the window titled `title` sits, as the layout keeps it.
fn kept(harness: &Harness<'_, Scene>, title: &str) -> Option<egui::Rect> {
    harness
        .state()
        .play
        .layout
        .as_ref()
        .and_then(|layout| layout.titled(title))
        .map(crate::layout::Holder::rect)
}

fn harness<'a>() -> Harness<'a, Scene> {
    Harness::builder()
        .with_size((1000.0, 700.0))
        .build_ui_state(|ui, scene: &mut Scene| scene.draw(ui), Scene::new())
}

#[test]
fn the_window_shows_what_a_player_glances_at() {
    let harness = harness();
    for label in [
        "Ashryn",
        "Ready",
        "Left: a steel broadsword",
        "Right: empty",
        "HP 348/400 87%",
        "MP 48/120 40%",
        "SP ?",
        "Rawknuckle's, Watering Hole",
        "a kobold",
        "Maravel",
        "Obvious exits: north, out",
        "You swing a steel broadsword at a kobold!",
        ">look",
        "Hunt: resting until mana is 50%.",
        "A kobold is here!",
        "ojandhaart",
        "resting (out of mana)",
        "Waiting: mana 30%, wants 50%",
    ] {
        assert!(harness.query_by_label(label).is_some(), "{label}");
    }
    // "RT 30s" or near it, counted on the wall clock: never "RT —".
    assert!(
        harness
            .query_all_by_label_contains("RT ")
            .filter_map(|node| node.value())
            .any(|label| label.ends_with('s')),
        "the roundtime counts"
    );
}

#[test]
fn stop_asks_to_stop() {
    let mut harness = harness();
    harness.get_by_label("Stop").click();
    harness.run();
    assert_eq!(harness.state().asked, [Asked::Stop]);
}

/// A bar's right-click opens its own page in the settings menu (the
/// author, 2026-09-28: *"It should take you to settings to edit that
/// bar"*); a widget with no settings of its own offers none. The Keys menu
/// opens Hydra's Keys page.
#[test]
fn a_widget_opens_its_own_settings() {
    let mut harness = harness();
    harness
        .get_by_label("Waiting: mana 30%, wants 50%")
        .click_secondary();
    harness.run();
    assert!(
        harness.query_by_label("Settings...").is_none(),
        "the hunt panel has none"
    );
    harness.key_press(egui::Key::Escape);
    harness.run();
    harness.get_by_label_contains("HP ").click_secondary();
    harness.run();
    harness.get_by_label("Settings...").click();
    harness.run();
    harness.get_by_label("Keys").click();
    harness.run();
    harness.get_by_label("Change the keys...").click();
    harness.run();
    let asked = &harness.state().asked;
    assert!(
        matches!(&asked[..], [Asked::Settings(Some(page)), Asked::Keys] if page.starts_with("widget:")),
        "{asked:?}"
    );
}

/// A story line's right-click offers a trigger on its words, which asks
/// the window for the trigger editor (`plan/54` step 4).
#[test]
fn a_story_lines_right_click_makes_a_trigger_of_it() {
    let mut harness = harness();
    let line = "You swing a steel broadsword at a kobold!";
    harness.get_by_label(line).hover();
    harness.run();
    harness.get_by_label(line).click_secondary();
    harness.run();
    harness
        .get_by_label("Make a trigger from this line")
        .click();
    harness.run();
    assert_eq!(harness.state().asked, [Asked::TriggerFrom(line.to_owned())]);
}

/// The Lich switch asks for the character's own Lich (`plan/51`).
#[test]
fn the_lich_switch_asks_for_lich() {
    let mut harness = harness();
    harness.get_by_label("Lich").click();
    harness.run();
    assert_eq!(harness.state().asked, [Asked::Lich(true)]);
}

/// The author's complaint about Despana, and `VellumFE`'s answer: a click
/// that nothing else took puts the keyboard back in the command input.
#[test]
fn a_click_nothing_took_returns_the_keyboard_to_the_input() {
    let mut harness = harness();
    harness.run();
    assert!(harness.get_by_role(Role::TextInput).is_focused(), "at once");
    harness.get_by_label("Stop").focus();
    harness.run();
    harness
        .get_by_label("You swing a steel broadsword at a kobold!")
        .click();
    harness.run();
    harness.run();
    assert!(harness.get_by_role(Role::TextInput).is_focused());
}

/// A pane dragged by its title to within a few points of the window's left
/// edge lands on it, and the layout is kept under the character's name: a
/// new window for Ashryn opens with Room where it was left.
#[test]
fn a_dragged_pane_snaps_and_is_kept_by_name() {
    let dir = std::env::temp_dir().join(format!("cena-play-layout-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut scene = Scene::new();
    scene.play = Play::new(0, "Ashryn", None, Some(dir.clone()));
    let mut harness = Harness::builder()
        .with_size((1000.0, 700.0))
        .build_ui_state(|ui, scene: &mut Scene| scene.draw(ui), scene);
    harness.run();
    // No grid: a pane is rarely a whole number of cells wide, so one edge or
    // the other is always on a grid line, and here the edge is the target.
    if let Some(layout) = harness.state_mut().play.layout.as_mut() {
        layout.grid = 0.0;
    }
    let before = kept(&harness, "Room").expect("fitted");
    // By its title bar, as a player moves a pane.
    let window = harness.get_by_label("Room").rect();
    let grip = egui::pos2(window.center().x, window.min.y + 12.0);
    // Five points short of the left edge: near enough to snap to it.
    let to = grip + egui::vec2(5.0 - before.min.x, 0.0);
    // Frame by frame: a window being dragged asks for the next frame.
    harness.hover_at(grip);
    harness.step();
    harness.drag_at(grip);
    harness.step();
    for step in 1..=4u8 {
        harness.hover_at(grip + (to - grip) * (f32::from(step) / 4.0));
        harness.step();
    }
    harness.drop_at(to);
    harness.step();
    harness.step();
    let after = kept(&harness, "Room").expect("still laid out");
    assert!(after.min.x.abs() < 0.01, "snapped to the edge: {after:?}");
    assert_eq!(after.size(), before.size(), "a move keeps the size");
    assert!(harness.state().play.engaged.is_empty(), "the gesture ended");

    let reopened = Play::new(0, "Ashryn", None, Some(dir.clone()));
    assert_eq!(
        reopened
            .layout
            .as_ref()
            .and_then(|layout| layout.titled("Room"))
            .map(crate::layout::Holder::rect),
        Some(after)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A small move in open space stays where it was put: a window's own
/// edges, where it started, are no target for it, or every nudge would
/// snap back.
#[test]
fn a_small_move_is_not_undone_by_the_windows_own_edges() {
    let mut harness = harness();
    harness.run();
    let room = harness
        .state()
        .play
        .layout
        .as_ref()
        .and_then(|layout| layout.titled("Room"))
        .map(|holder| holder.id)
        .expect("a room");
    let open = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(240.0, 160.0));
    if let Some(layout) = harness.state_mut().play.layout.as_mut() {
        layout.grid = 0.0;
        layout.set(room, open);
    }
    harness.run();
    let window = harness.get_by_label("Room").rect();
    let grip = egui::pos2(window.center().x, window.min.y + 12.0);
    let to = grip + egui::vec2(4.0, 3.0);
    harness.hover_at(grip);
    harness.step();
    harness.drag_at(grip);
    harness.step();
    for step in 1..=4u8 {
        harness.hover_at(grip + (to - grip) * (f32::from(step) / 4.0));
        harness.step();
    }
    harness.drop_at(to);
    harness.step();
    harness.step();
    let after = kept(&harness, "Room").expect("still laid out");
    assert!(
        (after.min.x - 104.0).abs() < 0.01 && (after.min.y - 103.0).abs() < 0.01,
        "moved by the nudge: {after:?}"
    );
}

/// A pane resized by its edge moves only that edge, which lands on the
/// grid: a held pane's size is let go for the gesture, or no handle could
/// resize it. The story's bottom edge, which borders no other pane: where
/// panes tile, two windows' handles meet on one edge.
#[test]
fn a_resized_pane_lands_on_the_grid() {
    let mut harness = harness();
    harness.run();
    let before = kept(&harness, "Story").expect("fitted");
    let window = harness.get_by_label("Story").rect();
    let edge = egui::pos2(window.center().x, window.max.y - 2.0);
    let to = edge - egui::vec2(0.0, 37.0);
    harness.hover_at(edge);
    harness.step();
    harness.drag_at(edge);
    harness.step();
    for step in 1..=4u8 {
        harness.hover_at(edge + (to - edge) * (f32::from(step) / 4.0));
        harness.step();
    }
    harness.drop_at(to);
    harness.step();
    harness.step();
    let after = kept(&harness, "Story").expect("still laid out");
    assert!(
        after.height() < before.height() - 20.0,
        "{before:?} -> {after:?}"
    );
    assert!((after.max.y % 10.0).abs() < 0.01, "on the grid: {after:?}");
    assert!((after.min.y - before.min.y).abs() < 0.01, "the top stayed");
    assert!(
        (after.width() - before.width()).abs() < 0.01,
        "and the width"
    );
}

/// On an edge two panes share, a drag resizes one of them -- whichever
/// egui gives the handle to -- rather than neither.
#[test]
fn a_shared_edge_resizes() {
    let mut harness = harness();
    harness.run();
    let rects = |harness: &Harness<'_, Scene>| (kept(harness, "Room"), kept(harness, "Hydra"));
    let (room, hydra) = rects(&harness);
    // Two points inside Room, where Room's window is on top but Hydra's
    // handle, drawn later, also reaches: egui resizes Hydra.
    let window = harness.get_by_label("Room").rect();
    let edge = egui::pos2(window.center().x, window.max.y - 2.0);
    let to = edge + egui::vec2(0.0, 33.0);
    harness.hover_at(edge);
    harness.step();
    harness.drag_at(edge);
    harness.step();
    for step in 1..=4u8 {
        harness.hover_at(edge + (to - edge) * (f32::from(step) / 4.0));
        harness.step();
    }
    harness.drop_at(to);
    harness.step();
    harness.step();
    let (room_after, hydra_after) = rects(&harness);
    assert!(
        room_after != room || hydra_after != hydra,
        "one of them moved its edge"
    );
}

/// The Layout menu lays the windows out afresh, however they were moved.
#[test]
fn the_layout_can_be_fitted_afresh() {
    let mut harness = harness();
    harness.run();
    let fitted = [kept(&harness, "Vitals"), kept(&harness, "Room")];
    let room = harness
        .state()
        .play
        .layout
        .as_ref()
        .and_then(|layout| layout.titled("Room"))
        .map(|holder| holder.id)
        .expect("a room");
    if let Some(layout) = harness.state_mut().play.layout.as_mut() {
        layout.set(
            room,
            egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 100.0)),
        );
    }
    harness.run();
    harness.get_by_label("Layout").click();
    harness.run();
    harness.get_by_label("Lay out afresh").click();
    harness.run();
    // The play area can settle by a point between frames, which moves the
    // bottom edges; the windows the menu sat over are exactly as fitted.
    assert_eq!([kept(&harness, "Vitals"), kept(&harness, "Room")], fitted);
    // And on the screen, not only in the layout: shrunk back from the size
    // it had, a frame after it was asked.
    let shown = harness.get_by_label("Room").rect().size();
    let wanted = fitted[1].expect("fitted").size();
    assert!((shown - wanted).length() < 0.5, "{shown:?} for {wanted:?}");
}

/// Drawn with no roundtime: its seconds come from the wall clock, and an
/// image must not depend on how fast the machine rendering it is.
#[test]
fn the_window_as_drawn() {
    let mut scene = Scene::new();
    scene.snapshot.state.roundtime_ends = None;
    let mut harness = Harness::builder()
        .with_size((1000.0, 700.0))
        .wgpu()
        .build_ui_state(|ui, scene: &mut Scene| scene.draw(ui), scene);
    harness.run();
    harness.snapshot("play");
}

/// The window's layout, once it has one.
fn layout<'a>(harness: &'a Harness<'_, Scene>) -> &'a crate::layout::Layout {
    harness.state().play.layout.as_ref().expect("laid out")
}

mod arrange;
mod drag;
mod drawers;
mod in_use;
mod input;
mod links;
mod menus;
mod pages;
mod tabs;
