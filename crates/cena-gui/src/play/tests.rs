//! The play window through `egui_kittest`, as a screen reader finds it; the
//! last test renders it and compares it with `tests/snapshots/play.png`.

use std::sync::Arc;

use super::*;
use cena_session::{
    Amount as Numbers, ChunkLine, Event, Frame, GameState, Generation, ObservedEvent, ProgressBar,
    SessionId, State,
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
            play: Play::new(0),
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
