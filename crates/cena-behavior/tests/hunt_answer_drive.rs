//! The hunt driver reads on past a prompt the game sent unasked, until the
//! line's own answer comes (`hunt/answer.rs`), over a scripted game that
//! speaks unasked as the real one does (`AnsweringSource`'s `say`).
//!
//! The regression is the author's hunt of 2026-09-30: a creature moving
//! between a `fire` and its reply ended the round trip at its own prompt;
//! the next tick decided from a state with no roundtime in it and sent
//! `fire` again (three times in 170 ms).

mod drive_support;
mod ready;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use cena_behavior::hunt::{Hunt, HuntEnd, Profile, hunt};
use cena_behavior::travel::TravelNotes;
use cena_behavior::watchdog::Heartbeat;
use cena_map::{Map, Room};
use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::{
    AuthorityToken, CommandId, Frame, GameState, Link, LinkKind, Origin, Run, Runs, Session,
};
use drive_support::PROMPT;
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

/// Let the paused clock run `millis`, yielding as it goes.
async fn pass(millis: u64) {
    for _ in 0..millis / 10 {
        tokio::time::advance(Duration::from_millis(10)).await;
        tokio::task::yield_now().await;
    }
}

/// Let the paused clock run until `n` lines starting `attack` are written.
async fn until_attacks(transcript: &TranscriptHandle, n: usize) -> bool {
    for _ in 0..2_000 {
        if attacks(transcript).len() >= n {
            return true;
        }
        pass(10).await;
    }
    false
}

fn attacks(transcript: &TranscriptHandle) -> Vec<String> {
    transcript
        .lines()
        .into_iter()
        .filter(|line| line.starts_with("attack"))
        .collect()
}

/// The first thing the game says after an attack, when it is not its
/// answer: a creature going by, ending in the prompt the game puts after
/// anything.
const PASSING: &[u8] = b"A gnarled kobold shaman just went through a narrow gap.\n<prompt time=\"1001\">&gt;</prompt>\n";

/// A hunt over a scripted game, in room 1 with a hostile kobold, its
/// attacks answered first by `answers`, in order. Returns the transcript,
/// the hunt's stop token and its task.
fn set_out(
    answers: &[&[u8]],
) -> (
    TranscriptHandle,
    CancellationToken,
    tokio::task::JoinHandle<Option<HuntEnd>>,
) {
    let (source, transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let (mut snapshot, events) = session.subscribe();
    let (_, ready) = session.subscribe();
    let state: &mut GameState = &mut snapshot.state;
    state.apply(&Frame::Prompt {
        time: "1000".into(),
        text: ">".into(),
    });
    state.room.id = Some("1001".into());
    state.status.set("standing", true);
    state.character.stance = Some("defensive (100%)".to_owned());
    for frame in kobold_frames() {
        state.apply(&frame);
    }
    state.targeting.read("#42", None);
    tokio::spawn(session.into_actor().run());

    transcript.answer("look", KOBOLD_ROOM);
    for answer in answers {
        transcript.answer("attack", answer);
    }
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
                (snapshot, events.into()),
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

    (transcript, stop, task)
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_prompt_the_game_sent_unasked_is_not_the_answer_and_the_attack_goes_once() {
    let (transcript, stop, task) = set_out(&[PASSING]);
    assert!(
        until_attacks(&transcript, 1).await,
        "hunting: {:?}",
        transcript.lines()
    );

    // Well inside the driver's wait for the answer, and long past the
    // prompt that closed the round trip.
    pass(1_000).await;
    assert_eq!(
        attacks(&transcript).len(),
        1,
        "the unasked prompt is not the answer: {:?}",
        transcript.lines()
    );

    // The reply, late, with the roundtime it states.
    transcript.say(
        b"You swing a broadsword at a kobold!\nRoundtime: 3 sec.\n<roundTime value=\"1004\"/><prompt time=\"1001\">&gt;</prompt>\n",
    );
    pass(1_000).await;
    assert_eq!(
        attacks(&transcript).len(),
        1,
        "inside the roundtime the reply stated, nothing more: {:?}",
        transcript.lines()
    );
    stop.cancel();
    let _ = task.await;
}

/// The crate review of 2026-10-01, BE-A-3: an attack whose answer never
/// came in the driver's wait is owed one, and a holding that comes after
/// the next attack went out may be the first's. It is taken as the first's:
/// the second is not sent again, since it may have gone through, and a line
/// sent twice is two attacks.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_late_holding_is_the_earlier_lines_and_the_next_is_not_sent_again() {
    let (transcript, stop, task) = set_out(&[PASSING, PASSING]);
    assert!(
        until_attacks(&transcript, 2).await,
        "the first unanswered, the second sent: {:?}",
        transcript.lines()
    );
    transcript.say(b"...wait 1 seconds.\n<prompt time=\"1001\">&gt;</prompt>\n");
    // Past the half second the driver would wait before sending it again,
    // and short of its own wait running out.
    pass(1_500).await;
    assert_eq!(
        attacks(&transcript).len(),
        2,
        "the holding was the first's: {:?}",
        transcript.lines()
    );
    stop.cancel();
    let _ = task.await;
}

/// Another player's words in an attack's window are not the game's
/// (BE-A-8): `"I tried my bow but it has no effect"` ended the hunt, as an
/// attack that cannot hurt what is here.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn another_players_no_effect_does_not_end_the_hunt() {
    let said: &[u8] = b"<preset id='speech'><a exist=\"-10007833\" noun=\"Pukk\">Pukk</a> says</preset>, \"I tried my bow but it has no effect.\"\n<prompt time=\"1001\">&gt;</prompt>\n";
    let (transcript, stop, task) = set_out(&[said]);
    assert!(
        until_attacks(&transcript, 1).await,
        "hunting: {:?}",
        transcript.lines()
    );
    pass(5_000).await;
    assert!(
        !task.is_finished(),
        "still hunting: {:?}",
        transcript.lines()
    );
    stop.cancel();
    let _ = task.await;
}
