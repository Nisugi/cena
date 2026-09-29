//! The command input: Enter sends, and the keys' actions walk and repeat
//! what was typed (`plan/52` step 3). Moved out of `play/tests.rs` at its
//! cap.

use super::*;

/// Enter sends and clears; the history's actions walk back through what
/// was sent, and the sending ones send it again (`plan/52` step 3).
#[test]
fn enter_sends_and_the_history_walks_back() {
    use crate::keys::Action;
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
    let context = harness.ctx.clone();
    let mut walked = Vec::new();
    for action in [
        Action::HistoryBack,
        Action::HistoryBack,
        Action::HistoryForward,
        Action::HistoryForward,
    ] {
        assert_eq!(harness.state_mut().play.act(&context, action), None);
        walked.push(typed(&harness));
    }
    assert_eq!(walked, ["north", "look", "north", ""]);

    let play = &mut harness.state_mut().play;
    assert_eq!(
        play.act(&context, Action::RepeatLast).as_deref(),
        Some("north")
    );
    assert_eq!(
        play.act(&context, Action::RepeatSecondLast).as_deref(),
        Some("look")
    );
    assert_eq!(
        play.act(&context, Action::RepeatSecondLast).as_deref(),
        Some("look"),
        "a repeat is not typed, and changes no history"
    );
    assert_eq!(
        play.act(&context, Action::SendOrRepeat).as_deref(),
        Some("north"),
        "an empty line repeats"
    );
    play.fill("search");
    assert_eq!(
        play.act(&context, Action::SendOrRepeat).as_deref(),
        Some("search")
    );
    assert_eq!(
        play.act(&context, Action::RepeatSecondLast).as_deref(),
        Some("north"),
        "a line sent is typed"
    );
    play.fill("hide");
    assert_eq!(play.act(&context, Action::ClearInput), None);
    assert_eq!(play.input, "");
}

/// The input's own actions wait while another field has the keyboard, and
/// act while the input has it (`plan/52` step 3).
#[test]
fn another_field_with_the_keyboard_is_not_typing() {
    let mut harness = harness();
    harness.run();
    assert!(harness.state().play.typing(&harness.ctx));
    let other = egui::Id::new("another field");
    harness.ctx.memory_mut(|memory| memory.request_focus(other));
    assert!(!harness.state().play.typing(&harness.ctx));
}
