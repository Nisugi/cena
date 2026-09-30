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
                minimap: None,
                tags: false,
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
    wheel_at(harness, egui::pos2(200.0, 100.0), by);
}

/// The wheel turned `by` points with the pointer `at`.
fn wheel_at(harness: &mut Harness<'_, Story>, at: egui::Pos2, by: f32) {
    harness.hover_at(at);
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

/// The wheel over the bottom pane scrolls the top as over the top: where
/// the pointer is does not matter (the author, 2026-09-29).
#[test]
fn the_wheel_below_the_separator_scrolls_the_top() {
    let mut harness = story_of(200);
    wheel(&mut harness, 600.0);
    let opened = split(&harness);
    assert!(opened.open);
    let bar = harness.get_by_label("⬇ Newest").rect();
    wheel_at(&mut harness, egui::pos2(200.0, bar.max.y + 30.0), 300.0);
    let now = split(&harness);
    assert!(now.open, "still split");
    assert!(
        now.offset < opened.offset - 200.0,
        "the top went back: {} -> {}",
        opened.offset,
        now.offset
    );
    wheel_at(&mut harness, egui::pos2(200.0, bar.max.y + 30.0), -50_000.0);
    assert!(!split(&harness).open, "and down to the newest, one pane");
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
                    minimap: None,
                    tags: false,
                };
                let _ = Widget::Story.draw_with(ui, &seen, egui::Id::new("s"), &Chosen::default());
            },
            story,
        );
    harness.run();
    wheel(&mut harness, 600.0);
    harness.snapshot("split");
}

/// A key asked of the widget by its id, and the frames it takes.
fn keyed(harness: &mut Harness<'_, Story>, scroll: super::Scroll) {
    super::ask(&harness.ctx, egui::Id::new("s"), scroll);
    harness.run();
    harness.run();
}

/// A key back a page splits as the wheel does, the top a page above the
/// newest; a line back from there is a line higher; to the top is the
/// oldest line; forward pages to the newest close the split, and so does
/// the key to the newest (`plan/52` step 4).
#[test]
fn keys_scroll_the_split() {
    use super::Scroll;
    let mut harness = story_of(80);
    let end = split(&harness).end;
    assert!(end > 0.0, "more lines than fit");
    keyed(&mut harness, Scroll::PageUp);
    let paged = split(&harness);
    assert!(paged.open, "split");
    assert!(
        paged.offset < end - 100.0 && paged.offset > 0.0,
        "a page back: {} of {end}",
        paged.offset
    );
    keyed(&mut harness, Scroll::LineUp);
    let lined = split(&harness).offset;
    assert!(
        lined < paged.offset && lined > paged.offset - 40.0,
        "a line: {} -> {lined}",
        paged.offset
    );
    keyed(&mut harness, Scroll::Top);
    assert!(split(&harness).offset < 0.5, "the oldest line");
    assert!(harness.query_by_label("⬇ Newest").is_some());
    for _ in 0..20 {
        keyed(&mut harness, Scroll::PageDown);
    }
    assert!(!split(&harness).open, "paged to the newest: one pane");

    keyed(&mut harness, Scroll::LineUp);
    assert!(split(&harness).open, "a line back splits too");
    keyed(&mut harness, Scroll::Bottom);
    assert!(!split(&harness).open, "to the newest: one pane");
    keyed(&mut harness, Scroll::PageDown);
    assert!(!split(&harness).open, "at the newest, forward is nothing");
}

/// A widget that scrolls without splitting -- a list, the room -- takes the
/// same keys: a page, a line, the top and the bottom.
#[test]
fn keys_scroll_a_widget_that_does_not_split() {
    use super::Scroll;
    let mut harness = Harness::builder().with_size((300.0, 200.0)).build_ui_state(
        |ui, offset: &mut (f32, f32)| {
            let scroll = super::asked(ui, egui::Id::new("list"));
            let shown = egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| {
                    super::keyed(ui, scroll, |ui| {
                        for n in 0..100 {
                            ui.label(format!("item {n}"));
                        }
                    });
                });
            *offset = (shown.state.offset.y, super::newest(&shown));
        },
        (0.0, 0.0),
    );
    harness.run();
    let key = |harness: &mut Harness<'_, (f32, f32)>, scroll| {
        super::ask(&harness.ctx, egui::Id::new("list"), scroll);
        harness.run_steps(30);
        harness.state().0
    };
    let paged = key(&mut harness, Scroll::PageDown);
    assert!(paged > 100.0, "a page down: {paged}");
    let lined = key(&mut harness, Scroll::LineUp);
    assert!(
        lined < paged && lined > paged - 40.0,
        "a line up: {paged} -> {lined}"
    );
    let bottom = key(&mut harness, Scroll::Bottom);
    assert!(
        (bottom - harness.state().1).abs() < 0.5,
        "the bottom: {bottom}"
    );
    assert!(key(&mut harness, Scroll::Top) < 0.5, "back at the top");
}

/// From one pane, the key to the oldest line splits, the top at the oldest.
#[test]
fn the_key_to_the_top_splits() {
    let mut harness = story_of(80);
    keyed(&mut harness, super::Scroll::Top);
    let split = split(&harness);
    assert!(split.open && split.offset < 0.5, "{}", split.offset);
}

/// A widget of the catalog that scrolls, the Room, takes the key asked of
/// it as it is drawn.
#[test]
fn a_catalog_widget_takes_its_key() {
    let mut harness = Harness::builder().with_size((300.0, 200.0)).build_ui_state(
        |ui, story: &mut Story| {
            let seen = Seen {
                snapshot: None,
                story,
                hunt: None,
                who: None,
                open: &[],
                minimap: None,
                tags: false,
            };
            let _ = Widget::Room.draw_with(ui, &seen, egui::Id::new("r"), &Chosen::default());
        },
        Story::default(),
    );
    harness.run();
    super::ask(&harness.ctx, egui::Id::new("r"), super::Scroll::PageUp);
    harness.run();
    let left = harness
        .ctx
        .data(|data| data.get_temp::<super::Scroll>(egui::Id::new("r").with(super::ASKED)));
    assert_eq!(left, None, "taken");
}

/// A line selected stays the line selected as new lines drop old ones, and
/// Ctrl+C copies it (the author, 2026-09-29: *"Texting coming in changes
/// which lines are selected"*, *"cannot copy/paste"*).
#[test]
fn a_selection_holds_its_line_as_old_ones_go() {
    let mut harness = story_of(MAX_STORY);
    let chosen = format!("line {}", MAX_STORY - 4);
    let rect = harness.get_by_label(&chosen).rect();
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let (from, to) = (rect.left_center(), rect.right_center());
    harness.hover_at(from);
    harness.step();
    harness.event(button(from, true));
    harness.step();
    for step in 1..=4u8 {
        harness.hover_at(from + (to - from) * (f32::from(step) / 4.0));
        harness.step();
    }
    harness.event(button(to, false));
    harness.step();
    for n in MAX_STORY..MAX_STORY + 2 {
        harness.state_mut().hear(&line(&format!("line {n}")), None);
    }
    harness.run();
    assert_eq!(harness.state().dropped, 2, "two went");
    harness.event(egui::Event::Copy);
    harness.step();
    let copied: Vec<String> = harness
        .output()
        .platform_output
        .commands
        .iter()
        .filter_map(|command| match command {
            egui::OutputCommand::CopyText(text) => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(copied, [chosen]);
}
