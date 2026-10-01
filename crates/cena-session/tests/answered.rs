//! A line that names its answer is answered by it, not by the first prompt
//! (`command/answer.rs`; `plan/12` §4.4 as amended 2026-10-01).
//!
//! The author's hunt of 2026-09-30: a creature leaving between `fire` and
//! its reply drew a prompt, and that prompt ended the round trip.

use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::queue::any_frame;
use cena_session::{
    AuthorityToken, CommandId, Event, Gate, Origin, Outcome, Session, SessionHandle, State,
};
use std::time::Duration;

const PROMPT: &[u8] = b"<prompt time=\"1000\">&gt;</prompt>\n";
const DEADLINE: Duration = Duration::from_secs(20);
const HUNT: AuthorityToken = AuthorityToken(3);

const LEAVING: &[u8] = b"A behemothic gorefrost golem just went through a rune-carved white granite arch.\n<prompt time=\"1001\">&gt;</prompt>\n";
const REPLY: &[u8] = b"You take aim and fire a faewood arrow at a grim gigas skald!\nRoundtime: 4 sec.\n<prompt time=\"1002\">&gt;</prompt>\n";

fn roundtime(line: &str) -> bool {
    line.starts_with("Roundtime: ")
}

async fn hunting() -> (SessionHandle, TranscriptHandle) {
    let (source, transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let (_, mut events) = session.subscribe();
    let handle = session.handle();
    tokio::spawn(session.into_actor().run());
    while !matches!(events.recv().await, Ok(Event::StateChanged(State::Ready))) {}
    let _ = handle
        .send_and_await(CommandId(0), "look", Origin::Manual, DEADLINE, any_frame)
        .await;
    assert_eq!(handle.claim(HUNT).await, Ok(()));
    (handle, transcript)
}

fn fire(handle: &SessionHandle, id: u64) -> tokio::task::JoinHandle<Outcome> {
    let handle = handle.clone();
    tokio::spawn(async move {
        handle
            .send_answered(
                CommandId(id),
                "fire",
                Origin::Behavior(HUNT),
                DEADLINE,
                roundtime,
                Gate::None,
            )
            .await
    })
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_creature_leaving_does_not_answer_fire() {
    let (handle, transcript) = hunting().await;
    transcript.answer("fire", LEAVING);
    let fired = fire(&handle, 1);
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        !fired.is_finished(),
        "the golem's prompt ended the round trip"
    );
    transcript.say(REPLY);
    let outcome = fired.await.expect("the round trip");
    assert_eq!(outcome, Outcome::Answered("Roundtime: 4 sec.".to_owned()));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn another_players_roundtime_does_not_answer_fire() {
    let (handle, transcript) = hunting().await;
    transcript.answer(
        "fire",
        b"<preset id='speech'>Bob says</preset>, \"Roundtime: 4 sec.\"\n<prompt time=\"1001\">&gt;</prompt>\n",
    );
    let fired = fire(&handle, 1);
    let outcome = fired.await.expect("the round trip");
    assert_eq!(outcome, Outcome::Timeout);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_late_answer_is_its_own_lines_and_not_the_next() {
    let (handle, transcript) = hunting().await;
    transcript.answer("fire", LEAVING);
    let first = fire(&handle, 1).await.expect("the first round trip");
    assert_eq!(first, Outcome::Timeout);
    transcript.answer("fire", LEAVING);
    let second = fire(&handle, 2);
    tokio::time::sleep(Duration::from_millis(200)).await;
    // The first line's reply, late.
    transcript.say(REPLY);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(
        !second.is_finished(),
        "the first line's late answer was taken as the second's"
    );
    transcript.say(b"Roundtime: 3 sec.\n<prompt time=\"1003\">&gt;</prompt>\n");
    let outcome = second.await.expect("the second round trip");
    assert_eq!(outcome, Outcome::Answered("Roundtime: 3 sec.".to_owned()));
    assert_eq!(transcript.lines(), ["look", "fire", "fire"]);
}

/// The replay (`plan/06` §1.4): the author's hunt of 2026-09-30, as the
/// game sent it after `fire #583850503`
/// (`cena-behavior/tests/fixtures/fire_golem.xml`). The golem leaving and
/// its prompt came first; Hydra sent `fire` twice more before the reply,
/// forty lines on. Here the golem's part answers the `fire`, the rest comes
/// later, and the round trip ends at the reply's own roundtime.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_authors_fire_waits_past_the_golem_for_its_roundtime() {
    let wire = include_str!("../../cena-behavior/tests/fixtures/fire_golem.xml");
    let golem = wire
        .find("just went through")
        .and_then(|at| wire[at..].find("</prompt>\n").map(|end| at + end + 10))
        .expect("the golem's prompt is in the fixture");
    let (before, after) = wire.split_at(golem);
    let (handle, transcript) = hunting().await;
    transcript.answer("fire #583850503", before.as_bytes());
    let handle_fired = handle.clone();
    let fired = tokio::spawn(async move {
        handle_fired
            .send_answered(
                CommandId(1),
                "fire #583850503",
                Origin::Behavior(HUNT),
                DEADLINE,
                roundtime,
                Gate::None,
            )
            .await
    });
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        !fired.is_finished(),
        "the golem's prompt ended the round trip, as on 2026-09-30"
    );
    transcript.say(after.as_bytes());
    let outcome = fired.await.expect("the round trip");
    assert_eq!(outcome, Outcome::Answered("Roundtime: 4 sec.".to_owned()));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_refusal_answers_any_line() {
    let (handle, transcript) = hunting().await;
    transcript.answer(
        "fire",
        b"...wait 2 seconds.\n<prompt time=\"1001\">&gt;</prompt>\n",
    );
    let outcome = fire(&handle, 1).await.expect("the round trip");
    assert_eq!(outcome, Outcome::Answered("...wait 2 seconds.".to_owned()));
}
