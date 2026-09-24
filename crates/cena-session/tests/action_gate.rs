//! An action's last check is the session's, against the live model, as its
//! bytes go out (`plan/30` §3). A refused action is not written.

use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::queue::any_frame;
use cena_session::{
    AuthorityToken, CommandId, Event, Gate, Origin, Outcome, Refusal, Session, SessionHandle, State,
};
use std::time::Duration;

const PROMPT: &[u8] = b"<prompt time=\"1000\">&gt;</prompt>\n";
const DEADLINE: Duration = Duration::from_secs(5);
const HUNT: AuthorityToken = AuthorityToken(3);
const KOBOLD: i64 = 123_456;

/// A session holding the authority, taught `setup` by a `look` first.
async fn a_session_that_saw(setup: &[u8]) -> (SessionHandle, TranscriptHandle) {
    let (source, transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let (_, mut events) = session.subscribe();
    let handle = session.handle();
    tokio::spawn(session.into_actor().run());
    while !matches!(events.recv().await, Ok(Event::StateChanged(State::Ready))) {}
    transcript.answer("look", setup);
    let _ = handle
        .send_and_await(CommandId(0), "look", Origin::Manual, DEADLINE, any_frame)
        .await;
    assert_eq!(handle.claim(HUNT).await, Ok(()));
    (handle, transcript)
}

async fn attack(handle: &SessionHandle, target: Option<i64>) -> Outcome {
    handle
        .send_gated(
            CommandId(1),
            "attack",
            Origin::Behavior(HUNT),
            DEADLINE,
            any_frame,
            Gate::Act { target },
        )
        .await
}

/// The kobold in the room. A room list registers a creature only when a
/// `<crtrStatus>` vouches for it, as the wire sends (`Creatures::apply_room_objs`).
fn room_with_the_kobold() -> Vec<u8> {
    room_with("hostile=\"1\"")
}

/// The same, dead.
fn room_with_a_dead_kobold() -> Vec<u8> {
    room_with("hostile=\"1\" dead=\"1\"")
}

fn room_with(status: &str) -> Vec<u8> {
    format!(
        "<component id='room objs'>You also see <pushBold/>a <a exist=\"{KOBOLD}\" \
         noun=\"kobold\">grimy kobold</a><popBold/>.</component>\n\
         <crtrStatus exist=\"{KOBOLD}\" {status}/>\n\
         <prompt time=\"1000\">&gt;</prompt>\n"
    )
    .into_bytes()
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_clear_character_with_its_target_present_attacks() {
    let (handle, transcript) = a_session_that_saw(&room_with_the_kobold()).await;
    let outcome = attack(&handle, Some(KOBOLD)).await;
    assert!(!matches!(outcome, Outcome::Refused(_)), "{outcome:?}");
    assert_eq!(transcript.lines(), ["look", "attack"]);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn each_condition_refuses_and_nothing_is_written() {
    let cases: [(&[u8], Option<i64>, Refusal); 5] = [
        (
            b"<roundTime value=\"1010\"/><prompt time=\"1000\">&gt;</prompt>\n",
            None,
            Refusal::Roundtime,
        ),
        (
            b"<castTime value=\"1010\"/><prompt time=\"1000\">&gt;</prompt>\n",
            None,
            Refusal::Casttime,
        ),
        (
            b"<indicator id=\"IconSTUNNED\" visible=\"y\"/><prompt time=\"1000\">&gt;</prompt>\n",
            None,
            Refusal::Stunned,
        ),
        (
            b"<indicator id=\"IconWEBBED\" visible=\"y\"/><prompt time=\"1000\">&gt;</prompt>\n",
            None,
            Refusal::Webbed,
        ),
        (PROMPT, Some(KOBOLD), Refusal::TargetGone),
    ];
    for (setup, target, refusal) in cases {
        let (handle, transcript) = a_session_that_saw(setup).await;
        assert_eq!(
            attack(&handle, target).await,
            Outcome::Refused(refusal),
            "{refusal:?}"
        );
        assert_eq!(transcript.lines(), ["look"], "{refusal:?}: nothing written");
    }
    let (handle, transcript) = a_session_that_saw(&room_with_a_dead_kobold()).await;
    assert_eq!(
        attack(&handle, Some(KOBOLD)).await,
        Outcome::Refused(Refusal::TargetGone),
        "a dead target is gone"
    );
    assert_eq!(transcript.lines(), ["look"]);
}

/// An ungated command is not checked: `look` in roundtime still goes out.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn an_ungated_command_goes_out_in_roundtime() {
    let (handle, transcript) =
        a_session_that_saw(b"<roundTime value=\"1010\"/><prompt time=\"1000\">&gt;</prompt>\n")
            .await;
    let _ = handle
        .send_and_await(
            CommandId(2),
            "look",
            Origin::Behavior(HUNT),
            DEADLINE,
            any_frame,
        )
        .await;
    assert_eq!(transcript.lines(), ["look", "look"]);
}
