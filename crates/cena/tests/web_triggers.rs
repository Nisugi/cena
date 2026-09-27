//! M8 Stage 1's "done when" (`plan/45` §6): a scripted two-character session
//! shows a colour, a squelch, a substitute and a redirect, one of them
//! limited to one character, in Despana.
//!
//! One triggers file, read as Hydra reads it, gives each character its own
//! list; each session answers its lines; each character's page draws what it
//! is given. Nothing here is live: the game is an `AnsweringSource`.

mod web_support;

use cena_behavior::triggers;
use cena_platform::AnsweringSource;
use cena_session::trigger::Matcher;
use cena_session::{Generation, Outcome, Session, SessionHandle, SessionId};
use cena_ui::{RoomItemView, ServerMessage, StoryLine};
use cena_web::WebServer;
use tokio_util::sync::CancellationToken;
use web_support::*;

/// The one file. The redirect is Nisugi's alone.
const FILE: &str = r##"
[trigger.stunned]
category = "Combat"
text = "You are stunned"
look = { color = "#ff4040", bold = true }

[trigger.spam]
category = "Ignores"
text = "gestures"
squelch = true

[trigger.rat]
text = "the rat"
substitute = "THE RAT"

[trigger.whispers]
text = "whispers"
redirect = { stream = "whispers" }
characters = ["Nisugi"]
"##;

/// What the game says to `look`: one line for each response, then a prompt.
const LOOK: &[u8] = b"You are stunned!\n\
Someone gestures.\n\
You see the rat.\n\
Dicate whispers, \"meet me at the gate\"\n\
<prompt time=\"1\">&gt;</prompt>\n";

fn text(line: &StoryLine) -> String {
    line.runs.iter().map(|run| run.text.as_str()).collect()
}

/// `character`'s triggers from the file, on its session, as the binary gives
/// them at start (`crates/cena/src/triggers.rs`).
fn give(handle: &SessionHandle, character: &str) -> TestResult {
    let loaded = triggers::read(FILE)?;
    assert!(loaded.refused.is_empty(), "{:?}", loaded.refused);
    handle.set_triggers(Matcher::new(loaded.triggers.for_character(character))?);
    Ok(())
}

/// Story lines from `page` until the whisper arrives; all of them.
async fn until_the_whisper(page: &mut Browser) -> TestResult<Vec<StoryLine>> {
    let mut lines: Vec<StoryLine> = Vec::new();
    tokio::time::timeout(DEADLINE, async {
        while !lines.iter().any(|line| text(line).contains("whispers")) {
            if let ServerMessage::Update { lines: new, .. } = receive(page).await? {
                lines.extend(new);
            }
        }
        TestResult::Ok(())
    })
    .await??;
    Ok(lines)
}

#[tokio::test]
async fn two_characters_see_one_file_answered_each_their_own_way() {
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

    for (name, page, whisper_stream) in [
        ("Nisugi", &mut nisugi, "whispers"),
        ("Dicate", &mut dicate, ""),
    ] {
        let lines = until_the_whisper(page).await.unwrap();
        let drawn: Vec<(String, String)> = lines
            .iter()
            .map(|line| (line.stream.clone(), text(line)))
            .collect();

        // The colour, on the words and not the rest.
        let stunned = lines
            .iter()
            .find(|line| text(line) == "You are stunned!")
            .unwrap_or_else(|| panic!("{name}: {drawn:?}"));
        let painted: Vec<(&str, Option<&str>, bool)> = stunned
            .runs
            .iter()
            .map(|run| (run.text.as_str(), run.color.as_deref(), run.bold))
            .collect();
        assert_eq!(
            painted,
            [
                ("You are stunned", Some("#ff4040"), true),
                ("!", None, false)
            ],
            "{name}"
        );
        // The squelch: the line never reaches the page.
        assert!(
            !drawn.iter().any(|(_, text)| text.contains("gestures")),
            "{name}: {drawn:?}"
        );
        // The substitute.
        assert!(
            drawn.contains(&(String::new(), "You see THE RAT.".to_owned())),
            "{name}: {drawn:?}"
        );
        // The redirect, Nisugi's alone: Dicate's whisper stays in the story.
        assert!(
            drawn.contains(&(
                whisper_stream.to_owned(),
                "Dicate whispers, \"meet me at the gate\"".to_owned()
            )),
            "{name}: {drawn:?}"
        );
    }

    stop_web.cancel();
    web.await.unwrap().unwrap();
    a_stop.cancel();
    b_stop.cancel();
    let _ = (a_actor.await, b_actor.await);
}

/// A player's name as the page is given it: each run's text and colour, or
/// nothing when no trigger painted it.
type Drawn = Option<Vec<(String, Option<String>)>>;

/// What the game says to `look` in a room with two players in it.
const PLAYERS: &[u8] = b"<component id='room players'>Also here: \
<a exist=\"-7\" noun=\"Maravel\">Maravel</a> and <a exist=\"-8\" noun=\"Orsen\">Orsen</a>\
</component>\n<prompt time=\"1\">&gt;</prompt>\n";

/// Stage 7 (`plan/45`): a name in the room window's players is painted by
/// the character's triggers, through the snapshot the page is drawn from.
#[tokio::test]
async fn a_name_in_the_room_window_is_painted_by_the_triggers() {
    let (source, transcript) = AnsweringSource::logged_in(ROOM);
    transcript.answer("look", PLAYERS);
    let session = Session::numbered(SessionId(0), source);
    let (handle, observer, stop) = (session.handle(), session.observer(), session.cancel_token());
    let file = "[trigger.friend]\ntext = 'Maravel'\nlook = { color = '#ecc013' }\n";
    let loaded = triggers::read(file).unwrap();
    handle.set_triggers(Matcher::new(loaded.triggers.for_character("Nisugi")).unwrap());
    let actor = tokio::spawn(session.into_actor().run());
    await_ready(&observer, Generation::FIRST).await.unwrap();

    let server = WebServer::open().await.unwrap();
    server.sessions().attach("Nisugi", observer, handle.clone());
    let pairing = server.pairing_url();
    let stop_web = CancellationToken::new();
    let web = tokio::spawn(server.run(stop_web.clone().cancelled_owned()));
    let mut page = browser_for(&pairing, Some("0")).await.unwrap();
    let sent = handle
        .send_manual_at(Generation::FIRST, "look", DEADLINE)
        .await;
    assert!(matches!(sent, Outcome::Confirmed(_)), "{sent:?}");

    let players: Vec<RoomItemView> = tokio::time::timeout(DEADLINE, async {
        loop {
            if let ServerMessage::Snapshot { view, .. } | ServerMessage::Update { view, .. } =
                receive(&mut page).await?
                && let Some(players) = view.room.players
                && players.len() == 2
            {
                return TestResult::Ok(players);
            }
        }
    })
    .await
    .unwrap()
    .unwrap();
    let colours: Vec<Drawn> = players
        .iter()
        .map(|player| {
            player.painted.as_ref().map(|runs| {
                runs.iter()
                    .map(|run| (run.text.clone(), run.color.clone()))
                    .collect()
            })
        })
        .collect();
    assert_eq!(
        colours,
        [
            Some(vec![("Maravel".to_owned(), Some("#ecc013".to_owned()))]),
            None
        ]
    );

    stop_web.cancel();
    web.await.unwrap().unwrap();
    stop.cancel();
    let _ = actor.await;
}
