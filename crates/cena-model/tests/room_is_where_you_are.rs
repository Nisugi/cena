//! `GameState.room` tracks WHERE THE CHARACTER IS, not what they last read.
//!
//! The game emits a room in two shapes and they are not interchangeable
//! (author, 2026-09-18, from live traffic):
//!
//!   * `<compDef id='room desc'>` feeds the room window and is truth for
//!     location -- and for the creatures, objects and players in the room.
//!   * inline text styled `<style id="roomDesc"/>` feeds the story window and
//!     is only what was seen.
//!
//! Abilities that look into another room emit the story form **without** the
//! window form, which is how a client knows the character did not move. This
//! file exists so that asymmetry is not "fixed" into a phantom relocation.

use cena_model::GameState;
use cena_protocol::Parser;

/// Fold every frame a wire snippet produces, and report the room description.
fn describe(wire: &str) -> Option<String> {
    let mut parser = Parser::new();
    let mut state = GameState::default();
    for line in wire.lines() {
        for frame in parser.parse_line(line) {
            state.apply(&frame);
        }
    }
    state
        .room
        .description
        .as_ref()
        .map(cena_protocol::runs::Runs::plain)
}

#[test]
fn the_window_feed_sets_the_room_and_the_story_feed_does_not() {
    // Movement: the window feed arrives, so this IS where the character is.
    let moved = describe(
        "<compDef id='room desc'>Pilings rise on either side of the dock's platform.</compDef>",
    );
    assert_eq!(
        moved.as_deref(),
        Some("Pilings rise on either side of the dock's platform."),
        "a `compDef` room description is the room window's feed and must set \
         the current room"
    );

    // A scry, or a plain `look`: story-window text only, no window feed. The
    // room must NOT change -- the character did not go anywhere.
    let scried = describe(
        "<style id=\"roomName\" />[Western Harbor, Docks] (3216012)\n\
         <style id=\"\"/><style id=\"roomDesc\"/>Transitioning seamlessly from cobblestones to wooden planks.",
    );
    assert_eq!(
        scried, None,
        "styled `roomDesc` text is the STORY window -- what was seen, not where \
         the character is. Abilities that look into another room emit exactly \
         this and no `compDef`, so folding it here would report a move that \
         never happened."
    );
}
