//! `plan/51` §7, step 4: a Lich started after the login is handed what it
//! missed first, as the game sent it -- the login, then the latest word since
//! on each piece of state it keeps -- and the character's text does not show
//! that again (`cena_session::script::lich`, started late).

use std::time::Duration;

use cena_platform::{AnsweringSource, LOGIN_BURST};
use cena_session::script::lich::Attached;
use cena_session::{Event, Session, State};
use tokio::sync::broadcast;

const PROMPT: &[u8] = b"<prompt time=\"1\">&gt;</prompt>\n";
const DEADLINE: Duration = Duration::from_secs(5);

/// The room, with a gem in the left hand.
const ROOM: &[u8] = b"<nav rm='7'/><streamWindow id='room' subtitle=' - [Town Square]'/>\
The square.\r\n<left exist=\"1\" noun=\"gem\">a gem</left>\r\n<prompt time=\"2\">&gt;</prompt>\r\n";
/// The hand again, now empty.
const DROP: &[u8] =
    b"You drop a gem.\r\n<left>Empty</left>\r\n<prompt time=\"3\">&gt;</prompt>\r\n";
/// Nothing Lich keeps.
const SMILE: &[u8] = b"You smile.\r\n<prompt time=\"4\">&gt;</prompt>\r\n";

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

/// Handed first: the login, the room's chunk (the latest to say which room,
/// though not the latest to say the hand), and the drop's (the latest to say
/// the hand); not the smile's. Then the live stream, which alone is shown.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_lich_started_late_is_handed_the_login_and_the_latest_word() {
    let (source, transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let (_, mut events) = session.subscribe();
    tokio::spawn(session.into_actor().run());
    let ready =
        async { while !matches!(events.recv().await, Ok(Event::StateChanged(State::Ready))) {} };
    tokio::time::timeout(DEADLINE, ready).await.expect("ready");
    for (command, reply) in [("look", ROOM), ("drop gem", DROP), ("smile", SMILE)] {
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
    let past = wire.next().await.expect("what it missed");
    assert_eq!(
        String::from_utf8_lossy(&past),
        String::from_utf8_lossy(&[LOGIN_BURST, ROOM, DROP].concat()),
    );
    // Lich passes the past on, and the live stream after it.
    assert!(shows.show(past));
    transcript.answer(
        "look",
        b"The square again.\r\n<prompt time=\"5\">&gt;</prompt>\r\n",
    );
    let _ = handle
        .send_manual_at(handle.generation(), "look", DEADLINE)
        .await;
    assert!(shows.show(wire.next().await.expect("the live stream")));
    assert_eq!(
        shown(&mut events).await,
        ["The square again.", "[>]"],
        "the past, which was shown once, is not shown again"
    );
}
