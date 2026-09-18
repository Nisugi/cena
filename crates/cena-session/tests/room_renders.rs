//! Criteria 2 and 8: a room renders from **typed frames**, and an unknown tag
//! survives to display.
//!
//! `plan/12:457` (criterion 2): "Renders a room description and prompt from
//! **typed frames**, never raw text."
//! `plan/12:466` (criterion 8): "Unknown tags survive to display
//! (`Frame::UnknownTag`) rather than panicking."
//!
//! # What "never raw text" is asserted to mean
//!
//! The failure criterion 2 rules out is a consumer that scans display prose
//! for `"Obvious exits:"` and splits it on commas. So the assertions below are
//! deliberately on the *typed* carriers, not on text that happens to contain
//! the right words:
//!
//! - the room id comes from `Frame::RoomId`, which the wire states as
//!   `<nav rm='7503251'/>` and which is **not recoverable from prose at all**;
//! - the exits come from `Frame::Compass`'s direction tokens `["e", "out"]`,
//!   which are not the words the prose uses (it says "east", the compass says
//!   `"e"`) -- so an implementation that scraped the text could not produce
//!   this answer.
//!
//! That second point is what makes the test falsifiable in the right
//! direction: `["e", "out"]` is evidence the value came from the compass and
//! not from the sentence.

mod support;

use cena_platform::ReplaySource;
use cena_session::Session;

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_room_renders_from_typed_frames_not_from_text() {
    let end = Session::new(ReplaySource::from_bytes(
        &support::room_fixture().expect("room fixture"),
    ))
    .into_actor()
    .run()
    .await;
    let state = end.state;

    // The room id: from `<nav rm=>`, which no amount of reading prose yields.
    assert_eq!(
        state.room.id.as_deref(),
        Some("7503251"),
        "the room id must come from Frame::RoomId. It is not present in any \
         display text, so a consumer that scraped prose could not produce it."
    );

    // The exits: direction TOKENS from `<compass><dir value=>`, not the words
    // the sentence uses. The prose says "east"; the compass says "e".
    assert_eq!(
        state.room.exits,
        vec!["e".to_owned(), "out".to_owned()],
        "exits must come from Frame::Compass. The display text says \
         'Obvious exits: east, out' -- if this were scraped from prose it \
         would read [\"east\", \"out\"], so the token form is the evidence that \
         it was not."
    );

    // The description: a parsed `Runs` body from the room-desc component, not
    // the inner XML string Vellum keeps (src/parser.rs:803-832).
    let description = state
        .room
        .description
        .as_ref()
        .expect("the room description must be present");
    let plain = description.plain();
    assert!(
        plain.starts_with("Low eaves, stained black with smoke"),
        "the description must be the parsed component body; got {plain:?}"
    );
    assert!(
        !plain.contains('<'),
        "the description body must be PARSED, not raw markup. Vellum stores \
         the inner XML verbatim here (reference/VellumFE/src/parser.rs:803-832) \
         and makes the layer above re-parse it, which is the Rule 2.1 \
         violation cena-protocol's Runs type exists to fix. Got: {plain:?}"
    );

    // The prompt: typed, with its text.
    assert_eq!(
        state.prompt.as_deref(),
        Some(">"),
        "the prompt must come from Frame::Prompt"
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn an_unknown_tag_survives_into_the_state_a_display_reads() {
    // Synthetic, because the corpus by definition does not contain a tag the
    // parser has never met. `<flooberty>` is not in KNOWN_WIRE_TAGS and is not
    // going to be.
    let wire = b"<flooberty zorp='3'/>after the unknown tag\n<prompt time=\"1\">&gt;</prompt>\n";
    let state = Session::new(ReplaySource::from_bytes(wire))
        .into_actor()
        .run()
        .await
        .state;

    // Criterion 8 is "survive to DISPLAY", not "do not panic". Asserting only
    // that the test finished would pass on an implementation that swallowed
    // the tag entirely, which is exactly what Rule 2.2 (plan/05:276-283)
    // forbids: an unmodelled tag must reach the user as text and a log.
    assert_eq!(
        state.unknown_tags.len(),
        1,
        "the unknown tag must reach the state a display reads, not merely fail \
         to crash. Got: {:?}",
        state.unknown_tags
    );
    assert_eq!(state.unknown_tags[0].name, "flooberty");
    assert!(
        state.unknown_tags[0].raw.contains("zorp='3'"),
        "the raw bytes ARE the diagnostic (Rule 2.2): a reader has to see what \
         the game actually sent, attributes included. Got: {:?}",
        state.unknown_tags[0].raw
    );

    // And the text after it still arrived: an unknown tag must not eat the
    // rest of the line.
    assert_eq!(state.prompt.as_deref(), Some(">"));
}
