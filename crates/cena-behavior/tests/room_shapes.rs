//! The room arrives in two shapes, and `look` gets the one the fixtures lacked.
//!
//! Lives in `cena-behavior`, not `cena-session`, because it asserts on the
//! BEHAVIOR's matcher -- and `cena-session` must not depend upward on
//! `cena-behavior` (`plan/12` §2).

use cena_platform::AnsweringSource;
use cena_session::{CommandId, Origin, Outcome, Session};
use std::time::Duration;

/// The game emits the room in **two different shapes**, and a behavior that
/// sends `look` gets the one the fixtures did not contain.
///
/// Author-supplied live traffic, 2026-09-18:
///
///   * `look` writes inline to the main stream, bracketed by
///     `<style id="roomDesc"/>`. **No `compDef`, no `component`.**
///   * movement emits `<compDef id='room desc'>` inside `<pushStream id='room'/>`,
///     which feeds the room window.
///
/// The matcher originally keyed only on `Component`, so a real `look` would
/// have matched nothing and returned `Timeout` forever -- while every
/// corpus-cut fixture passed, because those captures contained the window feed.
///
/// Goes RED on narrowing `is_room_description` back to `Component` alone
/// (verified): the `look` shape resolves `Timeout` instead of `Confirmed`.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn both_room_shapes_answer_a_look() {
    // Verbatim shapes from live traffic, trimmed to one sentence each.
    const LOOK_SHAPE: &[u8] = b"<style id=\"roomName\" />[Western Harbor, Docks] (3216012)\n\
<style id=\"\"/><style id=\"roomDesc\"/>Transitioning seamlessly from cobblestones to wooden planks.\n\
<prompt time=\"1789765767\">&gt;</prompt>\n";
    const MOVE_SHAPE: &[u8] =
        b"<compDef id='room desc'>Pilings rise on either side of the dock's platform.</compDef>\n\
<prompt time=\"1789765816\">&gt;</prompt>\n";

    for (label, wire) in [("look", LOOK_SHAPE), ("movement", MOVE_SHAPE)] {
        let (source, _transcript) = AnsweringSource::new(wire);
        let session = Session::new(source);
        let handle = session.handle();
        let cancel = session.cancel_token();
        let actor = tokio::spawn(session.into_actor().run());

        let outcome = handle
            .send_and_await(
                CommandId(1),
                "look",
                Origin::Manual,
                Duration::from_secs(5),
                cena_behavior::is_room_description,
            )
            .await;

        assert!(
            matches!(outcome, Outcome::Confirmed(_)),
            "the {label} room shape must answer a look. A matcher keyed only on \
             `Component` misses the look shape entirely, which is inline \
             `<style id=\"roomDesc\"/>` text on the main stream. Got: {outcome:?}"
        );

        cancel.cancel();
        let _ = actor.await;
    }
}
