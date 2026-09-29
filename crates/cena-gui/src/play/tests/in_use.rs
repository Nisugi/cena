//! The window in use (`plan/52` step 4): the story until another widget is
//! clicked in, and what the scrolling keys act on.

use std::sync::Arc;

use cena_session::{Event, Generation, ObservedEvent, SessionId};

use super::*;
use crate::keys::Action;

/// The story told `lines` more lines.
fn told(story: &mut Story, lines: usize) {
    for n in 0..lines {
        story.hear(
            &ObservedEvent {
                session: SessionId::FIRST,
                generation: Generation::FIRST,
                cursor: 1,
                event: Event::Line(Arc::new(cena_session::Line::new(
                    "",
                    cena_session::ChunkLine::plain(&format!("line {n}")).runs,
                ))),
            },
            None,
        );
    }
}

/// The placed id of the widget `widget`.
fn placed(harness: &Harness<'_, Scene>, widget: &crate::widget::Widget) -> Option<u32> {
    harness
        .state()
        .play
        .layout
        .as_ref()?
        .placed()
        .into_iter()
        .find(|placed| placed.widget == *widget)
        .map(|placed| placed.id)
}

/// The story is in use until another widget is clicked in; a scrolling key
/// acts on the one in use, so a page back splits the story, and once the
/// room is clicked, the room scrolls and the story does not.
#[test]
fn the_scrolling_keys_act_on_the_window_in_use() {
    let mut harness = harness();
    told(&mut harness.state_mut().story, 200);
    harness.run();
    let story = placed(&harness, &crate::widget::Widget::Story);
    assert!(story.is_some());
    assert_eq!(harness.state().play.in_use(), story, "the story at first");

    let act = |harness: &mut Harness<'_, Scene>, action| {
        let context = harness.ctx.clone();
        assert_eq!(harness.state_mut().play.act(&context, action), None);
        harness.run();
        harness.run();
    };
    act(&mut harness, Action::ScrollPageUp);
    assert!(
        harness.query_by_label("⬇ Newest").is_some(),
        "the story split"
    );
    act(&mut harness, Action::ScrollBottom);
    assert!(harness.query_by_label("⬇ Newest").is_none(), "and joined");

    harness.get_by_label("Rawknuckle's, Watering Hole").click();
    harness.run();
    let clicked = harness.state().play.in_use();
    assert!(
        clicked.is_some() && clicked != story,
        "the room's name, in use"
    );
    act(&mut harness, Action::ScrollPageUp);
    assert!(
        harness.query_by_label("⬇ Newest").is_none(),
        "the room scrolls, not the story"
    );
}

/// Ctrl+F opens the Find bar over the window in use, the story, with the
/// keyboard; what is typed is found whatever its case, the latest first,
/// and the story brought back to it, which splits it; F3 goes to the one
/// found before, Shift+F3 after; Escape closes the bar (`plan/52` step 6).
#[test]
fn find_looks_through_the_story() {
    let mut harness = harness();
    told(&mut harness.state_mut().story, 200);
    harness.run();
    let act = |harness: &mut Harness<'_, Scene>, action| {
        let context = harness.ctx.clone();
        assert_eq!(harness.state_mut().play.act(&context, action), None);
        harness.run();
        harness.run();
    };
    act(&mut harness, Action::Find);
    let field = || (Role::TextInput, "Find");
    assert!(
        harness
            .query_by_role_and_label(field().0, field().1)
            .is_some(),
        "open"
    );
    harness
        .get_by_role_and_label(field().0, field().1)
        .type_text("LINE 7");
    // Frames enough for the split to open and its bars to show.
    harness.run_steps(8);
    assert!(
        harness.query_by_label("1 of 11").is_some(),
        "7 and 70 to 79"
    );
    assert!(
        harness.query_by_label("⬇ Newest").is_some(),
        "the story brought back to line 79, and split"
    );
    act(&mut harness, Action::FindNext);
    assert!(harness.query_by_label("2 of 11").is_some());
    act(&mut harness, Action::FindPrevious);
    act(&mut harness, Action::FindPrevious);
    assert!(
        harness.query_by_label("1 of 11").is_some(),
        "no newer than the latest"
    );
    harness.get_by_role_and_label(field().0, field().1).focus();
    harness.run();
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(
        harness
            .query_by_role_and_label(field().0, field().1)
            .is_none(),
        "closed"
    );
}

/// The Find bar over the story, what it found marked, as drawn.
#[test]
fn the_find_bar_as_drawn() {
    let mut scene = Scene::new();
    told(&mut scene.story, 200);
    let mut harness = Harness::builder()
        .with_size((1000.0, 700.0))
        .wgpu()
        .build_ui_state(|ui, scene: &mut Scene| scene.draw(ui), scene);
    harness.run();
    let context = harness.ctx.clone();
    let _ = harness.state_mut().play.act(&context, Action::Find);
    harness.run();
    harness
        .get_by_role_and_label(Role::TextInput, "Find")
        .type_text("line 7");
    harness.run_steps(8);
    harness.snapshot("find");
}

/// Tab and Shift+Tab's actions target the creature after and before the one
/// the game says is targeted, round its list (`plan/52` step 7).
#[test]
fn the_targets_go_round_the_games_list() {
    let mut harness = harness();
    let targeting = &mut harness.state_mut().snapshot.state.targeting;
    targeting.read("#101,#202,#303", Some("a kobold"));
    harness.run();
    let context = harness.ctx.clone();
    let play = &mut harness.state_mut().play;
    assert_eq!(
        play.act(&context, Action::TargetNext).as_deref(),
        Some("target #202")
    );
    assert_eq!(
        play.act(&context, Action::TargetPrevious).as_deref(),
        Some("target #303"),
        "round, from the first"
    );
    assert_eq!(
        play.act(&context, Action::TargetClear).as_deref(),
        Some("target clear")
    );
}

/// Hydra's own actions on the window: each drawer opened and shut, Lock
/// holding every window and ending Arrange, Arrange only while unlocked
/// (`plan/52` step 8).
#[test]
fn the_drawers_lock_and_arrange_by_key() {
    let mut harness = harness();
    harness.run();
    let context = harness.ctx.clone();
    let play = &mut harness.state_mut().play;
    let open = |play: &Play| {
        play.layout
            .as_ref()
            .map(|layout| (layout.drawers.left.open, layout.drawers.top.open))
    };
    assert_eq!(open(play), Some((false, false)));
    assert_eq!(play.act(&context, Action::DrawerLeft), None);
    assert_eq!(open(play), Some((true, false)), "the left one");
    let _ = play.act(&context, Action::DrawerLeft);
    assert_eq!(open(play), Some((false, false)), "and shut");

    let _ = play.act(&context, Action::Arrange);
    assert!(play.arranging);
    let _ = play.act(&context, Action::Lock);
    assert!(play.layout.as_ref().is_some_and(|layout| layout.locked));
    assert!(!play.arranging, "locked, not arranging");
    let _ = play.act(&context, Action::Arrange);
    assert!(!play.arranging, "not while locked");
    let _ = play.act(&context, Action::Lock);
    let _ = play.act(&context, Action::Arrange);
    assert!(play.arranging, "free again");
}
