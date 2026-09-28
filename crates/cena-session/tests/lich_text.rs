//! `plan/51` §7, step 3: while the player's Lich runs, what it shows is the
//! character's text (`cena_session::script::lich`).
//!
//! The test stands in for Lich: it is handed the copy of the game's bytes,
//! and shows what it makes of them, as Lich's standard output would. It hides
//! a line, as a squelch does, and adds one of its own, as a script's message
//! does. What a viewer is given, [`Event::Line`] and [`Event::Prompt`], must
//! be that, once; the game's own parse must still be what the triggers act
//! on.

use std::time::Duration;

use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::script::lich::{Attached, Shown};
use cena_session::trigger::{Flag, Look, Matcher, Pattern, Rule, Span, Trigger};
use cena_session::{
    CommandId, ConnectError, Connector, Event, Generation, Origin, Session, SessionHandle, State,
    SupervisedSession, queue::any_frame,
};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;

const PROMPT: &[u8] = b"<prompt time=\"1\">&gt;</prompt>\n";
const LOOK: &[u8] = b"The room.\r\nA line to hide me.\r\n<prompt time=\"2\">&gt;</prompt>\r\n";
const DEADLINE: Duration = Duration::from_secs(5);

/// What Lich makes of the game's `chunk`: the lines saying `hide me` hidden,
/// and a line of its own after `The room.`. By lines, as the chunks here are
/// whole lines.
fn as_lich(chunk: &[u8]) -> Vec<u8> {
    let mut shown = String::new();
    for line in String::from_utf8_lossy(chunk).split_inclusive('\n') {
        if line.contains("hide me") {
            continue;
        }
        shown.push_str(line);
        if line.starts_with("The room.") {
            shown.push_str("[lich] Noted.\r\n");
        }
    }
    shown.into_bytes()
}

/// Lich, standing in: shows what it makes of each chunk it is handed, and
/// each line sent on the channel returned, as a script's message.
fn lich(attached: Attached) -> (JoinHandle<()>, mpsc::Sender<&'static [u8]>) {
    let Attached {
        mut wire, shown, ..
    } = attached;
    let (says, mut said) = mpsc::channel::<&'static [u8]>(8);
    let task = tokio::spawn(async move {
        loop {
            tokio::select! {
                chunk = wire.next() => match chunk {
                    Some(chunk) => show(&shown, as_lich(&chunk)),
                    None => return,
                },
                Some(line) = said.recv() => show(&shown, line.to_vec()),
            }
        }
    });
    (task, says)
}

fn show(shown: &Shown, chunk: Vec<u8>) {
    assert!(shown.show(chunk), "the session reads what Lich shows");
}

/// What a viewer is shown, from here on until `prompts` prompts: each line's
/// text, and each prompt as `[>]`.
async fn shown(events: &mut broadcast::Receiver<Event>, prompts: usize) -> Vec<String> {
    let mut shown = Vec::new();
    let mut seen = 0;
    let reading = async {
        while seen < prompts {
            match events.recv().await {
                Ok(Event::Line(line)) => shown.push(line.text()),
                Ok(Event::Prompt(prompt)) => {
                    seen += 1;
                    shown.push(format!("[{prompt}]"));
                }
                Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => return,
            }
        }
    };
    let _ = tokio::time::timeout(Duration::from_mins(1), reading).await;
    shown
}

fn trigger(name: &str, text: &str, rule: Rule) -> Trigger {
    Trigger {
        name: name.into(),
        rule: Rule {
            pattern: Some(Pattern::Literal {
                text: text.into(),
                whole_word: true,
            }),
            ..rule
        },
    }
}

/// A character logged in, and its events.
type Character = (SessionHandle, TranscriptHandle, broadcast::Receiver<Event>);

/// A character logged in with a Lich attached before its first byte, as the
/// relay attaches one, and its triggers.
fn character(triggers: Vec<Trigger>) -> Option<(Character, Attached)> {
    let (source, transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    handle.set_triggers(Matcher::new(triggers).unwrap_or_default());
    let (_, events) = session.subscribe();
    let attached = handle.lich_door().attach()?;
    tokio::spawn(session.into_actor().run());
    Some(((handle, transcript, events), attached))
}

/// What Lich shows is what a viewer is shown, once: the line it hid is not,
/// and its own is, with the prompt after both. The triggers paint Lich's
/// line, and act on the game's, which Lich hid.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn what_lich_shows_is_the_characters_text() {
    let ((handle, transcript, mut events), attached) = character(vec![
        trigger(
            "room",
            "The room",
            Rule {
                look: Some(Look {
                    color: None,
                    background: None,
                    bold: true,
                    span: Span::Line,
                }),
                ..Rule::default()
            },
        ),
        trigger(
            "hidden",
            "hide me",
            Rule {
                flag: Some(Flag {
                    name: "hidden".into(),
                    seconds: None,
                    clear: false,
                }),
                ..Rule::default()
            },
        ),
    ])
    .expect("a Lich");
    let (_lich, _says) = lich(attached);
    assert_eq!(shown(&mut events, 1).await, ["[>]"], "the login's, Lich's");

    transcript.answer("look", LOOK);
    let _ = handle
        .send_manual_at(handle.generation(), "look", DEADLINE)
        .await;
    let mut painted = Vec::new();
    let mut flagged = 0;
    let mut seen = Vec::new();
    let reading = async {
        loop {
            match events.recv().await {
                Ok(Event::Line(line)) => {
                    painted.push(!line.paint.is_empty());
                    seen.push(line.text());
                }
                Ok(Event::Prompt(prompt)) => {
                    seen.push(format!("[{prompt}]"));
                    return;
                }
                Ok(Event::Flag(_)) => flagged += 1,
                Ok(_) => {}
                Err(_) => return,
            }
        }
    };
    tokio::time::timeout(Duration::from_mins(1), reading)
        .await
        .expect("the look's prompt");
    assert_eq!(seen, ["The room.", "[lich] Noted.", "[>]"]);
    assert_eq!(painted, [true, false], "the triggers' look, on Lich's line");
    assert_eq!(flagged, 1, "the triggers act on the game's line Lich hid");
}

/// A quiet command's report is left out of what Lich shows, though Lich's
/// copy of it comes after the window, and no `Quiet` is published for a
/// viewer to leave out Lich's lines by.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_quiet_report_is_left_out_of_what_lich_shows() {
    let ((handle, transcript, mut events), attached) = character(Vec::new()).expect("a Lich");
    let (_lich, _says) = lich(attached);
    assert_eq!(shown(&mut events, 1).await, ["[>]"]);

    transcript.answer(
        "info",
        b"Name: Someone\r\nRace: Human\r\n<prompt time=\"2\">&gt;</prompt>\r\n",
    );
    let _ = handle
        .send_quietly(CommandId(1), "info", Origin::Manual, DEADLINE, any_frame)
        .await;
    transcript.answer("look", LOOK);
    let _ = handle
        .send_manual_at(handle.generation(), "look", DEADLINE)
        .await;

    let (mut seen, mut quiet) = (Vec::new(), 0);
    let reading = async {
        let mut prompts = 0;
        while prompts < 2 {
            match events.recv().await {
                Ok(Event::Line(line)) => seen.push(line.text()),
                Ok(Event::Prompt(prompt)) => {
                    prompts += 1;
                    seen.push(format!("[{prompt}]"));
                }
                Ok(Event::Quiet(_)) => quiet += 1,
                Ok(_) => {}
                Err(_) => return,
            }
        }
    };
    tokio::time::timeout(Duration::from_mins(1), reading)
        .await
        .expect("both prompts");
    assert_eq!(
        seen,
        ["[>]", "The room.", "[lich] Noted.", "[>]"],
        "the report, and nothing else, left out"
    );
    assert_eq!(quiet, 0);
}

/// Once Lich stops, the game's text is shown again, prompt and all.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn once_lich_stops_the_games_text_is_shown_again() {
    let ((handle, transcript, mut events), attached) = character(Vec::new()).expect("a Lich");
    let (lich, _says) = lich(attached);
    assert_eq!(shown(&mut events, 1).await, ["[>]"]);
    lich.abort();
    let _ = lich.await;

    transcript.answer("look", LOOK);
    let _ = handle
        .send_manual_at(handle.generation(), "look", DEADLINE)
        .await;
    assert_eq!(
        shown(&mut events, 1).await,
        ["The room.", "A line to hide me.", "[>]"]
    );
}

/// Every connect logs in; the test hangs each up when it chooses.
#[derive(Clone, Default)]
struct Logins(std::sync::Arc<std::sync::Mutex<Vec<TranscriptHandle>>>);

impl Connector for Logins {
    type Source = AnsweringSource;

    async fn connect(&mut self, _generation: Generation) -> Result<AnsweringSource, ConnectError> {
        let (source, transcript) = AnsweringSource::logged_in(PROMPT);
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(transcript);
        Ok(source)
    }
}

/// Lich stays up through a reconnect, and so does what it shows: what it
/// says while there is no connection is shown once there is one, and the
/// new login is more of its stream.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn what_lich_shows_outlives_a_connection() {
    let logins = Logins::default();
    let (session, handle) = SupervisedSession::new(logins.clone());
    let (_, mut events) = session.subscribe();
    let (_lich, says) = lich(handle.lich_door().attach().expect("the first Lich"));
    tokio::spawn(session.run());
    assert_eq!(shown(&mut events, 1).await, ["[>]"], "the first login's");

    logins.0.lock().unwrap()[0].hang_up();
    let reconnecting = async {
        loop {
            if let Ok(Event::StateChanged(State::Reconnecting)) = events.recv().await {
                return;
            }
        }
    };
    tokio::time::timeout(Duration::from_mins(1), reconnecting)
        .await
        .expect("a reconnect");
    says.send(b"[lich] Still here.\r\n").await.unwrap();

    assert_eq!(
        shown(&mut events, 1).await,
        ["[lich] Still here.", "[>]"],
        "said between connections, then the second login's prompt"
    );
}
