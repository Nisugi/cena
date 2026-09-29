//! A behavior's command is published with the name its run was given, for
//! a frontend to echo as `go2>north`, as Lich echoes a script's commands
//! (the author, 2026-09-29). The player's command carries none, nor one of a
//! run nobody named, nor a quiet one, whose report is kept out of the story.

use std::time::Duration;

use cena_platform::AnsweringSource;
use cena_session::queue::AuthorityToken;
use cena_session::{CommandId, Event, Frame, Origin, Session, State};
use tokio::sync::broadcast::Receiver;

const PROMPT: &[u8] = b"<prompt time=\"1\">&gt;</prompt>\n";
const DEADLINE: Duration = Duration::from_secs(5);
const WALK: AuthorityToken = AuthorityToken(2);

async fn ready(events: &mut Receiver<Event>) {
    while let Ok(event) = events.recv().await {
        if event == Event::StateChanged(State::Ready) {
            return;
        }
    }
}

/// The name each command after this was published with, until `count`.
async fn names(events: &mut Receiver<Event>, count: usize) -> Vec<(String, Option<String>)> {
    let mut named = Vec::new();
    while named.len() < count {
        match events.recv().await {
            Ok(Event::Sent { line, by, .. }) => named.push((line, by)),
            Ok(_) => {}
            Err(_) => break,
        }
    }
    named
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_named_runs_command_carries_its_name() {
    let (source, _transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let (_, mut events) = session.subscribe();
    tokio::spawn(session.into_actor().run());
    ready(&mut events).await;
    let prompt = |frame: &Frame| matches!(frame, Frame::Prompt { .. });

    handle.claim(WALK).await.expect("the authority");
    let walking = Origin::Behavior(WALK);
    let _ = handle
        .send_and_await(CommandId(1), "north", walking, DEADLINE, prompt)
        .await;
    handle.name_behavior(WALK, "go2");
    let _ = handle
        .send_and_await(CommandId(2), "south", walking, DEADLINE, prompt)
        .await;
    let _ = handle
        .send_quietly(CommandId(3), "info", walking, DEADLINE, prompt)
        .await;
    let _ = handle
        .send_and_await(CommandId(4), "look", Origin::Manual, DEADLINE, prompt)
        .await;
    assert_eq!(
        names(&mut events, 4).await,
        [
            ("north".to_owned(), None),
            ("south".to_owned(), Some("go2".to_owned())),
            ("info".to_owned(), None),
            ("look".to_owned(), None),
        ],
        "unnamed, named, quiet, the player's"
    );
}
