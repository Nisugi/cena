//! A quiet command's report stays out of the story; the next command's does
//! not (`plan/30` §2, the character sync).
//!
//! End to end, native session to browser, because the property spans both:
//! the session brackets the window with `Event::Quiet`, and the page's story
//! is what leaves the report out. Either half broken alone turns this red.

mod web_support;

use cena_platform::AnsweringSource;
use cena_session::{CommandId, Generation, Origin, Outcome, Session, queue::any_frame};
use cena_ui::{ServerMessage, StoryLine};
use cena_web::WebServer;
use tokio_util::sync::CancellationToken;
use web_support::*;

/// Every command gets the same answer: a main-stream line, which is the
/// report, and a thought, which is not the command's and always shows.
const REPLY: &[u8] = b"Your report, line one.\n\
<pushStream id=\"thoughts\"/>You hear a stray thought.\n<popStream/>\
<prompt time=\"2\">&gt;</prompt>\n";

fn text(line: &StoryLine) -> String {
    line.runs.iter().map(|run| run.text.as_str()).collect()
}

#[tokio::test]
async fn a_quiet_report_is_left_out_of_the_story_and_the_next_one_is_not() {
    let (source, _transcript) = AnsweringSource::logged_in(REPLY);
    let session = Session::new(source);
    let handle = session.handle();
    let observer = session.observer();
    let stop_session = session.cancel_token();
    let actor = tokio::spawn(session.into_actor().run());
    await_ready(&observer, Generation::FIRST).await.unwrap();

    let server = WebServer::bind(observer.clone(), handle.clone())
        .await
        .unwrap();
    let pairing = server.pairing_url();
    let stop_web = CancellationToken::new();
    let web = tokio::spawn(server.run(stop_web.clone().cancelled_owned()));
    let mut socket = browser(&pairing).await.unwrap();
    assert!(matches!(
        receive(&mut socket).await.unwrap(),
        ServerMessage::Snapshot { .. }
    ));

    let quiet = handle
        .send_quietly(
            CommandId(1),
            "info full",
            Origin::Manual,
            DEADLINE,
            any_frame,
        )
        .await;
    assert!(matches!(quiet, Outcome::Confirmed(_)), "{quiet:?}");
    let loud = handle
        .send_and_await(CommandId(2), "look", Origin::Manual, DEADLINE, any_frame)
        .await;
    assert!(matches!(loud, Outcome::Confirmed(_)), "{loud:?}");

    // Both thoughts arriving means both replies have been presented.
    let mut lines = Vec::new();
    tokio::time::timeout(DEADLINE, async {
        while lines
            .iter()
            .filter(|line| text(line) == "You hear a stray thought.")
            .count()
            < 2
        {
            if let ServerMessage::Update { lines: new, .. } = receive(&mut socket).await.unwrap() {
                lines.extend(new);
            }
        }
    })
    .await
    .expect("both replies reached the page");

    let reports = lines
        .iter()
        .filter(|line| text(line) == "Your report, line one.")
        .count();
    assert_eq!(
        reports,
        1,
        "the quiet command's report must be hidden and the next command's shown: {:?}",
        lines.iter().map(text).collect::<Vec<_>>()
    );

    stop_web.cancel();
    stop_session.cancel();
    let _ = web.await;
    let _ = actor.await;
}
