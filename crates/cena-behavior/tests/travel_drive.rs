//! The driver: `cena_behavior::travel::travel` (`plan/24` stage 4c), over a
//! real session and a scripted game.
//!
//! Virtual time throughout, for the reason `stop_and_interleave.rs` gives:
//! what is measured is how many awaits a stop had to cross, not the machine.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use cena_behavior::BehaviorError;
use cena_behavior::travel::{
    Ended, FOLLOW_WAIT, LOST_WAIT, TravelNotes, Travelled, Why, seed_for, travel,
};
use cena_map::{Map, Room, RoomId};
use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::group::{GroupEvent, Member};
use cena_session::hands::Hand;
use cena_session::{
    AuthorityToken, CommandId, Event, Frame, GameState, Gate, NoticeKind, Origin, Session,
    SessionHandle,
};
use tokio::sync::broadcast::Receiver;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

/// `plan/12` §4.3.
const PREEMPT_GRACE: Duration = Duration::from_millis(250);

/// What the game says to a command nothing was scripted for: nothing, and a
/// prompt. The character does not move.
const PROMPT: &[u8] = b"<prompt time=\"1\">&gt;</prompt>\n";

/// ```text
///   1 --north-- 2 --(hands emptied) climb rope (hands filled)-- 3
/// ```
/// The game's numbers are the ids plus a thousand, so a test that confused
/// the two would not find its room.
const ROOMS: &str = r#"[
  {"id":1,"uid":[1001],"exits":[{"to":2,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":2,"uid":[1002],"exits":[{"to":3,"kind":"scripted","cost":1,
     "steps":[{"empty_hands":null},{"move":"climb rope"},{"fill_hands":null}]}]},
  {"id":3,"uid":[1003]}
]"#;

fn arrival(uid: u32) -> Vec<u8> {
    format!("<nav rm='{uid}'/>\n<prompt time=\"2\">&gt;</prompt>\n").into_bytes()
}

const REFUSED: &[u8] = b"You can't go there.\n<prompt time=\"2\">&gt;</prompt>\n";
const SWORD_GONE: &[u8] = b"<right>Empty</right>\n<prompt time=\"2\">&gt;</prompt>\n";
const SWORD_BACK: &[u8] =
    b"<right exist=\"11\" noun=\"sword\">broadsword</right>\n<prompt time=\"3\">&gt;</prompt>\n";

/// A character in room 1 with a broadsword, walking to room 3.
fn set_out(
    stop: &CancellationToken,
    rooms: &'static str,
) -> (
    JoinHandle<Option<Travelled>>,
    TranscriptHandle,
    CancellationToken,
) {
    let (walk, transcript, session, _) = set_out_with(stop, rooms, &[]);
    (walk, transcript, session)
}

/// [`set_out`], grouped with `company`, and with the handle a player would
/// type into: the only way a test can make the game say something the walker
/// did not ask for.
fn set_out_with(
    stop: &CancellationToken,
    rooms: &'static str,
    company: &[Member],
) -> (
    JoinHandle<Option<Travelled>>,
    TranscriptHandle,
    CancellationToken,
    SessionHandle,
) {
    let (walk, transcript, session, typed, _) = set_out_as(stop, rooms, |state| {
        for member in company {
            state.group.apply(&GroupEvent::Joined(member.clone()));
        }
    });
    (walk, transcript, session, typed)
}

/// [`set_out`], with whatever else the character knows as it sets out.
fn set_out_as(
    stop: &CancellationToken,
    rooms: &'static str,
    knows: impl FnOnce(&mut GameState),
) -> (
    JoinHandle<Option<Travelled>>,
    TranscriptHandle,
    CancellationToken,
    SessionHandle,
    Receiver<Event>,
) {
    let (source, transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let session_cancel = session.cancel_token();
    let (mut snapshot, events) = session.subscribe();
    // A second listener, as a frontend would be: what the player is told.
    let (_, told) = session.subscribe();
    snapshot.state.room.id = Some("1001".into());
    snapshot.state.right_hand = Hand::Holding {
        id: Some("11".into()),
        noun: Some("sword".into()),
        name: "broadsword".into(),
    };
    snapshot.state.left_hand = Hand::Empty;
    knows(&mut snapshot.state);
    let typed = handle.clone();
    tokio::spawn(session.into_actor().run());

    let stop = stop.clone();
    let walk = tokio::spawn(async move {
        // `None` is a broken fixture, which every test unwraps into a failure.
        let rooms: Vec<Room> = serde_json::from_str(rooms).ok()?;
        let map = Map::from_rooms(rooms).ok()?;
        let next = Arc::new(AtomicU64::new(0));
        let ids = move || CommandId(next.fetch_add(1, Ordering::Relaxed));
        let mut notes = TravelNotes::default();
        let travelled = travel(
            &handle,
            &stop,
            ids,
            AuthorityToken(1),
            (snapshot, events),
            &map,
            RoomId(3),
            &mut notes,
            |_| {},
        )
        .await;
        Some(travelled)
    });
    (walk, transcript, session_cancel, typed, told)
}

/// Let virtual time run until `line` has been written. `false` if it never is.
async fn until_written(transcript: &TranscriptHandle, line: &str) -> bool {
    for _ in 0..200 {
        if transcript.lines().iter().any(|written| written == line) {
            return true;
        }
        tokio::time::advance(Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
    }
    false
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn it_walks_there_storing_and_taking_back_on_the_way() {
    let stop = CancellationToken::new();
    let (walk, transcript, session) = set_out(&stop, ROOMS);
    transcript.answer("north", &arrival(1002));
    transcript.answer("store right", SWORD_GONE);
    transcript.answer("climb rope", &arrival(1003));
    transcript.answer("get #11", SWORD_BACK);

    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Arrived);
    assert_eq!(
        transcript.lines(),
        ["north", "store right", "climb rope", "get #11"]
    );
    assert!(travelled.still_stored.is_empty(), "the sword came back");
    session.cancel();
}

/// The author's ruling, 2026-09-21: a stop sends one command per stored item
/// and then stops. Here the rope is never climbed, so the stop lands with the
/// sword stored and a move on the wire.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_stop_takes_back_what_is_stored_once_and_stops() {
    let stop = CancellationToken::new();
    let (walk, transcript, session) = set_out(&stop, ROOMS);
    transcript.answer("north", &arrival(1002));
    transcript.answer("store right", SWORD_GONE);
    let climbing = until_written(&transcript, "climb rope").await;
    assert!(climbing, "never reached the rope: {:?}", transcript.lines());

    let at_stop = Instant::now();
    stop.cancel();
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    let elapsed = at_stop.elapsed();

    assert_eq!(travelled.ended, Ended::Stopped(BehaviorError::Cancelled));
    assert!(elapsed <= PREEMPT_GRACE, "stop took {elapsed:?}");
    // Sharper than the budget, because the budget cannot see the defect: a
    // hold that is not raced against the stop still ends at its next beat,
    // and a beat IS the grace. In virtual time a raced stop crosses no timer.
    assert_eq!(elapsed, Duration::ZERO, "the stop waited out a timer");
    let lines = transcript.lines();
    assert_eq!(lines.last().map(String::as_str), Some("get #11"));
    assert_eq!(
        lines.iter().filter(|line| *line == "get #11").count(),
        1,
        "one get, and then it stops: {lines:?}"
    );
    // Nothing saw it come back, so it is still reported.
    assert_eq!(travelled.still_stored.len(), 1);
    assert_eq!(travelled.still_stored[0].id, "11");

    // ...and it really has stopped: time passing sends nothing more.
    tokio::time::advance(Duration::from_mins(1)).await;
    tokio::task::yield_now().await;
    assert_eq!(transcript.lines().len(), lines.len());
    session.cancel();
}

/// With nothing stored, a stop sends nothing at all (`plan/12` §4.3).
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_stop_with_nothing_stored_sends_nothing() {
    let stop = CancellationToken::new();
    let (walk, transcript, session) = set_out(&stop, ROOMS);
    // `north` is answered with a bare prompt: the walker is still in room 1.
    let walking = until_written(&transcript, "north").await;
    assert!(walking, "never set out: {:?}", transcript.lines());
    let sent = transcript.lines().len();

    let at_stop = Instant::now();
    stop.cancel();
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert!(at_stop.elapsed() <= PREEMPT_GRACE);
    assert_eq!(travelled.ended, Ended::Stopped(BehaviorError::Cancelled));
    assert!(travelled.still_stored.is_empty());
    assert_eq!(transcript.lines().len(), sent, "a stop is not a send");
    session.cancel();
}

/// The trip hears the game through the model's own lines. `north` is refused
/// in words, with no room change to go by, so the only way the trip can know
/// is by hearing the line -- and it is the only way to room 3.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn what_the_game_says_reaches_the_trip() {
    let stop = CancellationToken::new();
    let (walk, transcript, session) = set_out(&stop, ROOMS);
    transcript.answer(
        "north",
        b"You can't go there.\n<prompt time=\"2\">&gt;</prompt>\n",
    );

    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Failed(Why::NoRoute));
    assert_eq!(travelled.wrong_for_the_map, [(RoomId(1), RoomId(2))]);
    assert_eq!(
        transcript.lines(),
        ["north"],
        "told once, it does not insist"
    );
    session.cancel();
}

/// Two rooms with one number -- a room that changes form keeps its number
/// (`cena_map::locate`, 42 of them measured) -- and exits from room 1 to
/// both, so where the walker came from does not settle it. Only the text does.
const TWINS: &str = r#"[
  {"id":1,"uid":[1001],"exits":[{"to":2,"kind":"cardinal","cmd":"north","cost":1},
                                {"to":5,"kind":"cardinal","cmd":"south","cost":9}]},
  {"id":2,"uid":[1002],"description":["The east bank."],
     "exits":[{"to":3,"kind":"cardinal","cmd":"climb rope","cost":1}]},
  {"id":5,"uid":[1002],"description":["The west bank."],
     "exits":[{"to":3,"kind":"cardinal","cmd":"east","cost":1}]},
  {"id":3,"uid":[1003]}
]"#;

/// The walker goes north for room 2 and the game puts it on the west bank.
/// Told apart by the description, it goes `east` from there; not told apart,
/// it would not know where it was and would never move again.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn rooms_that_share_a_number_are_told_apart_by_what_they_say() {
    let stop = CancellationToken::new();
    let (walk, transcript, session) = set_out(&stop, TWINS);
    transcript.answer(
        "north",
        b"<nav rm='1002'/><compDef id='room desc'>The west bank.</compDef>
          <prompt time=\"2\">&gt;</prompt>
",
    );
    transcript.answer("east", &arrival(1003));

    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Arrived);
    assert_eq!(transcript.lines(), ["north", "east"]);
    session.cancel();
}

/// A room the map cannot name ends the trip; it does not hold it for ever
/// (`plan/12` section 5.5: every wait has a deadline).
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_room_the_map_does_not_have_ends_the_trip() {
    let stop = CancellationToken::new();
    let (walk, transcript, session) = set_out(&stop, ROOMS);
    transcript.answer("north", &arrival(4040));

    let began = Instant::now();
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Failed(Why::OffTheMap));
    assert!(began.elapsed() >= LOST_WAIT, "it waits for the title first");
    assert!(began.elapsed() < LOST_WAIT * 2, "and not much longer");
    assert_eq!(transcript.lines(), ["north"], "lost, it sends nothing");
    session.cancel();
}

/// A ladder does not carry a group: the walker climbs, and waits at the top.
const LADDER: &str = r#"[
  {"id":1,"uid":[1001],"exits":[{"to":2,"kind":"scripted","cost":1,
     "steps":[{"move":"climb ladder"},{"await_followers":null}]}]},
  {"id":2,"uid":[1002],"exits":[{"to":3,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":3,"uid":[1003]}
]"#;

fn oreh() -> Member {
    Member {
        id: "-10467645".into(),
        noun: "Oreh".into(),
        text: "Oreh".into(),
    }
}

/// `group.rb:420`'s example.
const OREH_JOINS: &[u8] = b"<a exist=\"-10467645\" noun=\"Oreh\">Oreh</a> joins your group.
    <prompt time=\"3\">&gt;</prompt>
";

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_walker_waits_at_the_top_until_its_company_rejoins() {
    let stop = CancellationToken::new();
    let (walk, transcript, session, typed) = set_out_with(&stop, LADDER, &[oreh()]);
    transcript.answer("climb ladder", &arrival(1002));
    transcript.answer("north", &arrival(1003));
    assert!(until_written(&transcript, "climb ladder").await);

    // Up the ladder, and Oreh is not: it does not go on without them.
    tokio::time::advance(FOLLOW_WAIT / 3).await;
    tokio::task::yield_now().await;
    assert_eq!(transcript.lines(), ["climb ladder"], "it went on alone");

    // Someone else joining is not Oreh rejoining.
    transcript.answer(
        "nod",
        b"<a exist=\"-5\" noun=\"Szan\">Szan</a> joins your group.
<prompt time=\"3\">&gt;</prompt>
",
    );
    let _ = typed.send_now("nod", Origin::Manual, Gate::None).await;
    tokio::time::advance(FOLLOW_WAIT / 3).await;
    tokio::task::yield_now().await;
    assert_eq!(
        transcript.lines(),
        ["climb ladder", "nod"],
        "the wrong one counted"
    );

    // Oreh reaches the top. The game says so in answer to nothing the walker
    // sent, which a player's own command stands in for.
    transcript.answer("smile", OREH_JOINS);
    let _ = typed.send_now("smile", Origin::Manual, Gate::None).await;

    let began = Instant::now();
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Arrived);
    assert_eq!(
        transcript.lines(),
        ["climb ladder", "nod", "smile", "north"]
    );
    assert!(
        began.elapsed() < FOLLOW_WAIT / 3,
        "it waited out the clock anyway"
    );
    session.cancel();
}

/// A follower who never comes does not strand the walker: upstream's way out
/// is typing `go`, and this one's is the clock.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_follower_who_never_comes_is_not_waited_for_for_ever() {
    let stop = CancellationToken::new();
    let (walk, transcript, session, _typed) = set_out_with(&stop, LADDER, &[oreh()]);
    transcript.answer("climb ladder", &arrival(1002));
    transcript.answer("north", &arrival(1003));

    let began = Instant::now();
    // Bounded here too: with the deadline gone this test would hang, and a
    // hang reports nothing. Found by the mutation that removed it.
    let walked = tokio::time::timeout(FOLLOW_WAIT * 4, walk).await;
    let travelled = walked
        .expect("the wait for a follower has no deadline")
        .expect("the walk must not panic")
        .unwrap();
    assert_eq!(travelled.ended, Ended::Arrived);
    assert!(began.elapsed() >= FOLLOW_WAIT, "it did not wait at all");
    session.cancel();
}

/// Alone, there is nobody to wait for.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn alone_the_walker_does_not_wait() {
    let stop = CancellationToken::new();
    let (walk, transcript, session) = set_out(&stop, LADDER);
    transcript.answer("climb ladder", &arrival(1002));
    transcript.answer("north", &arrival(1003));

    let began = Instant::now();
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Arrived);
    assert!(began.elapsed() < FOLLOW_WAIT, "it waited for nobody");
    session.cancel();
}

/// The seed is made of what the wire said, so a replay has it for nothing:
/// the same prompt second, room and goal give the same seed, and any one of
/// them changing gives another.
#[test]
fn the_seed_is_the_wires_and_not_the_machines() {
    let at = |second: &str, room: &str| {
        let mut state = GameState::default();
        state.apply(&Frame::Prompt {
            time: second.into(),
            text: ">".into(),
        });
        state.room.id = Some(room.into());
        state
    };
    let seed = seed_for(&at("1700000000", "1001"), RoomId(3));
    assert_eq!(
        seed,
        seed_for(&at("1700000000", "1001"), RoomId(3)),
        "a replay"
    );
    assert_ne!(
        seed,
        seed_for(&at("1700000001", "1001"), RoomId(3)),
        "a second on"
    );
    assert_ne!(
        seed,
        seed_for(&at("1700000000", "1002"), RoomId(3)),
        "another room"
    );
    assert_ne!(
        seed,
        seed_for(&at("1700000000", "1001"), RoomId(4)),
        "another goal"
    );
    // A second apart is not a bit apart: the maze's first turn must differ.
    let next = seed_for(&at("1700000001", "1001"), RoomId(3));
    assert!((seed ^ next).count_ones() > 8, "{seed:x} against {next:x}");
}

/// A maze: every way out is tried at random until the exits change.
const MAZE: &str = r#"[
  {"id":1,"uid":[1001],"exits":[{"to":3,"kind":"scripted","cost":1,
     "steps":[{"move_any_while":[["northwest","southwest","northeast","southeast"],
                                 {"exits_are":["ne","se","sw","nw"]}]}]}]},
  {"id":3,"uid":[1003]}
]"#;

/// The first turns a walker takes in the maze, setting out at this second of
/// the game's clock. The game never lets it out, so every turn is a choice.
async fn turns_at(second: &'static str) -> Vec<String> {
    let stop = CancellationToken::new();
    let (walk, transcript, session, _, _) = set_out_as(&stop, MAZE, |state| {
        state.apply(&Frame::Prompt {
            time: second.into(),
            text: ">".into(),
        });
        state.room.exits = Some(["ne", "se", "sw", "nw"].map(str::to_owned).to_vec());
    });
    for _ in 0..400 {
        if transcript.written_count() >= 8 {
            break;
        }
        tokio::time::advance(Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
    }
    stop.cancel();
    let _ = walk.await;
    session.cancel();
    transcript.lines().into_iter().take(8).collect()
}

/// The driver gives the trip the wire's seed: the same second walks the maze
/// the same way, and another second walks it another. With the seed fixed
/// (`Trip::to`) the last assertion fails -- every trip would take one walk.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_maze_is_walked_by_the_wires_seed() {
    let first = turns_at("1700000000").await;
    assert_eq!(first.len(), 8, "the walker never wandered: {first:?}");
    assert_eq!(first, turns_at("1700000000").await, "a replay");
    assert_ne!(
        first,
        turns_at("1700000777").await,
        "another day, another walk"
    );
}

/// Every notice published so far, as `(kind, text)`.
fn told_so_far(told: &mut Receiver<Event>) -> Vec<(NoticeKind, String)> {
    let mut said = Vec::new();
    while let Ok(event) = told.try_recv() {
        if let Event::Notice(notice) = event {
            said.push((notice.kind, notice.lines().join(" ")));
        }
    }
    said
}

/// The player is told, in words, by the trip itself: why it failed, and what
/// it could not put back. Nobody has to remember to print a return value.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_trip_says_how_it_ended() {
    // No way there: room 3 cannot be reached once `north` is refused.
    let stop = CancellationToken::new();
    let (walk, transcript, session, _, mut told) = set_out_as(&stop, ROOMS, |_| {});
    transcript.answer("north", REFUSED);
    let _ = walk.await;
    let said = told_so_far(&mut told);
    assert_eq!(said.len(), 1, "{said:?}");
    assert_eq!(said[0].0, NoticeKind::Error);
    assert!(said[0].1.contains("no way there"), "{said:?}");
    session.cancel();

    // Stopped with the sword put away: a stop is not news, the sword is.
    let stop = CancellationToken::new();
    let (walk, transcript, session, _, mut told) = set_out_as(&stop, ROOMS, |_| {});
    transcript.answer("north", &arrival(1002));
    transcript.answer("store right", SWORD_GONE);
    assert!(until_written(&transcript, "climb rope").await);
    stop.cancel();
    let _ = walk.await;
    let said = told_so_far(&mut told);
    assert_eq!(said.len(), 1, "{said:?}");
    assert_eq!(said[0].0, NoticeKind::Warn);
    assert!(said[0].1.contains("broadsword"), "{said:?}");
    session.cancel();

    // Arrived, with everything back: nothing to say.
    let stop = CancellationToken::new();
    let (walk, transcript, session, _, mut told) = set_out_as(&stop, ROOMS, |_| {});
    transcript.answer("north", &arrival(1002));
    transcript.answer("store right", SWORD_GONE);
    transcript.answer("climb rope", &arrival(1003));
    transcript.answer("get #11", SWORD_BACK);
    let _ = walk.await;
    assert_eq!(told_so_far(&mut told), [], "arriving is not news");
    session.cancel();
}

/// Rooms told apart by name alone, as a login leaves them for a moment.
const NAMED: &str = r#"[
  {"id":1,"uid":[7086],"title":["[Wehnimer's, Erebor Square]"],
   "exits":[{"to":2,"kind":"cardinal","cmd":"south","cost":1}]},
  {"id":2,"uid":[7087],"title":["[Wehnimer's, Land's End Rd.]"]},
  {"id":3,"uid":[1003]}
]"#;

/// The first live login, 2026-09-21: the burst names the room at frame 11 and
/// numbers it at frame 378. A walker asked in between had the title, the
/// description and the exits, and said it could not tell where it was,
/// because it would not look without a number. It looks now.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_room_the_game_has_not_numbered_yet_is_found_by_its_name() {
    let stop = CancellationToken::new();
    let (walk, transcript, session, _, _) = set_out_as(&stop, NAMED, |state| {
        state.room.id = None;
        state.room.title = Some("Wehnimer's, Erebor Square".into());
    });
    // Room 3 cannot be reached; what matters is that the walker knows it is
    // in room 1, which is the only way it can say so and not "off the map".
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Failed(Why::NoRoute));
    assert_eq!(transcript.lines(), Vec::<String>::new());
    session.cancel();
}
