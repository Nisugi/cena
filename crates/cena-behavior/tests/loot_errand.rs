//! Loot's parts run by themselves, with no hunt around them (`plan/61` step
//! 1), over a scripted game: `loot`, `loot skin` and `loot deposit` as
//! one-shot errands that do their work and end. The bank trip is the first
//! time the selling round's driver meets a scripted game: it walks there,
//! deposits, keeps what the profile says and walks back.

mod drive_support;
mod ready;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cena_behavior::hunt::{Ending, FullBags, Hunt, HuntEnd, hunt};
use cena_behavior::loot::{Errand, Learned, LootProfile};
use cena_behavior::travel::TravelNotes;
use cena_behavior::watchdog::Heartbeat;
use cena_map::{Map, Room};
use cena_platform::AnsweringSource;
use cena_session::containers::{ContainerEvent, ItemRef, StowSlot};
use cena_session::{
    AuthorityToken, CommandId, Frame, GameState, Link, LinkKind, RoomItem, Run, Runs, Session,
};
use drive_support::{PROMPT, arrival};
use tokio_util::sync::CancellationToken;

/// Room 1, and a bank one step north of it.
const ROOMS: &str = r#"[
  {"id":1,"uid":[1001],"exits":[{"to":2,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":2,"uid":[1002],"tags":["bank"],"exits":[{"to":1,"kind":"cardinal","cmd":"south","cost":1}]}
]"#;

const PROFILE: &str = "take = [\"gem\", \"magic\"]\n\n[town]\nsell_keep_silver = 500\n";

/// Room 1 between a town's bank, north, and Mist Harbor's, south.
const TWO_BANKS: &str = r#"[
  {"id":1,"uid":[1001],"location":"Wehnimer's Landing","exits":[
    {"to":2,"kind":"cardinal","cmd":"north","cost":1},
    {"to":3,"kind":"cardinal","cmd":"south","cost":1}]},
  {"id":2,"uid":[1002],"tags":["bank"],"location":"Wehnimer's Landing","exits":[
    {"to":1,"kind":"cardinal","cmd":"south","cost":1}]},
  {"id":3,"uid":[1003],"tags":["bank"],"location":"Mist Harbor","exits":[
    {"to":1,"kind":"cardinal","cmd":"north","cost":1}]}
]"#;

const SELLS_IN_FWI: &str = "take = [\"gem\"]\n\n[town]\nsell_fwi = true\n";

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

/// `errand` run by itself over a scripted game: in room 1, a dead warg and
/// `floor` at the character's feet, empty hands, a backpack to stow in.
fn set_out(
    errand: Errand,
    floor: Vec<RoomItem>,
) -> (
    cena_platform::TranscriptHandle,
    tokio::task::JoinHandle<Option<HuntEnd>>,
    tokio::sync::broadcast::Receiver<cena_session::Event>,
) {
    set_out_in(
        errand,
        floor,
        (ROOMS, PROFILE),
        &Arc::default(),
        &FullBags::default(),
    )
}

/// [`set_out`], on this map and by this loot profile, with the bags known
/// full shared as one desk shares them.
fn set_out_in(
    errand: Errand,
    floor: Vec<RoomItem>,
    (rooms, profile): (&'static str, &'static str),
    learned: &Arc<Mutex<Vec<Learned>>>,
    full: &FullBags,
) -> (
    cena_platform::TranscriptHandle,
    tokio::task::JoinHandle<Option<HuntEnd>>,
    tokio::sync::broadcast::Receiver<cena_session::Event>,
) {
    let (source, transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let (mut snapshot, events) = session.subscribe();
    let (_, ready) = session.subscribe();
    let (_, told) = session.subscribe();
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
    // Skinning is done with what the right hand holds, when the profile
    // names no weapon.
    state.apply(&if errand == Errand::Skin {
        Frame::RightHand {
            item: "dagger".to_owned(),
            link: Some(Link {
                kind: LinkKind::Exist {
                    id: "11".to_owned(),
                    noun: "dagger".to_owned(),
                },
                text: "dagger".to_owned(),
                coord: None,
            }),
        }
    } else {
        Frame::RightHand {
            item: "Empty".to_owned(),
            link: None,
        }
    });
    state.apply(&Frame::LeftHand {
        item: "Empty".to_owned(),
        link: None,
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
    state.room.objects = floor;
    tokio::spawn(session.into_actor().run());

    let stop = CancellationToken::new();
    let kept = Arc::clone(learned);
    let full = Arc::clone(full);
    let task = tokio::spawn(async move {
        ready::until_ready(ready).await.ok()?;
        let rooms: Vec<Room> = serde_json::from_str(rooms).ok()?;
        let map = Map::from_rooms(rooms).ok()?;
        let next = Arc::new(AtomicU64::new(0));
        let ids = move || CommandId(next.fetch_add(1, Ordering::Relaxed));
        let machine =
            Hunt::loot_only(LootProfile::parse(profile).ok()?, errand).with_full_bags(full);
        handle.claim(AuthorityToken(1)).await.ok()?;
        let heartbeat = Heartbeat::default();
        let end = Box::pin(hunt(
            &handle,
            &stop,
            ids,
            AuthorityToken(1),
            (snapshot, events.into()),
            &map,
            machine,
            &heartbeat,
            TravelNotes::default(),
            |_| {},
            move |learned: &Learned| {
                if let Ok(mut held) = kept.lock() {
                    held.push(learned.clone());
                }
            },
        ))
        .await;
        Some(end)
    });
    (transcript, task, told)
}

/// The errand's ending, once it has one.
async fn ended(task: tokio::task::JoinHandle<Option<HuntEnd>>) -> Option<HuntEnd> {
    for _ in 0..400 {
        if task.is_finished() {
            break;
        }
        tokio::time::advance(Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
    }
    if !task.is_finished() {
        task.abort();
        return None;
    }
    task.await.ok().flatten()
}

/// Where `line` was written, among everything written.
fn at(lines: &[String], line: &str) -> Option<usize> {
    lines.iter().position(|written| written == line)
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn loot_alone_searches_the_dead_takes_the_floor_and_ends() {
    let emerald = RoomItem {
        id: "7".to_owned(),
        noun: "emerald".to_owned(),
        text: "uncut emerald".to_owned(),
        before: None,
        after: None,
        status: None,
    };
    let (transcript, task, _) = set_out(Errand::Room, vec![emerald]);
    transcript.answer(
        "loot #42",
        b"You search the giant warg.\nIt had nothing of interest.\n<prompt time=\"1001\">&gt;</prompt>\n",
    );
    transcript.answer(
        "loot room",
        b"You gather up the emerald.\n<prompt time=\"1002\">&gt;</prompt>\n",
    );
    let end = ended(task).await;
    let lines = transcript.lines();
    assert_eq!(
        end,
        Some(HuntEnd::Finished(Ending::Looted(Errand::Room))),
        "{lines:?}"
    );
    let (search, room) = (at(&lines, "loot #42"), at(&lines, "loot room"));
    assert!(search.is_some() && search < room, "{lines:?}");
    assert!(
        !lines.iter().any(|line| line == "attack"),
        "an errand is not a hunt: {lines:?}"
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn loot_skin_skins_and_searches_nothing() {
    let (transcript, task, _) = set_out(Errand::Skin, Vec::new());
    let end = ended(task).await;
    let lines = transcript.lines();
    assert_eq!(
        end,
        Some(HuntEnd::Finished(Ending::Looted(Errand::Skin))),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(|line| line.starts_with("skin #42")),
        "skinned though the profile's skin.enable is off: {lines:?}"
    );
    assert_eq!(at(&lines, "loot #42"), None, "{lines:?}");
    assert_eq!(at(&lines, "loot room"), None, "{lines:?}");
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn loot_deposit_walks_to_the_bank_keeps_the_silver_and_comes_back() {
    let (transcript, task, mut told) = set_out(Errand::Deposit, Vec::new());
    transcript.answer("north", &arrival(1002));
    transcript.answer("south", &arrival(1001));
    transcript.answer(
        "deposit all",
        b"You deposit 12,340 silvers into your account.\n<prompt time=\"1003\">&gt;</prompt>\n",
    );
    let end = ended(task).await;
    let lines = transcript.lines();
    assert_eq!(
        end,
        Some(HuntEnd::Finished(Ending::Looted(Errand::Deposit))),
        "{lines:?}"
    );
    let order: Vec<Option<usize>> = ["north", "deposit all", "withdraw 500 silver", "south"]
        .iter()
        .map(|line| at(&lines, line))
        .collect();
    assert!(order.iter().all(Option::is_some), "{lines:?}");
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{lines:?}");
    assert_eq!(at(&lines, "loot #42"), None, "the dead are left alone");
    // What the round came to is said at its end (`plan/61` step 2), from
    // the game's own line.
    let said = drive_support::told_so_far(&mut told);
    assert!(
        said.iter()
            .any(|(_, text)| text.contains("the round came to")
                && text.contains("Bank: deposited 12,340.")),
        "{said:?}"
    );
}

/// The author's round: *"I do sell in fwi"* (`plan/61` section 7 item 5). With
/// `sell_fwi`, the bank is Mist Harbor's though the town's is as near.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_profile_that_sells_in_mist_harbor_banks_there() {
    let (transcript, task, _) = set_out_in(
        Errand::Deposit,
        Vec::new(),
        (TWO_BANKS, SELLS_IN_FWI),
        &Arc::default(),
        &FullBags::default(),
    );
    transcript.answer("south", &arrival(1003));
    transcript.answer("north", &arrival(1001));
    let end = ended(task).await;
    let lines = transcript.lines();
    assert_eq!(
        end,
        Some(HuntEnd::Finished(Ending::Looted(Errand::Deposit))),
        "{lines:?}"
    );
    let order: Vec<Option<usize>> = ["south", "deposit all", "north"]
        .iter()
        .map(|line| at(&lines, line))
        .collect();
    assert!(order.iter().all(Option::is_some), "{lines:?}");
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{lines:?}");
}

/// What a visit learns reaches whoever writes the profile: a thing that
/// crumbles as it is stowed is named, as eloot saves it (`eloot.lic:4149-4153`).
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_thing_that_crumbles_is_told_to_the_profile() {
    let learned = Arc::default();
    let (transcript, task, _) = set_out_in(
        Errand::Room,
        whatsit_and_leaf(),
        (ROOMS, PROFILE),
        &learned,
        &FullBags::default(),
    );
    transcript.answer("loot #42", SEARCHED);
    transcript.answer(
        "_drag #3 #902",
        b"The peculiar glowing whatsit crumbles and decays away.\n<prompt time=\"1002\">&gt;</prompt>\n",
    );
    let end = ended(task).await;
    let lines = transcript.lines();
    assert_eq!(
        end,
        Some(HuntEnd::Finished(Ending::Looted(Errand::Room))),
        "{lines:?}"
    );
    let told = learned.lock().map(|held| held.clone()).unwrap_or_default();
    assert_eq!(told.len(), 1, "{told:?}");
    assert_eq!(told[0].crumbly, ["peculiar glowing whatsit"]);
}

const SEARCHED: &[u8] = b"You search the giant warg.\n<prompt time=\"1001\">&gt;</prompt>\n";

const WONT_FIT: &[u8] =
    b"The peculiar glowing whatsit won't fit in the backpack.\n<prompt time=\"1002\">&gt;</prompt>\n";

/// A whatsit to drag into the backpack, and a leaf nobody wants: the floor
/// goes item by item, not by `loot room`.
fn whatsit_and_leaf() -> Vec<RoomItem> {
    [
        ("3", "whatsit", "peculiar glowing whatsit"),
        ("4", "acantha", "acantha leaf"),
    ]
    .into_iter()
    .map(|(id, noun, text)| RoomItem {
        id: id.to_owned(),
        noun: noun.to_owned(),
        text: text.to_owned(),
        before: None,
        after: None,
        status: None,
    })
    .collect()
}

/// The bags known full, as the desk holds them between runs.
fn held(full: &FullBags) -> Vec<String> {
    full.lock()
        .map(|set| set.iter().cloned().collect())
        .unwrap_or_default()
}

/// A bag found full stays so for the desk's next `loot` (`track_full_sacks`)
/// until a trip to town frees it, as eloot clears `sacks_full` around every
/// round (`eloot.lic:7996`, `:8006`).
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_bag_found_full_stays_full_for_the_next_loot_until_a_round() {
    let full = FullBags::default();
    let room = (ROOMS, PROFILE);
    let (transcript, task, _) = set_out_in(
        Errand::Room,
        whatsit_and_leaf(),
        room,
        &Arc::default(),
        &full,
    );
    transcript.answer("loot #42", SEARCHED);
    transcript.answer("_drag #3 #902", WONT_FIT);
    let end = ended(task).await;
    let lines = transcript.lines();
    assert_eq!(
        end,
        Some(HuntEnd::Finished(Ending::Looted(Errand::Room))),
        "{lines:?}"
    );
    assert_eq!(held(&full), ["902"], "{lines:?}");

    let (transcript, task, _) = set_out_in(
        Errand::Room,
        whatsit_and_leaf(),
        room,
        &Arc::default(),
        &full,
    );
    transcript.answer("loot #42", SEARCHED);
    let _ = ended(task).await;
    let lines = transcript.lines();
    assert_eq!(
        at(&lines, "_drag #3 #902"),
        None,
        "the backpack is still full: {lines:?}"
    );

    let (transcript, task, _) =
        set_out_in(Errand::Deposit, Vec::new(), room, &Arc::default(), &full);
    transcript.answer("north", &arrival(1002));
    transcript.answer("south", &arrival(1001));
    let _ = ended(task).await;
    assert!(held(&full).is_empty(), "{:?}", transcript.lines());
}

/// With `track_full` off, each visit tries every bag again
/// (`eloot.lic:7911-7914`).
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn without_track_full_the_next_loot_tries_the_full_bag_again() {
    const FORGETS: &str = "take = [\"gem\", \"magic\"]\ntrack_full = false\n";
    let full = FullBags::default();
    for _ in 0..2 {
        let (transcript, task, _) = set_out_in(
            Errand::Room,
            whatsit_and_leaf(),
            (ROOMS, FORGETS),
            &Arc::default(),
            &full,
        );
        transcript.answer("loot #42", SEARCHED);
        transcript.answer("_drag #3 #902", WONT_FIT);
        let _ = ended(task).await;
        let lines = transcript.lines();
        assert!(at(&lines, "_drag #3 #902").is_some(), "{lines:?}");
    }
}

/// A bag the round sells from is opened and looked in when its contents are
/// not listed (`open_single_container`, `eloot.lic:3892-3921`), and with
/// `keep_closed` closed again at the end (`close_sell_containers`,
/// `:3737-3746`).
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_rounds_bags_are_looked_in_first_and_closed_after_when_kept_closed() {
    const KEEPS_CLOSED: &str =
        "take = [\"gem\", \"magic\"]\nkeep_closed = true\n\n[town]\nsell_keep_silver = 500\n";
    let (transcript, task, _) = set_out_in(
        Errand::Deposit,
        Vec::new(),
        (ROOMS, KEEPS_CLOSED),
        &Arc::default(),
        &FullBags::default(),
    );
    transcript.answer("north", &arrival(1002));
    transcript.answer("south", &arrival(1001));
    let end = ended(task).await;
    let lines = transcript.lines();
    assert_eq!(
        end,
        Some(HuntEnd::Finished(Ending::Looted(Errand::Deposit))),
        "{lines:?}"
    );
    let order: Vec<Option<usize>> = [
        "open #902",
        "look in #902",
        "north",
        "deposit all",
        "south",
        "close #902",
    ]
    .iter()
    .map(|line| at(&lines, line))
    .collect();
    assert!(order.iter().all(Option::is_some), "{lines:?}");
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{lines:?}");
}
