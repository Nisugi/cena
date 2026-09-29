//! The split as a player meets it, read from what it keeps: both panes
//! draw every line, so which pane shows a line is a matter of offsets, not
//! of which labels exist.

use std::sync::Arc;

use cena_session::{Event, Generation, ObservedEvent, SessionId};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::Split;
use crate::story::{MAX_STORY, Story};
use crate::widget::{Chosen, Seen, Widget};

fn line(text: &str) -> ObservedEvent {
    ObservedEvent {
        session: SessionId::FIRST,
        generation: Generation::FIRST,
        cursor: 1,
        event: Event::Line(Arc::new(cena_session::Line::new(
            "",
            cena_session::ChunkLine::plain(text).runs,
        ))),
    }
}

/// A story widget 300 points tall over a story of `lines` lines.
fn story_of<'a>(lines: usize) -> Harness<'a, Story> {
    let mut story = Story::default();
    for n in 0..lines {
        story.hear(&line(&format!("line {n}")), None);
    }
    let mut harness = Harness::builder().with_size((400.0, 300.0)).build_ui_state(
        |ui, story: &mut Story| {
            let seen = Seen {
                snapshot: None,
                story,
                hunt: None,
                who: None,
                open: &[],
            };
            let _ = Widget::Story.draw_with(ui, &seen, egui::Id::new("s"), &Chosen::default());
        },
        story,
    );
    harness.run();
    harness
}

/// What the story widget's split keeps.
fn split(harness: &Harness<'_, Story>) -> Split {
    let id = egui::Id::new("s").with("story").with("split");
    harness
        .ctx
        .data(|data| data.get_temp::<Split>(id))
        .unwrap_or_default()
}

/// Whether the line saying `text` is drawn between `top` and `bottom` on
/// the screen: a pane shows it, not only lays it out out of sight.
fn in_sight(harness: &Harness<'_, Story>, text: &str, (top, bottom): (f32, f32)) -> bool {
    harness
        .query_all_by_label(text)
        .any(|node| node.rect().min.y >= top && node.rect().max.y <= bottom)
}

/// The wheel turned `by` points over the widget: back, to older lines,
/// when positive.
fn wheel(harness: &mut Harness<'_, Story>, by: f32) {
    harness.hover_at(egui::pos2(200.0, 100.0));
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, by),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
    // Enough frames for egui's smooth scrolling to play the turn out.
    harness.run_steps(60);
}

/// One pane until the player scrolls back; then two, with the separator's
/// button, the top where the player left it and holding still as lines
/// come in.
#[test]
fn scrolling_back_splits_and_the_top_holds_still() {
    let mut harness = story_of(80);
    assert!(!split(&harness).open, "one pane");
    assert!(harness.query_by_label("⬇ Newest").is_none());
    wheel(&mut harness, 600.0);
    let opened = split(&harness);
    assert!(opened.open, "split");
    let bar = harness.get_by_label("⬇ Newest").rect();
    assert!(
        in_sight(&harness, "line 79", (bar.max.y, 300.0)),
        "the bottom shows the newest"
    );
    for n in 80..90 {
        harness.state_mut().hear(&line(&format!("line {n}")), None);
    }
    harness.run();
    let now = split(&harness);
    assert!(now.open, "still split");
    assert!(
        (now.offset - opened.offset).abs() < 0.5,
        "the top held still: {} -> {}",
        opened.offset,
        now.offset
    );
}

/// With the story full, a new line drops the oldest and the rest move up;
/// the top moves with them by exactly the height of the lines gone, so it
/// stays on its lines.
#[test]
fn the_top_holds_its_lines_as_old_ones_go() {
    let mut harness = story_of(MAX_STORY);
    wheel(&mut harness, 900.0);
    let opened = split(&harness);
    assert!(opened.open);
    let five = opened.ys.get(5).copied().expect("lines marked");
    assert!(five > 0.0);
    for n in MAX_STORY..MAX_STORY + 5 {
        harness.state_mut().hear(&line(&format!("line {n}")), None);
    }
    harness.run();
    assert_eq!(harness.state().dropped, 5, "five went");
    let now = split(&harness);
    assert!(
        (now.offset - (opened.offset - five)).abs() < 0.5,
        "moved up by the five lines' height, {five}: {} -> {}",
        opened.offset,
        now.offset
    );
}

/// The separator dragged down gives the top more; its button goes back to
/// one pane at the newest line, which stays one pane.
#[test]
fn the_separator_drags_and_its_button_goes_back() {
    let mut harness = story_of(80);
    wheel(&mut harness, 600.0);
    let before = split(&harness).share;
    let bar = harness.get_by_label("⬇ Newest").rect();
    // On the separator, clear of its button.
    let grip = egui::pos2(bar.min.x - 40.0, bar.center().y);
    harness.hover_at(grip);
    harness.run();
    harness.drag_at(grip);
    harness.run();
    harness.hover_at(grip + egui::vec2(0.0, 40.0));
    harness.run();
    harness.drop_at(grip + egui::vec2(0.0, 40.0));
    harness.run();
    let after = split(&harness).share;
    assert!(after > before + 0.1, "{before} -> {after}");

    harness.get_by_label("⬇ Newest").click();
    harness.run();
    harness.run();
    assert!(!split(&harness).open, "one pane");
    assert!(
        in_sight(&harness, "line 79", (0.0, 300.0)),
        "at the newest, not scrolled past it"
    );
    harness.state_mut().hear(&line("line 80"), None);
    harness.run();
    assert!(!split(&harness).open, "and it stays one, at the newest");
}

/// Scrolled back down to the newest line, the top closes the split, as a
/// split in Mudlet does.
#[test]
fn the_top_scrolled_to_the_newest_closes_the_split() {
    let mut harness = story_of(80);
    wheel(&mut harness, 600.0);
    assert!(split(&harness).open);
    wheel(&mut harness, -5_000.0);
    assert!(!split(&harness).open, "one pane again");
}

/// The split as a player sees it, rendered and compared with the committed
/// image.
#[test]
fn the_split_as_drawn() {
    let mut story = Story::default();
    for n in 0..80 {
        story.hear(&line(&format!("line {n}")), None);
    }
    let mut harness = Harness::builder()
        .with_size((400.0, 300.0))
        .wgpu()
        .build_ui_state(
            |ui, story: &mut Story| {
                let seen = Seen {
                    snapshot: None,
                    story,
                    hunt: None,
                    who: None,
                    open: &[],
                };
                let _ = Widget::Story.draw_with(ui, &seen, egui::Id::new("s"), &Chosen::default());
            },
            story,
        );
    harness.run();
    wheel(&mut harness, 600.0);
    harness.snapshot("split");
}
