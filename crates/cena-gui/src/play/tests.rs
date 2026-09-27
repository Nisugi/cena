//! The play window through `egui_kittest`, as a screen reader finds it; the
//! last test renders it and compares it with `tests/snapshots/play.png`.

use std::sync::Arc;

use super::*;
use cena_session::{
    Amount as Numbers, ChunkLine, Event, Frame, GameState, Generation, Notice, NoticeKind,
    ObservedEvent, ProgressBar, RoomItem, SessionId, State,
};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

fn bar(id: &str, percent: u32, current: i32, max: i32) -> Frame {
    Frame::ProgressBar(ProgressBar {
        id: id.to_owned(),
        dialog: Some("minivitals".to_owned()),
        percent,
        text: format!("{id} {current}/{max}"),
        amount: Some(Numbers { current, max }),
        time_remaining_secs: None,
        attrs: Vec::new(),
    })
}

fn item(noun: &str, text: &str) -> RoomItem {
    RoomItem {
        id: format!("-{}", noun.len()),
        noun: noun.to_owned(),
        text: text.to_owned(),
        before: None,
        after: None,
        status: None,
    }
}

/// Ashryn mid-hunt: hurt, holding a sword, a kobold and a player in the
/// room, in roundtime.
fn snapshot() -> Snapshot {
    let mut state = GameState::default();
    state.apply(&bar("health", 87, 348, 400));
    state.apply(&bar("mana", 40, 48, 120));
    state.apply(&Frame::LeftHand {
        item: "a steel broadsword".to_owned(),
        link: None,
    });
    state.apply(&Frame::RightHand {
        item: "Empty".to_owned(),
        link: None,
    });
    state.apply(&Frame::Prompt {
        time: "1000".to_owned(),
        text: ">".to_owned(),
    });
    state.roundtime_ends = Some(1_030);
    for id in ["room objs", "room players"] {
        state.apply(&Frame::Component {
            id: id.to_owned(),
            body: ChunkLine::plain("").runs,
        });
    }
    state.room.title = Some("[Rawknuckle's, Watering Hole]".to_owned());
    state.room.exits = Some(vec!["north".to_owned(), "out".to_owned()]);
    state.room.creatures = vec![item("kobold", "a kobold")];
    state.room.players = vec![item("Maravel", "Maravel")];
    Snapshot {
        session: SessionId::FIRST,
        generation: Generation::FIRST,
        cursor: 0,
        state,
        lifecycle: State::Ready,
        retry: None,
        stopped: None,
        triggers: Arc::default(),
    }
}

fn story() -> Story {
    let mut story = Story::default();
    let observed = |event| ObservedEvent {
        session: SessionId::FIRST,
        generation: Generation::FIRST,
        cursor: 1,
        event,
    };
    story.hear(
        &observed(Event::Line(Arc::new(cena_session::Line::new(
            "",
            ChunkLine::plain("You swing a steel broadsword at a kobold!").runs,
        )))),
        None,
    );
    story.typed("look");
    story.tell(Notice::line(
        NoticeKind::Info,
        "Hunt: resting until mana is 50%.",
    ));
    story.hear(
        &observed(Event::Attention(Arc::new(
            cena_session::trigger::Attention {
                trigger: "kobold".to_owned(),
                sound: None,
                notify: None,
                alert: Some("A kobold is here!".to_owned()),
                cooldown: 0,
            },
        ))),
        None,
    );
    story
}

/// A play window over Ashryn, and what it asked for.
struct Scene {
    play: Play,
    snapshot: Snapshot,
    story: Story,
    asked: Vec<Asked>,
}

impl Scene {
    fn new() -> Self {
        Self {
            play: Play::new(0, "Ashryn", None),
            snapshot: snapshot(),
            story: story(),
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
            numlock: None,
            keys: &[],
        };
        if let Some(asked) = self.play.show(ui, &view) {
            self.asked.push(asked);
        }
    }
}

fn harness<'a>() -> Harness<'a, Scene> {
    Harness::builder()
        .with_size((900.0, 520.0))
        .build_ui_state(|ui, scene: &mut Scene| scene.draw(ui), Scene::new())
}

#[test]
fn the_window_shows_what_a_player_glances_at() {
    let harness = harness();
    for label in [
        "Ashryn",
        "Ready",
        "Left: a steel broadsword · Right: empty",
        "HP 348/400 87%",
        "MP 48/120 40%",
        "SP ?",
        "[Rawknuckle's, Watering Hole]",
        "a kobold",
        "Maravel",
        "Obvious exits: north, out",
        "You swing a steel broadsword at a kobold!",
        "> look",
        "Hunt: resting until mana is 50%.",
        "A kobold is here!",
    ] {
        assert!(harness.query_by_label(label).is_some(), "{label}");
    }
    assert!(
        harness.query_by_label_contains("RT ").is_some(),
        "the roundtime counts"
    );
}

/// Enter sends and clears; up and down walk back through what was sent.
#[test]
fn enter_sends_and_up_walks_back() {
    let mut harness = harness();
    harness.run();
    for line in ["look", "north"] {
        harness.get_by_role(Role::TextInput).type_text(line);
        harness.run();
        harness.key_press(egui::Key::Enter);
        harness.run();
    }
    assert_eq!(
        harness.state().asked,
        [
            Asked::Send("look".to_owned()),
            Asked::Send("north".to_owned())
        ]
    );
    let typed = |harness: &Harness<'_, Scene>| harness.state().play.input.clone();
    assert_eq!(typed(&harness), "", "cleared once sent");
    let mut walked = Vec::new();
    for key in [
        egui::Key::ArrowUp,
        egui::Key::ArrowUp,
        egui::Key::ArrowDown,
        egui::Key::ArrowDown,
    ] {
        harness.key_press(key);
        harness.run();
        walked.push(typed(&harness));
    }
    assert_eq!(walked, ["north", "look", "north", ""]);
}

#[test]
fn stop_asks_to_stop() {
    let mut harness = harness();
    harness.get_by_label("Stop").click();
    harness.run();
    assert_eq!(harness.state().asked, [Asked::Stop]);
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
    scene.play = Play::new(0, "Ashryn", Some(dir.clone()));
    let mut harness = Harness::builder()
        .with_size((900.0, 520.0))
        .build_ui_state(|ui, scene: &mut Scene| scene.draw(ui), scene);
    harness.run();
    let room = |harness: &Harness<'_, Scene>| {
        harness
            .state()
            .play
            .layout
            .as_ref()
            .map(|layout| layout.rect(crate::layout::Pane::Room))
    };
    // No grid: a pane is rarely a whole number of cells wide, so one edge or
    // the other is always on a grid line, and here the edge is the target.
    if let Some(layout) = harness.state_mut().play.layout.as_mut() {
        layout.grid = 0.0;
    }
    let before = room(&harness).expect("fitted");
    let grip = harness.get_by_label("Room").rect().center();
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
    let after = room(&harness).expect("still laid out");
    assert!(after.min.x.abs() < 0.01, "snapped to the edge: {after:?}");
    assert_eq!(after.size(), before.size(), "a move keeps the size");
    assert!(harness.state().play.engaged.is_empty(), "the gesture ended");

    let reopened = Play::new(0, "Ashryn", Some(dir.clone()));
    assert_eq!(
        reopened
            .layout
            .map(|layout| layout.rect(crate::layout::Pane::Room)),
        Some(after)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A pane resized by its edge moves only that edge, which lands on the
/// grid: a held pane's size is let go for the gesture, or no handle could
/// resize it. The story's bottom edge, which borders no other pane: where
/// panes tile, two windows' handles meet on one edge.
#[test]
fn a_resized_pane_lands_on_the_grid() {
    let mut harness = harness();
    harness.run();
    let story = |harness: &Harness<'_, Scene>| {
        harness
            .state()
            .play
            .layout
            .as_ref()
            .map(|layout| layout.rect(crate::layout::Pane::Story))
    };
    let before = story(&harness).expect("fitted");
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
    let after = story(&harness).expect("still laid out");
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
    let rects = |harness: &Harness<'_, Scene>| {
        let layout = harness.state().play.layout.clone().expect("fitted");
        (
            layout.rect(crate::layout::Pane::Room),
            layout.rect(crate::layout::Pane::Hydra),
        )
    };
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

/// The Layout menu fits the panes afresh, however they were moved.
#[test]
fn the_layout_can_be_fitted_afresh() {
    let mut harness = harness();
    harness.run();
    let fitted = harness.state().play.layout.clone().expect("fitted");
    if let Some(layout) = harness.state_mut().play.layout.as_mut() {
        layout.set(
            crate::layout::Pane::Room,
            egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 100.0)),
        );
    }
    harness.run();
    harness.get_by_label("Layout").click();
    harness.run();
    harness.get_by_label("Fit the panes afresh").click();
    harness.run();
    let now = harness.state().play.layout.clone().expect("fitted again");
    // The pane area can settle by a point between frames, which moves the
    // bottom edges; the panes the menu sat over are exactly as fitted.
    for pane in [crate::layout::Pane::Vitals, crate::layout::Pane::Room] {
        assert_eq!(now.rect(pane), fitted.rect(pane), "{pane:?}");
    }
}

/// Drawn with no roundtime: its seconds come from the wall clock, and an
/// image must not depend on how fast the machine rendering it is.
#[test]
fn the_window_as_drawn() {
    let mut scene = Scene::new();
    scene.snapshot.state.roundtime_ends = None;
    let mut harness = Harness::builder()
        .with_size((900.0, 520.0))
        .wgpu()
        .build_ui_state(|ui, scene: &mut Scene| scene.draw(ui), scene);
    harness.run();
    harness.snapshot("play");
}
