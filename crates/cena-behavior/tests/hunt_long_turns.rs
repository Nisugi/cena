//! A hunt's turn as it really runs: long, lagged, cut by a lost connection,
//! and hearing the game's lines as the parser gives them, a run at a time
//! (the crate review of 2026-10-01: BE-A-4, BE-A-7, BE-A-10, BE-A-1). Over a
//! scripted game; the hunt reads a channel the session's events are passed
//! on to, which the test can also speak on.

mod ready;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use cena_behavior::BehaviorError;
use cena_behavior::hunt::{Hunt, HuntEnd, Profile, hunt};
use cena_behavior::travel::TravelNotes;
use cena_behavior::watchdog::{BEHAVIOR_WATCHDOG, Heartbeat, Watched, watch};
use cena_map::{Map, Room};
use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::{
    AuthorityToken, CommandId, Event, Frame, GameState, GenerationCell, Link, LinkKind, Origin,
    Run, Runs, Session, SessionHandle, State,
};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

const PROMPT: &[u8] = b"<prompt time=\"1\">&gt;</prompt>\n";
const TOKEN: AuthorityToken = AuthorityToken(1);

/// A hostile kobold #42, as the room and its tag state it.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only its bold depth matters"
)]
fn kobold_frames() -> Vec<Frame> {
    let mut run = Run {
        text: "kobold".to_owned(),
        style: Default::default(),
        link: Some(Link {
            kind: LinkKind::Exist {
                id: "42".to_owned(),
                noun: "kobold".to_owned(),
            },
            text: "kobold".to_owned(),
            coord: None,
        }),
        inner_link: None,
    };
    run.style.bold_depth = 1;
    vec![
        Frame::Component {
            id: "room players".into(),
            body: Runs { runs: Vec::new() },
        },
        Frame::Component {
            id: "room objs".into(),
            body: Runs { runs: vec![run] },
        },
        Frame::CreatureStatus {
            id: "42".into(),
            attrs: vec![
                ("exist".to_owned(), "42".to_owned()),
                ("hostile".to_owned(), "1".to_owned()),
            ],
        },
    ]
}

/// The kobold's room as the game says it, so the session's own model, which
/// the write-time gate reads, has it too.
const KOBOLD_ROOM: &[u8] = b"<component id='room players'></component>
<component id='room objs'>You also see <pushBold/>a <a exist=\"42\" noun=\"kobold\">kobold</a><popBold/>.</component>
<crtrStatus exist=\"42\" hostile=\"1\"/>
<prompt time=\"1001\">&gt;</prompt>
";

/// One running hunt and what the test reaches it by.
struct Hunting {
    transcript: TranscriptHandle,
    handle: SessionHandle,
    cell: GenerationCell,
    /// The channel the hunt reads; the test speaks on it too.
    say: broadcast::Sender<Event>,
    stop: CancellationToken,
    heartbeat: Heartbeat,
    task: JoinHandle<Option<HuntEnd>>,
}

/// A hunt on `profile` over `rooms`, standing in the game's room `room`,
/// with the kobold there (and targeted) when `kobold`. Its stream holds
/// `capacity` events; `answers` script the game.
fn set_out(
    rooms: &'static str,
    profile: &'static str,
    room: &str,
    kobold: bool,
    capacity: usize,
    answers: &[(&str, Vec<u8>)],
) -> Hunting {
    let (source, transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let cell = session.generation_cell();
    let (mut snapshot, from_session) = session.subscribe();
    let (_, ready) = session.subscribe();
    let state: &mut GameState = &mut snapshot.state;
    state.apply(&Frame::Prompt {
        time: "1000".into(),
        text: ">".into(),
    });
    state.room.id = Some(room.into());
    state.status.set("standing", true);
    state.character.stance = Some("defensive (100%)".to_owned());
    if kobold {
        for frame in kobold_frames() {
            state.apply(&frame);
        }
        state.targeting.read("#42", None);
        transcript.answer("look", KOBOLD_ROOM);
    }
    for (command, answer) in answers {
        transcript.answer(command, answer);
    }
    tokio::spawn(session.into_actor().run());

    let (say, heard) = broadcast::channel::<Event>(capacity);
    forward(from_session, say.clone());
    let stop = CancellationToken::new();
    let heartbeat = Heartbeat::default();
    let task = {
        let (handle, stop, heartbeat) = (handle.clone(), stop.clone(), heartbeat.clone());
        tokio::spawn(async move {
            ready::until_ready(ready).await.ok()?;
            if kobold {
                let _ = handle
                    .send_and_await(
                        CommandId(900),
                        "look",
                        Origin::Manual,
                        Duration::from_secs(5),
                        |frame: &Frame| matches!(frame, Frame::Prompt { .. }),
                    )
                    .await;
            }
            let rooms: Vec<Room> = serde_json::from_str(rooms).ok()?;
            let map = Map::from_rooms(rooms).ok()?;
            let next = Arc::new(AtomicU64::new(0));
            let ids = move || CommandId(next.fetch_add(1, Ordering::Relaxed));
            let machine = Hunt::new(Profile::parse(profile).ok()?, 1);
            handle.claim(TOKEN).await.ok()?;
            Some(
                Box::pin(hunt(
                    &handle,
                    &stop,
                    ids,
                    TOKEN,
                    (snapshot, heard.into()),
                    &map,
                    machine,
                    &heartbeat,
                    TravelNotes::default(),
                    |_| {},
                    |_| {},
                ))
                .await,
            )
        })
    };
    Hunting {
        transcript,
        handle,
        cell,
        say,
        stop,
        heartbeat,
        task,
    }
}

/// Pass the session's events on to `say`.
fn forward(mut from_session: broadcast::Receiver<Event>, say: broadcast::Sender<Event>) {
    tokio::spawn(async move {
        loop {
            match from_session.recv().await {
                Ok(event) => {
                    let _ = say.send(event);
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

/// Let the paused clock run `millis`, yielding as it goes.
async fn pass(millis: u64) {
    for _ in 0..millis / 10 {
        tokio::time::advance(Duration::from_millis(10)).await;
        tokio::task::yield_now().await;
    }
}

/// How many lines written are exactly `line`.
fn written(transcript: &TranscriptHandle, line: &str) -> usize {
    transcript
        .lines()
        .iter()
        .filter(|written| written.as_str() == line)
        .count()
}

/// Let the paused clock run until `line` has been written `n` times.
async fn until(transcript: &TranscriptHandle, line: &str, n: usize) -> bool {
    for _ in 0..3_000 {
        if written(transcript, line) >= n {
            return true;
        }
        pass(10).await;
    }
    false
}

/// Eleven rooms in a line, each north of the last; nothing leads back.
const CORRIDOR: &str = r#"[
  {"id":1,"uid":[1001],"exits":[{"to":2,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":2,"uid":[1002],"exits":[{"to":3,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":3,"uid":[1003],"exits":[{"to":4,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":4,"uid":[1004],"exits":[{"to":5,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":5,"uid":[1005],"exits":[{"to":6,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":6,"uid":[1006],"exits":[{"to":7,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":7,"uid":[1007],"exits":[{"to":8,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":8,"uid":[1008],"exits":[{"to":9,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":9,"uid":[1009],"exits":[{"to":10,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":10,"uid":[1010],"exits":[{"to":11,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":11,"uid":[1011]}
]"#;

/// Hunting at the corridor's end.
const AT_THE_END: &str = "targets = [{ any = true, routine = \"a\" }]\n[rooms]\nhunting = 11\n[stance]\nwander = \"defensive\"\n[routines]\na = [\"attack\"]\n";

/// BE-A-4: a walk is one turn of the hunt, and the heartbeat was beaten
/// once a turn, so a walk longer than the watchdog's thirty seconds was
/// preempted as wedged, leaving the character mid-route. Here each of the
/// ten steps is answered seven seconds late, inside travel's own eight
/// before it sends a step again: 70 s of walking, every step heard.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_walk_longer_than_the_watchdog_is_not_taken_for_a_wedge() {
    let answers: Vec<(&str, Vec<u8>)> = (2..=11)
        .map(|room| {
            (
                "north",
                format!(
                    "<nav rm='{}'/>\n<prompt time=\"2\">&gt;</prompt>\n",
                    1000 + room
                )
                .into_bytes(),
            )
        })
        .collect();
    let run = set_out(CORRIDOR, AT_THE_END, "1001", false, 1024, &answers);
    // Every step's answer late, from the first.
    run.transcript.hold_replies();
    let watcher = {
        let (handle, stop, heartbeat) =
            (run.handle.clone(), run.stop.clone(), run.heartbeat.clone());
        tokio::spawn(
            async move { watch(&handle, &stop, &heartbeat, BEHAVIOR_WATCHDOG, "Hunt").await },
        )
    };
    assert!(
        until(&run.transcript, "north", 1).await,
        "walking: {:?}",
        run.transcript.lines()
    );
    for _ in 0..10 {
        pass(7_000).await;
        run.transcript.release_one();
    }
    pass(1_000).await;
    assert_eq!(
        written(&run.transcript, "north"),
        10,
        "walked the whole corridor: {:?}",
        run.transcript.lines()
    );
    assert!(
        !watcher.is_finished(),
        "a long walk is not a wedge: {:?}",
        run.transcript.lines()
    );
    assert_eq!(run.handle.holder(), Some(TOKEN), "nothing was taken");
    run.stop.cancel();
    assert_eq!(watcher.await.ok(), Some(Watched::Stopped));
    let _ = run.task.await;
}

/// BE-A-4's other half: a hunt that neither turns nor hears anything is
/// still preempted. Its one walk's steps are never answered and the game
/// says nothing: travel gives the walk up in time, but until it does the
/// turn hears nothing, and a heartbeat beaten by events alone stays quiet.
/// So the watchdog is left to the turn: after the give-up the loop turns.
/// What this pins is the bound: a turn that hears nothing for the
/// watchdog's limit is preempted.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_turn_that_hears_nothing_for_the_limit_is_still_preempted() {
    let run = set_out(CORRIDOR, AT_THE_END, "1001", false, 1024, &[]);
    let watcher = {
        let (handle, stop, heartbeat) =
            (run.handle.clone(), run.stop.clone(), run.heartbeat.clone());
        tokio::spawn(async move {
            watch(&handle, &stop, &heartbeat, Duration::from_secs(4), "Hunt").await
        })
    };
    run.transcript.hold_replies();
    // Every step unanswered and nothing said: no event to fold for longer
    // than this watchdog's four seconds, inside the walk's one turn.
    for _ in 0..1_000 {
        if watcher.is_finished() {
            break;
        }
        pass(100).await;
    }
    assert!(
        matches!(watcher.await.ok(), Some(Watched::Wedged(_))),
        "a turn that hears nothing is preempted: {:?}",
        run.transcript.lines()
    );
    let _ = run.task.await;
}

/// The kobold's room, with a room north of it and back.
const TWO_ROOMS: &str = r#"[
  {"id":1,"uid":[1001],"exits":[{"to":2,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":2,"uid":[1002],"exits":[{"to":1,"kind":"cardinal","cmd":"south","cost":1}]}
]"#;

/// Fighting the kobold, never wandering off.
const FIGHTING: &str = "targets = [{ any = true, routine = \"a\" }]\n[rooms]\nhunting = 1\n[stance]\nwander = \"defensive\"\n[wander]\nwait = 1000\n[routines]\na = [\"attack\"]\n[flee]\nmessages = [\"kobold howls\"]\n";

/// BE-A-7: the connection lost while a line is out (the session answers it
/// `Disconnected`) ended the hunt; only a drop between round trips was
/// waited out. Moving the session's connection on under the hunt makes
/// every line it sends from now on one of an older connection.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_connection_lost_while_a_line_is_out_is_waited_out() {
    let run = set_out(TWO_ROOMS, FIGHTING, "1001", true, 1024, &[]);
    assert!(
        until(&run.transcript, "attack", 1).await,
        "hunting: {:?}",
        run.transcript.lines()
    );
    run.cell.advance();
    pass(10_000).await;
    assert!(
        !run.task.is_finished(),
        "a lost connection does not end the hunt: {:?}",
        run.task.await.ok().flatten()
    );
    run.stop.cancel();
    let end = run.task.await.ok().flatten();
    assert_eq!(end, Some(HuntEnd::Stopped(BehaviorError::Cancelled)));
}

/// BE-A-10: a lag seen while a line's answer is read was caught up only at
/// the next turn. The hunt's stream here cannot be taken afresh, so a lag
/// caught up is the end of the hunt (`FellBehind`), and when it ends says
/// when it was caught up. The game says nothing to the attack, so the
/// stream lags while the hunt waits on its prompt (eight seconds); the hunt
/// read on for its answer after that (three seconds) and only then turned.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_lag_seen_while_a_line_is_out_is_caught_up_before_the_answer_is_read() {
    let run = set_out(
        TWO_ROOMS,
        FIGHTING,
        "1001",
        true,
        1024,
        &[("attack", Vec::new())],
    );
    assert!(
        until(&run.transcript, "attack", 1).await,
        "hunting: {:?}",
        run.transcript.lines()
    );
    // More than the stream holds: the hunt's receiver lags.
    for _ in 0..2_048 {
        let _ = run.say.send(Event::StateChanged(State::Ready));
    }
    // The attack's own wait for its prompt is eight seconds; past that, and
    // well short of the three more the answer would be read for.
    pass(9_000).await;
    assert!(
        run.task.is_finished(),
        "the lag is caught up as soon as it is seen: {:?}",
        run.transcript.lines()
    );
    let end = run.task.await.ok().flatten();
    assert_eq!(end, Some(HuntEnd::Stopped(BehaviorError::FellBehind)));
}

/// BE-A-1: what the hunt hears is a whole line. A creature's name is a link,
/// and the parser ends a run at every link, so `The kobold howls!` reached
/// the hunt as three runs, and a phrase across the link was never heard.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_flee_phrase_across_a_link_is_heard() {
    let run = set_out(TWO_ROOMS, FIGHTING, "1001", true, 1024, &[]);
    assert!(
        until(&run.transcript, "attack", 1).await,
        "hunting: {:?}",
        run.transcript.lines()
    );
    run.transcript.say(
        b"The <a exist=\"42\" noun=\"kobold\">kobold</a> howls!\n<prompt time=\"1001\">&gt;</prompt>\n",
    );
    assert!(
        until(&run.transcript, "north", 1).await,
        "fled on the phrase: {:?}",
        run.transcript.lines()
    );
    run.stop.cancel();
    let _ = run.task.await;
}
