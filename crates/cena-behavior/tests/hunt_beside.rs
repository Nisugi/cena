//! A cast goes beside what the hunt desk runs, never in its place (the
//! author, 2026-09-29: *";sc should run all the time if it's enabled"*; the
//! review of the same day: a typed `401` cancelled the running hunt).

mod ready;

use std::sync::Arc;
use std::time::Duration;

use cena_behavior::hunt::drive::HuntEnd;
use cena_behavior::hunt::{Command, Desk};
use cena_platform::AnsweringSource;
use cena_session::{AuthorityToken, Session};

const PROMPT: &[u8] = b"<prompt time=\"1\">&gt;</prompt>\n";

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_cast_while_something_runs_goes_beside_it() {
    let (source, transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let observer = session.observer();
    let (_, ready) = session.subscribe();
    tokio::spawn(session.into_actor().run());
    ready::until_ready(ready).await.unwrap();
    let dir = std::env::temp_dir().join(format!("cena-hunt-beside-{}", std::process::id()));
    let desk = Desk::new(
        Arc::new(cena_map::Map::from_rooms(Vec::new()).unwrap()),
        dir,
        AuthorityToken(1),
    );

    // The first cast runs on the desk, and waits on the game's answer.
    transcript.hold_replies();
    let joined = observer.subscribe().await.unwrap();
    let first = desk
        .run(&handle, joined, Command::Sc(vec!["401".to_owned()]))
        .expect("the first cast runs");
    while !transcript.lines().iter().any(|l| l.contains("401")) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // A second, while the first runs: beside it, not in its place.
    let joined = observer.subscribe().await.unwrap();
    let second = desk
        .run(&handle, joined, Command::Sc(vec!["406".to_owned()]))
        .expect("the second cast runs");
    transcript.release_replies();
    let (first, second) = (first.await.unwrap(), second.await.unwrap());
    assert!(
        matches!(first, HuntEnd::Finished(_)),
        "what ran was left alone, not stopped: {first:?}"
    );
    assert!(matches!(second, HuntEnd::Finished(_)), "{second:?}");
    let sent = transcript.lines();
    assert!(sent.iter().any(|l| l.contains("401")), "{sent:?}");
    assert!(sent.iter().any(|l| l.contains("406")), "{sent:?}");
}
