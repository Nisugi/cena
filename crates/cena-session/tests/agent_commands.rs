//! An agent's game command, from its admission to its write: a stop or a
//! lowered level takes it back until the session writes it, and it goes on
//! the connection it was allowed on or not at all (the integrated crate
//! review of 2026-09-28, I1 and I2). Once written, nothing takes it back,
//! and the stop says so.

use std::time::Duration;

use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::agent::{Admitted, Call, Denied, Door, Level};
use cena_session::operation::{Control, Ended, Lifecycle, Report, Work};
use cena_session::{Event, Generation, Session, SessionHandle, State};

/// A session against a scripted game, `Ready`, the agent at `level`.
async fn ready(level: Level) -> Option<(SessionHandle, TranscriptHandle)> {
    let (source, transcript) = AnsweringSource::logged_in(b"<prompt time=\"1\">&gt;</prompt>\n");
    let session = Session::new(source);
    let handle = session.handle();
    let (_, mut events) = session.subscribe();
    tokio::spawn(session.into_actor().run());
    while !matches!(events.recv().await.ok()?, Event::StateChanged(State::Ready)) {}
    handle.set_agent_level(level);
    Some((handle, transcript))
}

/// The agent's `command`, on the connection it saw.
fn command(door: &Door, line: &str, request: &str, seen: Generation) -> Option<Report> {
    let call = Call {
        request,
        generation: Some(seen),
    };
    match door.command(line, "the test", call) {
        Ok(Admitted::Operation(report)) => Some(report),
        _ => None,
    }
}

/// How operation `id` ended.
async fn ended(door: &Door, id: u64) -> Option<Ended> {
    for _ in 0..400 {
        match door.operation(id) {
            Some(report) if report.lifecycle == Lifecycle::Ended => return report.ended,
            _ => tokio::time::sleep(Duration::from_millis(5)).await,
        }
    }
    None
}

fn wrote(transcript: &TranscriptHandle, line: &str) -> bool {
    transcript.lines().iter().any(|written| written == line)
}

/// The review's probe: `look` admitted and not yet written; the player's
/// stop and the level set to Off, with nothing yielding between. The stop
/// finds it and stops it, and the game never gets it.
#[tokio::test(flavor = "current_thread")]
async fn a_stop_before_the_write_takes_an_agents_line_back() {
    let (handle, transcript) = ready(Level::Commands).await.unwrap();
    let door = handle.agent_door();
    let report = command(&door, "look", "c1", handle.generation()).unwrap();
    assert!(!wrote(&transcript, "look"));

    assert_eq!(handle.stop_agent(), (1, false), "the stop finds it");
    handle.set_agent_level(Level::Off);

    let ended = ended(&door, report.id).await.unwrap();
    assert_eq!(
        (ended.work, ended.reason.as_str()),
        (Work::Interrupted, "stopped")
    );
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!wrote(&transcript, "look"), "{:?}", transcript.lines());
}

/// A line queued behind another still in its window, then the level
/// lowered: taken back, and not written once the window closes.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_lowered_level_takes_back_a_line_waiting_its_turn() {
    let (handle, transcript) = ready(Level::Commands).await.unwrap();
    let door = handle.agent_door();
    transcript.hold_replies();
    let typed = {
        let handle = handle.clone();
        let at = handle.generation();
        tokio::spawn(async move {
            handle
                .send_manual_at(at, "ponder", Duration::from_secs(5))
                .await
        })
    };
    while !wrote(&transcript, "ponder") {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let report = command(&door, "look", "c1", handle.generation()).unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!wrote(&transcript, "look"), "waiting behind `ponder`");

    handle.set_agent_level(Level::Observe);
    let ended = ended(&door, report.id).await.unwrap();
    assert_eq!(
        (ended.work, ended.reason.as_str()),
        (Work::Interrupted, "level_lowered")
    );
    transcript.release_replies();
    let _ = typed.await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!wrote(&transcript, "look"), "{:?}", transcript.lines());
}

/// Written, it is the game's: a stop is refused saying so, and the
/// player's stop has nothing to stop.
#[tokio::test(flavor = "current_thread")]
async fn a_line_already_written_is_not_stopped_and_says_so() {
    let (handle, transcript) = ready(Level::Commands).await.unwrap();
    let door = handle.agent_door();
    transcript.answer("ponder", b"You ponder.\n<prompt time=\"2\">&gt;</prompt>\n");
    transcript.hold_replies();
    let report = command(&door, "ponder", "c1", handle.generation()).unwrap();
    while !wrote(&transcript, "ponder") {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let call = Call {
        request: "s1",
        generation: Some(handle.generation()),
    };
    let refused = door.control(report.id, Control::Stop, "the test", call);
    assert!(
        matches!(&refused, Err(Denied::Invalid(why)) if why.contains("already the game's")),
        "{refused:?}"
    );
    assert_eq!(handle.stop_agent(), (0, false));
    transcript.release_replies();
    let ended = ended(&door, report.id).await.unwrap();
    assert_eq!(ended.work, Work::Completed, "{ended:?}");
}
