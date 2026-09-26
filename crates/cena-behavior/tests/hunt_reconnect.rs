//! A reconnect mid-hunt keeps the hunt (`plan/30` §7, M6's acceptance; SE-4
//! (c): the session keeps the authority across a reconnect). Over a scripted
//! game, with the session's events passed through a channel the test can
//! also speak on: the drop and the return are said there.

mod drive_support;
mod ready;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use cena_behavior::hunt::{Hunt, HuntEnd, Profile, hunt};
use cena_behavior::travel::TravelNotes;
use cena_behavior::watchdog::Heartbeat;
use cena_map::{Map, Room};
use cena_platform::AnsweringSource;
use cena_session::{
    AuthorityToken, CommandId, Event, Frame, GameState, Link, LinkKind, Origin, Run, Runs, Session,
    State,
};
use drive_support::{PROMPT, until_written};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

const ROOMS: &str = r#"[{"id":1,"uid":[1001]}]"#;
const PROFILE: &str = r#"
targets = [{ any = true, routine = "a" }]
[rooms]
hunting = 1
[stance]
wander = "defensive"
[wander]
wait = 0
[routines]
a = ["attack"]
"#;

/// A hostile kobold #42 in the room, as the room and its tag state it.
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

/// The room with the kobold, as the game says it: what the session's own
/// model must hold for the write-time gate to let an attack go.
const KOBOLD_ROOM: &[u8] = b"<component id='room players'></component>
<component id='room objs'>You also see <pushBold/>a <a exist=\"42\" noun=\"kobold\">kobold</a><popBold/>.</component>
<crtrStatus exist=\"42\" hostile=\"1\"/>
<prompt time=\"1001\">&gt;</prompt>
";

/// Let the paused clock run `seconds`, yielding as it goes.
async fn pass(seconds: u64) {
    for _ in 0..seconds * 4 {
        tokio::time::advance(Duration::from_millis(250)).await;
        tokio::task::yield_now().await;
    }
}

/// Pass the session's events on to `say`, so the test can speak on the same
/// channel the hunt reads.
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

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_hunt_waits_out_a_drop_and_hunts_on_after_it() {
    let (source, transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let (mut snapshot, from_session) = session.subscribe();
    let (_, ready) = session.subscribe();
    let state: &mut GameState = &mut snapshot.state;
    state.apply(&Frame::Prompt {
        time: "1000".into(),
        text: ">".into(),
    });
    state.room.id = Some("1001".into());
    state.status.set("standing", true);
    for frame in kobold_frames() {
        state.apply(&frame);
    }
    state.targeting.read("#42", None);
    tokio::spawn(session.into_actor().run());

    // The hunt reads this channel: the session's events, and what the test says.
    let (say, heard) = broadcast::channel::<Event>(1024);
    forward(from_session, say.clone());

    transcript.answer("look", KOBOLD_ROOM);
    let stop = CancellationToken::new();
    let hunt_stop = stop.clone();
    let task = tokio::spawn(async move {
        ready::until_ready(ready).await.ok()?;
        // The session's own model learns the kobold, as a look teaches it.
        let _ = handle
            .send_and_await(
                CommandId(900),
                "look",
                Origin::Manual,
                Duration::from_secs(5),
                |frame: &Frame| matches!(frame, Frame::Prompt { .. }),
            )
            .await;
        let rooms: Vec<Room> = serde_json::from_str(ROOMS).ok()?;
        let map = Map::from_rooms(rooms).ok()?;
        let next = Arc::new(AtomicU64::new(0));
        let ids = move || CommandId(next.fetch_add(1, Ordering::Relaxed));
        let machine = Hunt::new(Profile::parse(PROFILE).ok()?, 1);
        handle.claim(AuthorityToken(1)).await.ok()?;
        let heartbeat = Heartbeat::default();
        Some(
            Box::pin(hunt(
                &handle,
                &hunt_stop,
                ids,
                AuthorityToken(1),
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
    });

    assert!(
        until_written(&transcript, "attack").await,
        "hunting: {:?}",
        transcript.lines()
    );
    let _ = say.send(Event::StateChanged(State::Reconnecting));
    pass(2).await;
    let before = transcript.written_count();
    pass(60).await;
    assert!(!task.is_finished(), "a drop does not end the hunt");
    // An idle hunt would take its wander stance; away, it sends nothing.
    assert_eq!(
        transcript.written_count(),
        before,
        "nothing is sent while the session is away: {:?}",
        transcript.lines()
    );

    // Back: the login burst re-teaches the room, and the hunt goes on. The
    // drop forgot the target (targeting is invalidated), so it is taken
    // again first; this scripted game answers with a bare prompt, not the
    // dropdown the real one sends, so the retargeting is what shows.
    let away = transcript.written_count();
    for frame in kobold_frames() {
        let _ = say.send(Event::Frame(Box::new(frame)));
    }
    let _ = say.send(Event::StateChanged(State::Ready));
    let mut resumed = false;
    for _ in 0..200 {
        pass(1).await;
        if transcript.written_count() > away {
            resumed = true;
            break;
        }
    }
    let lines = transcript.lines();
    assert!(resumed, "hunting again after the return: {lines:?}");
    assert_eq!(
        lines.last().map(String::as_str),
        Some("target #42"),
        "the target taken again"
    );
    stop.cancel();
    let end = task.await.ok().flatten();
    assert!(
        matches!(end, Some(HuntEnd::Stopped(_))),
        "it ended when stopped, not before: {end:?}"
    );
}
