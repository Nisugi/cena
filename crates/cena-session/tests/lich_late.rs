//! `plan/51` §7, step 4: a Lich started after the login is handed a login
//! built from what the session knows as it attaches (`GameState::login`),
//! then the live stream; and the character's text does not show that login
//! again (`cena_session::script::lich`, started late).

use std::time::Duration;

use cena_platform::AnsweringSource;
use cena_session::script::lich::Attached;
use cena_session::{Event, Session, State};
use tokio::sync::broadcast;

const PROMPT: &[u8] = b"<prompt time=\"1\">&gt;</prompt>\n";
const DEADLINE: Duration = Duration::from_secs(5);

/// The room, with a gem in the left hand.
const ROOM: &str = "<nav rm='7'/><streamWindow id='room' subtitle=' - [Town Square]'/>\
The square.\r\n<left exist=\"1\" noun=\"gem\">a gem</left>\r\n<prompt time=\"2\">&gt;</prompt>\r\n";
/// The hand again, now empty.
const DROP: &[u8] =
    b"You drop a gem.\r\n<left>Empty</left>\r\n<prompt time=\"3\">&gt;</prompt>\r\n";

/// What a viewer is shown, from here on until a prompt: each line's text,
/// and the prompt as `[>]`.
async fn shown(events: &mut broadcast::Receiver<Event>) -> Vec<String> {
    let mut shown = Vec::new();
    let reading = async {
        loop {
            match events.recv().await {
                Ok(Event::Line(line)) => shown.push(line.text()),
                Ok(Event::Prompt(prompt)) => {
                    shown.push(format!("[{prompt}]"));
                    return;
                }
                Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => return,
            }
        }
    };
    let _ = tokio::time::timeout(Duration::from_mins(1), reading).await;
    shown
}

/// A Lich attached to a quiet character, after the game named it, a room
/// and a dropped gem, is handed a login saying who, where, and what it holds
/// now: an empty hand, not the gem the room's chunk said. None of the game's
/// text is resent. Then the live stream, which alone is shown.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_lich_started_late_is_told_the_character_as_it_is() {
    let (source, transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let (_, mut events) = session.subscribe();
    tokio::spawn(session.into_actor().run());
    let ready =
        async { while !matches!(events.recv().await, Ok(Event::StateChanged(State::Ready))) {} };
    tokio::time::timeout(DEADLINE, ready).await.expect("ready");
    // Assembled, as the golden corpus assembles it: Rule 3.4's scan flags
    // the instance's name wherever it is spelled.
    let named = format!(
        "<playerID id='5'/><settingsInfo instance='{}'/><app char=\"Tester\" game=\"Prime\"/>\n{ROOM}",
        concat!("GS", "4")
    );
    for (command, reply) in [("look", named.as_bytes()), ("drop gem", DROP)] {
        transcript.answer(command, reply);
        let _ = handle
            .send_manual_at(handle.generation(), command, DEADLINE)
            .await;
    }

    // What was shown before Lich was.
    let mut events = events.resubscribe();
    let Attached {
        mut wire,
        shown: shows,
        ..
    } = handle.lich_door().attach().expect("a Lich");
    let login = tokio::time::timeout(DEADLINE, wire.next())
        .await
        .expect("handed at the actor's next turn, the game being quiet")
        .expect("its login");
    let told = String::from_utf8_lossy(&login);
    for said in [
        "<playerID id=\"5\"/>",
        concat!("<settingsInfo instance=\"GS", "4\"/>"),
        "<app char=\"Tester\" game=\"Prime\"/>",
        "<nav rm=\"7\"/>",
        "subtitle=\" - [Town Square]\"",
        "<left>Empty</left>",
        "<prompt time=",
    ] {
        assert!(told.contains(said), "{said} in {told}");
    }
    assert!(!told.contains("gem"), "the hand as it is now: {told}");
    assert!(!told.contains("The square."), "no text resent: {told}");

    // Lich passes its login on, and the live stream after it.
    assert!(shows.show(login));
    transcript.answer(
        "look",
        b"The square again.\r\n<prompt time=\"5\">&gt;</prompt>\r\n",
    );
    let _ = handle
        .send_manual_at(handle.generation(), "look", DEADLINE)
        .await;
    let live = tokio::time::timeout(DEADLINE, wire.next())
        .await
        .expect("the live stream")
        .expect("a chunk");
    assert!(shows.show(live));
    assert_eq!(
        shown(&mut events).await,
        ["The square again.", "[>]"],
        "what the login told was shown once already"
    );
}
