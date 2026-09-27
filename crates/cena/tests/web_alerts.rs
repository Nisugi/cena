//! M8 Stage 3's banner (`plan/45`): a trigger's `alert` reaches its own
//! character's page as an `alert` message, after the line that called it,
//! and not the other character's page. Nothing here is live: the game is an
//! `AnsweringSource`.

mod web_support;

use std::time::Duration;

use cena_behavior::triggers;
use cena_platform::AnsweringSource;
use cena_session::trigger::Matcher;
use cena_session::{Generation, Outcome, Session, SessionHandle, SessionId};
use cena_ui::{ServerMessage, StoryLine};
use cena_web::WebServer;
use tokio_util::sync::CancellationToken;
use web_support::*;

/// Nisugi alone is told of a whisper, in the whisperer's name.
const FILE: &str = r#"
[trigger.whisper]
regex = '^(\w+) whispers'
alert = "$1 whispered"
characters = ["Nisugi"]
"#;

const LOOK: &[u8] = b"Dicate whispers, \"meet me at the gate\"\n\
<prompt time=\"1\">&gt;</prompt>\n";

fn text(line: &StoryLine) -> String {
    line.runs.iter().map(|run| run.text.as_str()).collect()
}

fn give(handle: &SessionHandle, character: &str) -> TestResult {
    let loaded = triggers::read(FILE)?;
    handle.set_triggers(Matcher::new(loaded.triggers.for_character(character))?);
    Ok(())
}

/// What `page` is sent until the whisper's line, and for a moment after: the
/// banners, in order, and whether the line came first.
async fn banners(page: &mut Browser) -> TestResult<(Vec<String>, bool)> {
    let mut seen_line = false;
    let mut line_first = true;
    let mut alerts = Vec::new();
    tokio::time::timeout(DEADLINE, async {
        while !seen_line {
            match receive(page).await? {
                ServerMessage::Update { lines, .. }
                | ServerMessage::Snapshot { story: lines, .. } => {
                    seen_line |= lines.iter().any(|line| text(line).contains("whispers"));
                }
                ServerMessage::Alert { text, .. } => {
                    line_first = false;
                    alerts.push(text);
                }
                _ => {}
            }
        }
        TestResult::Ok(())
    })
    .await??;
    // The banner follows the update that carried its line.
    while let Ok(message) = tokio::time::timeout(Duration::from_millis(500), receive(page)).await {
        if let ServerMessage::Alert { text, .. } = message? {
            alerts.push(text);
        }
    }
    Ok((alerts, line_first))
}

#[tokio::test]
async fn a_banner_reaches_its_own_characters_page_after_its_line() {
    let (a_source, a_transcript) = AnsweringSource::logged_in(ROOM);
    let (b_source, b_transcript) = AnsweringSource::logged_in(ROOM);
    a_transcript.answer("look", LOOK);
    b_transcript.answer("look", LOOK);
    let a = Session::numbered(SessionId(0), a_source);
    let b = Session::numbered(SessionId(1), b_source);
    let (a_handle, b_handle) = (a.handle(), b.handle());
    let (a_observer, b_observer) = (a.observer(), b.observer());
    let (a_stop, b_stop) = (a.cancel_token(), b.cancel_token());
    give(&a_handle, "Nisugi").unwrap();
    give(&b_handle, "Dicate").unwrap();
    let a_actor = tokio::spawn(a.into_actor().run());
    let b_actor = tokio::spawn(b.into_actor().run());
    await_ready(&a_observer, Generation::FIRST).await.unwrap();
    await_ready(&b_observer, Generation::FIRST).await.unwrap();

    let server = WebServer::open().await.unwrap();
    let sessions = server.sessions();
    sessions.attach("Nisugi", a_observer, a_handle.clone());
    sessions.attach("Dicate", b_observer, b_handle.clone());
    let pairing = server.pairing_url();
    let stop_web = CancellationToken::new();
    let web = tokio::spawn(server.run(stop_web.clone().cancelled_owned()));
    let mut nisugi = browser_for(&pairing, Some("0")).await.unwrap();
    let mut dicate = browser_for(&pairing, Some("1")).await.unwrap();
    for page in [&mut nisugi, &mut dicate] {
        assert!(matches!(
            receive(page).await.unwrap(),
            ServerMessage::Snapshot { .. }
        ));
    }
    for handle in [&a_handle, &b_handle] {
        let sent = handle
            .send_manual_at(Generation::FIRST, "look", DEADLINE)
            .await;
        assert!(matches!(sent, Outcome::Confirmed(_)), "{sent:?}");
    }

    let (told, line_first) = banners(&mut nisugi).await.unwrap();
    assert_eq!(told, ["Dicate whispered"]);
    assert!(line_first, "the line is drawn before its banner");
    let (untold, _) = banners(&mut dicate).await.unwrap();
    assert!(untold.is_empty(), "Dicate's page: {untold:?}");

    stop_web.cancel();
    let _ = web.await;
    a_stop.cancel();
    b_stop.cancel();
    let _ = a_actor.await;
    let _ = b_actor.await;
}
