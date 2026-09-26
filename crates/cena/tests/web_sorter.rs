//! `;sorter` end to end: real container looks, through the parser and the
//! presentation pump, to the page (`plan/30` §4, M6e).
//!
//! The sorter's own tests (`crates/cena-ui/src/sorter.rs`) cut the
//! fixture's links by hand. This one gives the same bytes to a session, so
//! the parser decides the pieces: the `<container>` and `<inv>` preamble
//! before each look -- which holds its own `In the <a ...>box</a>:` -- must
//! stay out of the story, and the look's own links must reach the sorter
//! with their nouns. If either broke, the sorted lines below would not
//! appear.

mod web_support;

use cena_platform::AnsweringSource;
use cena_session::{CommandId, Generation, Origin, Outcome, Session, queue::any_frame};
use cena_ui::{ServerMessage, StoryLine};
use cena_web::WebServer;
use tokio_util::sync::CancellationToken;
use web_support::*;

/// Five looks from the author's logs, each with its prompt; provenance in
/// the sorter's tests.
const LOOKS: &str = include_str!("../../cena-ui/tests/fixtures/container_looks.xml");

/// The fixture's look at line `index` and the prompt after it, as the game
/// sends them. Rejoined with bare LF, so a CRLF checkout cannot change them.
fn reply(index: usize) -> Vec<u8> {
    let mut bytes = String::new();
    for line in LOOKS.lines().skip(index).take(2) {
        bytes.push_str(line);
        bytes.push('\n');
    }
    bytes.into_bytes()
}

fn text(line: &StoryLine) -> String {
    line.runs.iter().map(|run| run.text.as_str()).collect()
}

/// Story lines from `socket` until one satisfies `done`; all of them.
async fn until(socket: &mut Browser, done: impl Fn(&str) -> bool) -> TestResult<Vec<String>> {
    let mut lines = Vec::new();
    tokio::time::timeout(DEADLINE, async {
        while !lines.iter().any(|line: &String| done(line)) {
            if let ServerMessage::Update { lines: new, .. } = receive(socket).await? {
                lines.extend(new.iter().map(text));
            }
        }
        TestResult::Ok(())
    })
    .await??;
    Ok(lines)
}

#[tokio::test]
async fn a_look_reaches_the_page_sorted_once_sorting_is_on() {
    let (source, transcript) = AnsweringSource::logged_in(b"<prompt time=\"2\">&gt;</prompt>\n");
    transcript.answer("look in box", &reply(0));
    transcript.answer("look in box", &reply(0));
    transcript.answer("look in kit", &reply(8));
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
    let sessions = server.sessions();
    let stop_web = CancellationToken::new();
    let web = tokio::spawn(server.run(stop_web.clone().cancelled_owned()));
    let mut socket = browser(&pairing).await.unwrap();
    assert!(matches!(
        receive(&mut socket).await.unwrap(),
        ServerMessage::Snapshot { .. }
    ));
    let look = async |id: u64, line: &str| {
        let sent = handle
            .send_and_await(CommandId(id), line, Origin::Manual, DEADLINE, any_frame)
            .await;
        assert!(matches!(sent, Outcome::Confirmed(_)), "{sent:?}");
    };

    // Off, as it starts: the look arrives as the game sent it.
    look(1, "look in box").await;
    let whole = "In the mahogany box you see a bright gold ingot, some silver coins, \
                 a smooth amber wand, a piece of brown jade, a steel lockpick, \
                 a pinch of electrum dust and a tar black tourmaline.";
    let lines = until(&mut socket, |line| line.starts_with("In the mahogany box"))
        .await
        .unwrap();
    assert_eq!(
        lines,
        [whole],
        "the preamble's `In the box:` is not the story's"
    );

    // On: the same bytes, one line per category.
    assert!(sessions.sort_containers(handle.session(), true));
    look(2, "look in box").await;
    let lines = until(&mut socket, |line| line.starts_with("  lockpick"))
        .await
        .unwrap();
    assert_eq!(
        lines,
        [
            "In the mahogany box:",
            "  valuable (1): bright gold ingot",
            "  other (1): some silver coins",
            "  wand (1): smooth amber wand",
            "  gem (3): pinch of electrum dust, piece of brown jade, tar black tourmaline",
            "  lockpick (1): steel lockpick",
        ]
    );

    // Still on, and the herb kit is shown whole: it goes on past its list.
    look(3, "look in kit").await;
    let lines = until(&mut socket, |line| {
        line.starts_with("In the leather herb kit")
    })
    .await
    .unwrap();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("wolifrew (143)"), "{lines:?}");

    stop_web.cancel();
    stop_session.cancel();
    let _ = web.await;
    let _ = actor.await;
}
