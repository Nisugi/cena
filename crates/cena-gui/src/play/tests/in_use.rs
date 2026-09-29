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
