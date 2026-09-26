//! Two members' hunts over two scripted sessions and one set of boards
//! (`plan/39` Stage 3): what the leader publishes reaches the follower's
//! engine, and the follower sends on its own session only. The engine's
//! rules are `group_engine.rs`'s; this is the plumbing between them.
mod drive_support;
mod ready;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use cena_behavior::group::{Boards, Place};
use cena_behavior::hunt::{Hunt, HuntEnd, Profile, hunt_in};
use cena_behavior::travel::TravelNotes;
use cena_behavior::watchdog::Heartbeat;
use cena_map::Map;
use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::group::{GroupEvent, Member};
use cena_session::{
    AuthorityToken, CommandId, Frame, GameState, Link, LinkKind, Origin, Run, Runs, Session,
};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

const LEADER: &str = r#"
targets = [{ name = "troll", routine = "a" }, { any = true, routine = "a" }]
[rooms]
hunting = 1
resting = 2
[routines]
a = ["attack"]
[group]
ma_looter = "Kiyna"
"#;

const FOLLOWER: &str = r#"
targets = [{ name = "warg", routine = "a" }, { any = true, routine = "a" }]
[rooms]
hunting = 1
resting = 2
[routines]
a = ["kick"]
"#;

/// The room as the game says it, the answer to every command: the
/// session's own model must hold a creature for the write-time gate to let
/// a line aimed at it go (`hunt_reconnect.rs`, `KOBOLD_ROOM`).
const ROOM: &[u8] = b"<component id='room players'></component>
<component id='room objs'>You also see <pushBold/>a <a exist=\"41\" noun=\"warg\">warg</a><popBold/>, <pushBold/>a <a exist=\"42\" noun=\"troll\">troll</a><popBold/> and <pushBold/>a <a exist=\"43\" noun=\"warg\">warg</a><popBold/>.</component>
<crtrStatus exist=\"41\" hostile=\"1\"/>
<crtrStatus exist=\"42\" hostile=\"1\"/>
<crtrStatus exist=\"43\" hostile=\"1\" dead=\"1\"/>
<prompt time=\"1001\">&gt;</prompt>
";

fn member(noun: &str) -> Member {
    Member {
        id: format!("-10{}", noun.len()),
        noun: noun.to_owned(),
        text: noun.to_owned(),
    }
}

#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only its bold depth matters"
)]
fn room_with(state: &mut GameState, creatures: &[(i64, &str, bool)]) {
    let runs = creatures
        .iter()
        .map(|(id, noun, _)| {
            let mut run = Run {
                text: (*noun).to_owned(),
                style: Default::default(),
                link: Some(Link {
                    kind: LinkKind::Exist {
                        id: id.to_string(),
                        noun: (*noun).to_owned(),
                    },
                    text: (*noun).to_owned(),
                    coord: None,
                }),
                inner_link: None,
            };
            run.style.bold_depth = 1;
            run
        })
        .collect();
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs { runs },
    });
    for (id, _, dead) in creatures {
        let mut attrs = vec![
            ("exist".to_owned(), id.to_string()),
            ("hostile".to_owned(), "1".to_owned()),
        ];
        if *dead {
            attrs.push(("dead".to_owned(), "1".to_owned()));
        }
        state.apply(&Frame::CreatureStatus {
            id: id.to_string(),
            attrs,
        });
    }
}

/// A character standing in the game's room 1000 with a warg, a troll and a
/// warg's corpse, its group as `grouped` leaves it.
fn start(name: &str, grouped: &GroupEvent, state: &mut GameState) {
    state.character.name = Some(name.to_owned());
    state.room.id = Some("1000".into());
    state.status.set("standing", true);
    state.character.experience.mind_percent = Some(0);
    state.apply(&Frame::Component {
        id: "room players".into(),
        body: Runs { runs: Vec::new() },
    });
    room_with(
        state,
        &[
            (41, "warg", false),
            (42, "troll", false),
            (43, "warg", true),
        ],
    );
    state.group.apply(grouped, None);
}

/// One member's hunt over its own scripted session.
fn member_hunt(
    name: &'static str,
    profile: &'static str,
    grouped: &GroupEvent,
    place: Place,
    boards: &Arc<Boards>,
    stop: &CancellationToken,
) -> (
    TranscriptHandle,
    JoinHandle<Option<HuntEnd>>,
    CancellationToken,
) {
    let (source, transcript) = AnsweringSource::logged_in(ROOM);
    let session = Session::new(source);
    let handle = session.handle();
    let stop_session = session.cancel_token();
    let (mut snapshot, events) = session.subscribe();
    let (_, ready) = session.subscribe();
    start(name, grouped, &mut snapshot.state);
    tokio::spawn(session.into_actor().run());
    let (boards, cancel) = (Arc::clone(boards), stop.clone());
    let task = tokio::spawn(async move {
        ready::until_ready(ready).await.ok()?;
        // The session's own model learns the room, as a look teaches it.
        let _ = handle
            .send_and_await(
                CommandId(900),
                "look",
                Origin::Manual,
                Duration::from_secs(5),
                |frame: &Frame| matches!(frame, Frame::Prompt { .. }),
            )
            .await;
        let rooms = serde_json::from_str(r#"[{"id":1,"uid":[1000]},{"id":2,"uid":[1002]}]"#);
        let map = Map::from_rooms(rooms.ok()?).ok()?;
        let next = Arc::new(AtomicU64::new(0));
        let ids = move || CommandId(next.fetch_add(1, Ordering::Relaxed));
        handle.claim(AuthorityToken(3)).await.ok()?;
        let machine = Hunt::new(Profile::parse(profile).ok()?, 1);
        let end = Box::pin(hunt_in(
            &handle,
            &cancel,
            ids,
            AuthorityToken(3),
            (snapshot, events.into()),
            &map,
            machine,
            &Heartbeat::default(),
            TravelNotes::default(),
            |_| {},
            |_| {},
            Some((boards, place)),
        ))
        .await;
        Some(end)
    });
    (transcript, task, stop_session)
}

/// `plan/39` §5: the follower fights the leader's target over its own
/// first choice, and loots the corpse the leader's `ma_looter` gives it;
/// the leader loots nothing and fights on.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_follower_takes_the_leaders_target_and_the_loot_it_is_given() {
    let boards = Boards::new();
    let stop = CancellationToken::new();
    let (leader, leader_task, leader_session) = member_hunt(
        "Ashryn",
        LEADER,
        &GroupEvent::Listed {
            leading: true,
            members: vec![member("Kiyna")],
        },
        Place::Read,
        &boards,
        &stop,
    );
    let (follower, follower_task, follower_session) = member_hunt(
        "Kiyna",
        FOLLOWER,
        &GroupEvent::JoinedGroup(member("Ashryn")),
        Place::Read,
        &boards,
        &stop,
    );
    let looted = drive_support::until_written(&follower, "loot #43").await;
    let assisted = drive_support::until_written(&follower, "target #42").await;
    stop.cancel();
    let _ = leader_task.await;
    let _ = follower_task.await;
    leader_session.cancel();
    follower_session.cancel();
    let (written_by_leader, written_by_follower) = (leader.lines(), follower.lines());
    assert!(
        looted,
        "the follower was named the looter: {written_by_follower:?}"
    );
    assert!(
        assisted,
        "the follower takes the leader's target: {written_by_follower:?} / leader: {written_by_leader:?}"
    );
    assert!(
        written_by_leader.contains(&"target #42".to_owned()),
        "{written_by_leader:?}"
    );
    assert!(
        !written_by_leader.contains(&"loot #43".to_owned()),
        "the leader does not loot what it gave away: {written_by_leader:?}"
    );
    assert!(
        !written_by_follower.iter().any(|line| line == "attack"),
        "the follower sends its own routine, on its own session: {written_by_follower:?}"
    );
}

/// `plan/39` §8, question 3: the leader's hunt stopped while it led, the
/// follower's is over too.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_leaders_stop_ends_the_followers_hunt() {
    let boards = Boards::new();
    let leader_stop = CancellationToken::new();
    let follower_stop = CancellationToken::new();
    let (leader, leader_task, leader_session) = member_hunt(
        "Ashryn",
        LEADER,
        &GroupEvent::Listed {
            leading: true,
            members: vec![member("Kiyna")],
        },
        Place::Read,
        &boards,
        &leader_stop,
    );
    let (follower, follower_task, follower_session) = member_hunt(
        "Kiyna",
        FOLLOWER,
        &GroupEvent::JoinedGroup(member("Ashryn")),
        Place::Read,
        &boards,
        &follower_stop,
    );
    assert!(drive_support::until_written(&leader, "target #42").await);
    assert!(drive_support::until_written(&follower, "loot #43").await);
    leader_stop.cancel();
    let _ = leader_task.await;
    let mut ended = None;
    for _ in 0..200 {
        if follower_task.is_finished() {
            ended = follower_task.await.ok().flatten();
            break;
        }
        tokio::time::advance(Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
    }
    follower_stop.cancel();
    leader_session.cancel();
    follower_session.cancel();
    assert_eq!(
        ended,
        Some(HuntEnd::Finished(
            cena_behavior::hunt::Ending::LeaderStopped
        )),
        "{:?}",
        follower.lines()
    );
}

/// `plan/39` §8, question 2: `hunt <name> with Kiyna` starts Kiyna's hunt
/// following the leader, though the game's group does not hold it yet: it
/// joins; the leader opens its group and waits for it.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_named_follower_joins_the_leader_it_was_given() {
    let boards = Boards::new();
    let stop = CancellationToken::new();
    let (leader, leader_task, leader_session) = member_hunt(
        "Ashryn",
        LEADER,
        &GroupEvent::NotInGroup,
        Place::Lead(vec!["Kiyna".to_owned()]),
        &boards,
        &stop,
    );
    let (follower, follower_task, follower_session) = member_hunt(
        "Kiyna",
        FOLLOWER,
        &GroupEvent::NotInGroup,
        Place::Follow("Ashryn".to_owned()),
        &boards,
        &stop,
    );
    let opened = drive_support::until_written(&leader, "group open").await;
    let joined = drive_support::until_written(&follower, "join Ashryn").await;
    stop.cancel();
    let _ = leader_task.await;
    let _ = follower_task.await;
    leader_session.cancel();
    follower_session.cancel();
    assert!(opened, "{:?}", leader.lines());
    assert!(joined, "{:?}", follower.lines());
}

/// Question 3: a follower's own stop leaves the game's group.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_followers_stop_leaves_the_group() {
    let boards = Boards::new();
    let leader_stop = CancellationToken::new();
    let follower_stop = CancellationToken::new();
    let (leader, leader_task, leader_session) = member_hunt(
        "Ashryn",
        LEADER,
        &GroupEvent::Listed {
            leading: true,
            members: vec![member("Kiyna")],
        },
        Place::Read,
        &boards,
        &leader_stop,
    );
    let (follower, follower_task, follower_session) = member_hunt(
        "Kiyna",
        FOLLOWER,
        &GroupEvent::JoinedGroup(member("Ashryn")),
        Place::Read,
        &boards,
        &follower_stop,
    );
    assert!(drive_support::until_written(&follower, "loot #43").await);
    follower_stop.cancel();
    let _ = follower_task.await;
    leader_stop.cancel();
    let _ = leader_task.await;
    leader_session.cancel();
    follower_session.cancel();
    assert!(
        follower.lines().contains(&"leave group".to_owned()),
        "{:?}",
        follower.lines()
    );
    assert!(!leader.lines().contains(&"leave group".to_owned()));
}
