//! What a `<prompt>` must be, for the consumer that treats it as a boundary.
//!
//! `dispatch.rs`'s `prompt()` is the round boundary: every prompt force-closes
//! whatever streams were left open, which is what makes the measured
//! `pushStream`:`popStream` imbalance of 1.301 harmless and bounds state
//! corruption to one round.
//!
//! Two properties make that work, and neither was asserted. Both were found by
//! review (PR-12 and PR-9) and both are pinned here, in a new file because
//! `fixed_defects.rs` is at 379 of its 400 lines and `plan/05` Rule 4.1 says
//! move code down rather than raise the cap.

use cena_protocol::Parser;
use cena_protocol::frame::Frame;

/// The forced pops arrive BEFORE the prompt that forces them.
///
/// A consumer snapshotting on `Frame::Prompt` -- the documented round boundary
/// -- otherwise snapshots with streams still open and sees them close *after*
/// the boundary they define. Vellum emits them first
/// (`reference/VellumFE/src/parser/handlers.rs:306-314,328`); this crate
/// emitted them after, with nothing recording the divergence as deliberate.
#[test]
fn forced_stream_pops_precede_the_prompt_that_forces_them() {
    let mut parser = Parser::new();
    parser.parse_line("<pushStream id='room'/>You have entered a room.");
    let frames = parser.parse_line("<prompt time='1789775821'>&gt;</prompt>");

    let prompt_at = frames
        .iter()
        .position(|f| matches!(f, Frame::Prompt { .. }));
    let pop_at = frames
        .iter()
        .position(|f| matches!(f, Frame::StreamPopForced { .. }));

    let Some(prompt_at) = prompt_at else {
        panic!("the prompt must be emitted: {frames:?}")
    };
    let Some(pop_at) = pop_at else {
        panic!("the open `room` stream must be force-closed: {frames:?}")
    };
    assert!(
        pop_at < prompt_at,
        "the forced pop must come BEFORE the prompt. A consumer that \
         snapshots on `Prompt` -- the round boundary -- would otherwise \
         snapshot with `room` still open and see it close after the boundary \
         that closed it. Frames were: {frames:?}"
    );
}

/// The prompt's text is stripped of control characters, like every other text
/// this parser produces.
///
/// # Why this is a security property and not a tidiness one
///
/// The game socket is **plain TCP**. `run.rs` prints the prompt straight to
/// stderr with `eprintln!("{text}")`. Entities are decoded inside `prompt()`,
/// so `&#27;` becomes a real ESC *here* -- it does not exist as a control
/// character anywhere upstream for something else to have caught.
///
/// `&#27;]52;c;<base64>&#7;` is OSC-52: a clipboard write into whoever is
/// running the client. The three other text paths (`emit.rs:48`,
/// `emit.rs:135`, `inner.rs:63`) all pair `decode_entities` with
/// `strip_control_chars`. This one did not.
#[test]
fn the_prompt_text_cannot_carry_a_terminal_escape() {
    let mut parser = Parser::new();
    // OSC-52 as the wire would have to encode it to survive XML.
    let frames = parser.parse_line("<prompt time='1'>&#27;]52;c;QUFB&#7;&gt;</prompt>");

    let text = frames
        .iter()
        .find_map(|f| match f {
            Frame::Prompt { text, .. } => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("the prompt must be emitted: {frames:?}"));

    assert!(
        !text.contains('\u{1b}'),
        "ESC survived into the prompt text, which `run.rs` prints to the \
         terminal unescaped. `&#27;]52;c;...&#7;` is an OSC-52 clipboard \
         write, and the socket carrying it is plain TCP. Got: {text:?}"
    );
    assert!(
        !text.contains('\u{7}'),
        "BEL survived into the prompt text: {text:?}"
    );
    assert!(
        text.contains('>'),
        "the ordinary prompt character must still be there -- stripping \
         control characters must not eat the content: {text:?}"
    );
}

/// Stripping does not disturb the ordinary case.
///
/// The guard above could be satisfied by a prompt that lost its text
/// entirely, so the common shape is pinned beside it.
#[test]
fn an_ordinary_prompt_is_unchanged() {
    let mut parser = Parser::new();
    let frames = parser.parse_line("<prompt time='1789775821'>&gt;</prompt>");
    assert!(
        frames.iter().any(|f| matches!(
            f,
            Frame::Prompt { time, text } if time == "1789775821" && text == ">"
        )),
        "the live shape from the 2026-09-18 capture must round-trip: {frames:?}"
    );
}
