//! What is in each hand, and the id a command would target it by.

use cena_model::GameState;
use cena_protocol::Parser;

fn state_after(lines: &[&str]) -> GameState {
    let mut parser = Parser::new();
    let mut state = GameState::default();
    for line in lines {
        for frame in parser.parse_line(line) {
            state.apply(&frame);
        }
    }
    state
}

#[test]
fn a_held_item_keeps_the_id_the_wire_sent() {
    // **THE DEFECT THIS TYPE FIXES.** `GameState::apply` matched
    // `Frame::LeftHand { item, .. }` and discarded the link, so the `exist`
    // id reached no consumer -- Rule 2.2a. The wire has been sending it all
    // along: MEASURED 2,806 `<right>Empty` and thousands of identified rows
    // across the 208 live Lich XML logs.
    //
    // It matters because an id is what a command targets and a name is not.
    // Two `glowbark long bow`s are different items.
    let state = state_after(&[r#"<left exist="157925365" noun="bow">glowbark long bow</left>"#]);
    assert_eq!(state.left_hand.id(), Some("157925365"));
    assert_eq!(state.left_hand.noun(), Some("bow"));
    assert_eq!(state.left_hand.name(), Some("glowbark long bow"));
    assert!(state.left_hand.holds("157925365"));
    assert!(!state.left_hand.holds("999"), "a different item");
}

#[test]
fn an_empty_hand_is_empty_and_not_an_item_called_empty() {
    // The tag is always present, so emptiness arrives as the literal word
    // `Empty` with no `exist=`. Reading it as an item would give a behavior
    // something to `get`.
    let state = state_after(&["<right>Empty</right>"]);
    assert!(state.right_hand.is_empty());
    assert!(!state.right_hand.is_holding());
    assert_eq!(state.right_hand.id(), None);
    assert_eq!(state.right_hand.name(), None, "`Empty` is not a name");
}

#[test]
fn an_unreported_hand_is_unknown_not_empty() {
    // §5.2: absent is not zero. Before the game says anything, "is my right
    // hand free?" has no answer -- and answering `true` would have a behavior
    // try to put something in a hand that is already full.
    let hand = GameState::default().right_hand;
    assert!(!hand.is_known());
    assert!(!hand.is_empty(), "nobody has said it is empty");
    assert!(!hand.is_holding(), "nor that it holds anything");
}

#[test]
fn emptiness_and_silence_are_told_apart() {
    // The distinction the enum exists for, asserted directly: both are
    // "not holding", and only one is an answer.
    let said = state_after(&["<left>Empty</left>"]).left_hand;
    let unsaid = GameState::default().left_hand;
    assert!(!said.is_holding() && !unsaid.is_holding(), "neither holds");
    assert!(said.is_known(), "but one of them was stated");
    assert!(!unsaid.is_known());
}

#[test]
fn an_item_without_a_link_is_still_held() {
    // The wire is not obliged to send `exist=`, and an item with no id is
    // still an item worth knowing about -- dropping it would be the same
    // class of loss this type was written to fix.
    let state = state_after(&["<left>some unidentified thing</left>"]);
    assert!(state.left_hand.is_holding());
    assert_eq!(state.left_hand.name(), Some("some unidentified thing"));
    assert_eq!(state.left_hand.id(), None, "none was sent");
}

#[test]
fn a_hand_is_replaced_not_merged() {
    let state = state_after(&[
        r#"<left exist="111" noun="bow">a bow</left>"#,
        r#"<left exist="222" noun="sword">a sword</left>"#,
    ]);
    assert_eq!(state.left_hand.id(), Some("222"));
    assert!(!state.left_hand.holds("111"), "the bow is gone");
}

#[test]
fn putting_a_held_item_down_empties_the_hand() {
    let state = state_after(&[
        r#"<left exist="111" noun="bow">a bow</left>"#,
        "<left>Empty</left>",
    ]);
    assert!(state.left_hand.is_empty());
    assert!(!state.left_hand.holds("111"));
}
