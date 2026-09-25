//! The hunt driver looting with the planner (`plan/31` Stage 2), over a
//! scripted game: a corpse and a floor, the planner's steps sent through the
//! session in eloot's order, each reply read, and a full bag turned into a
//! reason to rest.

mod drive_support;
mod ready;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use cena_behavior::hunt::{Hunt, HuntEnd, Profile, hunt};
use cena_behavior::loot::LootProfile;
use cena_behavior::travel::TravelNotes;
use cena_behavior::watchdog::Heartbeat;
use cena_map::{Map, Room};
use cena_platform::AnsweringSource;
use cena_session::containers::{ContainerEvent, ItemRef, StowSlot};
use cena_session::{
    AuthorityToken, CommandId, Frame, GameState, Link, LinkKind, RoomItem, Run, Runs, Session,
};
use drive_support::{PROMPT, until_written};
use tokio_util::sync::CancellationToken;

const ROOMS: &str = r#"[{"id":1,"uid":[1001]},{"id":2,"uid":[1002]}]"#;

const PROFILE: &str = r#"
targets = [{ any = true, routine = "a" }]
[rooms]
hunting = 1
resting = 2
[loot]
delay = false
[routines]
a = ["attack"]
"#;

/// A dead warg in the room, as the wire states it.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only its bold depth matters"
)]
fn corpse(state: &mut GameState, id: i64) {
    let mut run = Run {
        text: "giant warg".to_owned(),
        style: Default::default(),
        link: Some(Link {
            kind: LinkKind::Exist {
                id: id.to_string(),
                noun: "warg".to_owned(),
            },
            text: "giant warg".to_owned(),
            coord: None,
        }),
        inner_link: None,
    };
    run.style.bold_depth = 1;
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs { runs: vec![run] },
    });
    state.apply(&Frame::CreatureStatus {
        id: id.to_string(),
        attrs: vec![
            ("exist".to_owned(), id.to_string()),
            ("hostile".to_owned(), "1".to_owned()),
            ("dead".to_owned(), "1".to_owned()),
        ],
    });
}

fn floor(id: &str, noun: &str, text: &str) -> RoomItem {
    RoomItem {
        id: id.to_owned(),
        noun: noun.to_owned(),
        text: text.to_owned(),
        before: None,
        after: None,
        status: None,
    }
}

/// A hunt over a scripted game, in room 1 with a dead warg and these things
/// on the floor, looting by `loot`. Returns the transcript and the hunt's
/// stop token.
fn set_out(
    floor_items: Vec<RoomItem>,
    loot: LootProfile,
) -> (
    cena_platform::TranscriptHandle,
    CancellationToken,
    tokio::task::JoinHandle<Option<HuntEnd>>,
) {
    let (source, transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let (mut snapshot, events) = session.subscribe();
    let (_, ready) = session.subscribe();
    let state = &mut snapshot.state;
    state.apply(&Frame::Prompt {
        time: "1000".into(),
        text: ">".into(),
    });
    state.room.id = Some("1001".into());
    state.status.set("standing", true);
    state.apply(&Frame::Component {
        id: "room players".into(),
        body: Runs { runs: Vec::new() },
    });
    state.character.stance = Some("defensive (100%)".to_owned());
    state.containers.apply(&ContainerEvent::StowListBegins);
    state.containers.apply(&ContainerEvent::StowSet {
        slot: StowSlot::Default,
        item: ItemRef {
            id: "902".to_owned(),
            noun: "backpack".to_owned(),
            text: "backpack".to_owned(),
        },
    });
    corpse(state, 42);
    state.room.objects = floor_items;
    tokio::spawn(session.into_actor().run());

    let stop = CancellationToken::new();
    let hunt_stop = stop.clone();
    let task = tokio::spawn(async move {
        ready::until_ready(ready).await.ok()?;
        let rooms: Vec<Room> = serde_json::from_str(ROOMS).ok()?;
        let map = Map::from_rooms(rooms).ok()?;
        let next = Arc::new(AtomicU64::new(0));
        let ids = move || CommandId(next.fetch_add(1, Ordering::Relaxed));
        let machine = Hunt::new(Profile::parse(PROFILE).ok()?, 1).with_loot(loot);
        handle.claim(AuthorityToken(1)).await.ok()?;
        let heartbeat = Heartbeat::default();
        let end = Box::pin(hunt(
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
        .await;
        Some(end)
    });
    (transcript, stop, task)
}

fn loot_profile() -> LootProfile {
    LootProfile {
        take: vec!["gem".to_owned(), "magic".to_owned()],
        defensive: true,
        ..LootProfile::default()
    }
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_corpse_is_searched_then_the_floor_looted_in_eloots_order() {
    let (transcript, stop, task) =
        set_out(vec![floor("7", "emerald", "uncut emerald")], loot_profile());
    transcript.answer(
        "loot #42",
        b"You search the giant warg.\nIt had nothing of interest.\n<prompt time=\"1001\">&gt;</prompt>\n",
    );
    transcript.answer(
        "loot room",
        b"You gather up the emerald.\n<prompt time=\"1002\">&gt;</prompt>\n",
    );
    assert!(
        until_written(&transcript, "loot room").await,
        "the floor is looted: {:?}",
        transcript.lines()
    );
    let lines = transcript.lines();
    let search = lines.iter().position(|l| l == "loot #42");
    let room = lines.iter().position(|l| l == "loot room");
    assert!(search < room, "the corpse first, then the floor: {lines:?}");
    stop.cancel();
    let _ = task.await;
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_full_bag_sends_the_hunt_to_rest() {
    // A whatsit of no kind is dragged into the default bag; the game says
    // it will not fit; nothing else will take it, so the hunt rests: it
    // walks to the resting room, and the walk is what the transcript shows.
    let (transcript, stop, task) = set_out(
        vec![
            floor("8", "whatsit", "peculiar glowing whatsit"),
            floor("9", "acantha", "acantha leaf"),
        ],
        loot_profile(),
    );
    transcript.answer(
        "loot #42",
        b"You search the giant warg.\n<prompt time=\"1001\">&gt;</prompt>\n",
    );
    transcript.answer(
        "_drag #8 #902",
        b"The whatsit won't fit in the backpack.\n<prompt time=\"1002\">&gt;</prompt>\n",
    );
    assert!(
        until_written(&transcript, "_drag #8 #902").await,
        "{:?}",
        transcript.lines()
    );
    // The rest that follows is a walk to room 2; with no exits in the map
    // the hunt ends as unreachable, which is the rest reason having fired.
    for _ in 0..200 {
        if task.is_finished() {
            break;
        }
        tokio::time::advance(Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
    }
    stop.cancel();
    let end = task.await.ok().flatten();
    assert!(
        matches!(
            end,
            Some(HuntEnd::Finished(cena_behavior::hunt::Ending::Unreachable(
                _
            )))
        ),
        "the full bag became a rest, and the rest a walk: {end:?}; sent {:?}",
        transcript.lines()
    );
}
