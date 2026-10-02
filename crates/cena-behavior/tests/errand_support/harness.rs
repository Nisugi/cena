//! The harness itself; `mod.rs` wires it.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cena_behavior::hunt::{FullBags, Hunt, HuntEnd, drive, hunt};
use cena_behavior::loot::{Errand, Learned, LootProfile};
use cena_behavior::town::Choice;
use cena_behavior::travel::TravelNotes;
use cena_behavior::watchdog::Heartbeat;
use cena_map::{Map, Room};
use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::containers::{ContainerEvent, ItemRef, StowSlot};
use cena_session::{
    AuthorityToken, CommandId, Event, Frame, GameState, Link, LinkKind, RoomItem, Run, Runs,
    Session,
};
use tokio::sync::broadcast::Receiver;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::drive_support::PROMPT;

/// An errand under way: what it writes, the task that ends with it, and
/// what it tells the player.
pub type Under = (
    TranscriptHandle,
    JoinHandle<Option<HuntEnd>>,
    Receiver<Event>,
);

/// A link to the thing `id`, a `noun` called `text`.
pub fn link(id: &str, noun: &str, text: &str) -> Link {
    Link {
        kind: LinkKind::Exist {
            id: id.to_owned(),
            noun: noun.to_owned(),
        },
        text: text.to_owned(),
        coord: None,
    }
}

/// A dead warg in the room, as the wire states it.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only its bold depth matters"
)]
pub fn corpse(state: &mut GameState, id: i64) {
    let mut run = Run {
        text: "giant warg".to_owned(),
        style: Default::default(),
        link: Some(link(&id.to_string(), "warg", "giant warg")),
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

/// These `(id, noun, name)` listed inside the container `bag`.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only the link matters"
)]
pub fn inside(state: &mut GameState, bag: &str, things: &[(&str, &str, &str)]) {
    state.apply(&Frame::Container {
        id: bag.to_owned(),
        title: Some("Backpack".to_owned()),
        target: None,
        attrs: Vec::new(),
    });
    for (id, noun, text) in things {
        state.apply(&Frame::ContainerItem {
            container_id: bag.to_owned(),
            content: Runs {
                runs: vec![Run {
                    text: (*text).to_owned(),
                    style: Default::default(),
                    link: Some(link(id, noun, text)),
                    inner_link: None,
                }],
            },
        });
    }
}

/// `errand` run by itself over a scripted game, on `rooms` by `profile`: in
/// room 1, a dead warg and `floor` at the character's feet, a backpack to
/// stow in, and the hands empty -- but for skinning, a dagger, and for the
/// ground's boxes, a broadsword to put away -- before `knows` says what else
/// the character knows. `learned` hears what a visit learns; `full` is the
/// bags known full, as one desk shares them.
pub fn set_out_knowing(
    errand: Errand,
    floor: Vec<RoomItem>,
    place: (&'static str, &'static str),
    learned: &Arc<Mutex<Vec<Learned>>>,
    full: &FullBags,
    knows: impl FnOnce(&mut GameState),
) -> Under {
    set_out_run((errand, Choice::All), floor, place, learned, full, knows)
}

/// `loot sell type|shop|item`: the selling round, only as much of it as
/// `choice` says, on `place` (rooms and profile), the character knowing
/// what `knows` says.
pub fn set_out_choosing(
    choice: Choice,
    place: (&'static str, &'static str),
    knows: impl FnOnce(&mut GameState),
) -> Under {
    set_out_run(
        (Errand::Sell, choice),
        Vec::new(),
        place,
        &Arc::default(),
        &FullBags::default(),
        knows,
    )
}

/// [`set_out_knowing`] and [`set_out_choosing`] both.
fn set_out_run(
    (errand, choice): (Errand, Choice),
    floor: Vec<RoomItem>,
    (rooms, profile): (&'static str, &'static str),
    learned: &Arc<Mutex<Vec<Learned>>>,
    full: &FullBags,
    knows: impl FnOnce(&mut GameState),
) -> Under {
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
    let holding = |noun: &str, name: &str| Frame::RightHand {
        item: name.to_owned(),
        link: Some(link("11", noun, name)),
    };
    state.apply(&match errand {
        Errand::Skin => holding("dagger", "dagger"),
        Errand::Ground => holding("broadsword", "steel broadsword"),
        _ => Frame::RightHand {
            item: "Empty".to_owned(),
            link: None,
        },
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
    knows(state);
    tokio::spawn(session.into_actor().run());

    let stop = CancellationToken::new();
    let kept = Arc::clone(learned);
    let full = Arc::clone(full);
    let task = tokio::spawn(async move {
        crate::ready::until_ready(ready).await.ok()?;
        let rooms: Vec<Room> = serde_json::from_str(rooms).ok()?;
        let map = Map::from_rooms(rooms).ok()?;
        let next = Arc::new(AtomicU64::new(0));
        let ids = move || CommandId(next.fetch_add(1, Ordering::Relaxed));
        let machine = Hunt::loot_only(LootProfile::parse(profile).ok()?, errand)
            .with_full_bags(full)
            .with_choice(choice);
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
            move |told: drive::Learned<'_>| {
                if let drive::Learned::Loot(learned) = told
                    && let Ok(mut held) = kept.lock()
                {
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
pub async fn ended(task: JoinHandle<Option<HuntEnd>>) -> Option<HuntEnd> {
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
pub fn at(lines: &[String], line: &str) -> Option<usize> {
    lines.iter().position(|written| written == line)
}

/// Whether every one of `expected` was written, each after the one before
/// it: a line written twice is looked for after the first.
pub fn in_order(lines: &[String], expected: &[&str]) -> bool {
    let mut from = 0;
    for line in expected {
        match lines[from..].iter().position(|written| written == line) {
            Some(found) => from += found + 1,
            None => return false,
        }
    }
    true
}

/// These `(id, noun, name)` on the floor.
pub fn floor(things: &[(&str, &str, &str)]) -> Vec<RoomItem> {
    things
        .iter()
        .map(|(id, noun, text)| RoomItem {
            id: (*id).to_owned(),
            noun: (*noun).to_owned(),
            text: (*text).to_owned(),
            before: None,
            after: None,
            status: None,
        })
        .collect()
}

/// `text` and the prompt after it, as the game sends a reply.
pub fn reply(text: &str) -> Vec<u8> {
    format!("{text}\n<prompt time=\"1001\">&gt;</prompt>\n").into_bytes()
}
