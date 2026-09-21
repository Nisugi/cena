//! Named routines, through the real driver and a scripted game (`plan/24`
//! stage 6). What each solver decides is tested by table beside it; this is
//! the loop that runs them: a routine's moves going back through the trip,
//! its answers reaching it, and the trip looking at where it landed.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use cena_behavior::travel::{Ended, TravelNotes, Travelled, travel};
use cena_map::{Map, Room, RoomId};
use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::{AuthorityToken, CommandId, Session};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

const PROMPT: &[u8] = b"<prompt time=\"1\">&gt;</prompt>\n";

fn arrival(uid: u32) -> Vec<u8> {
    format!("<nav rm='{uid}'/>\n<prompt time=\"2\">&gt;</prompt>\n").into_bytes()
}

/// An arrival in a room that shows a thing.
fn arrival_showing(uid: u32, noun: &str) -> Vec<u8> {
    format!(
        "<nav rm='{uid}'/>\n<component id='room objs'>You also see a <a exist=\"5\" \
         noun=\"{noun}\">{noun}</a>.</component>\n<prompt time=\"2\">&gt;</prompt>\n"
    )
    .into_bytes()
}

/// A character in room 1 (the game's 1001), walking to `goal`.
fn set_out(
    rooms: &'static str,
    goal: u32,
    mut notes: TravelNotes,
) -> (
    JoinHandle<Option<Travelled>>,
    TranscriptHandle,
    CancellationToken,
) {
    let (source, transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let session_cancel = session.cancel_token();
    let (mut snapshot, events) = session.subscribe();
    snapshot.state.room.id = Some("1001".into());
    tokio::spawn(session.into_actor().run());
    let walk = tokio::spawn(async move {
        let rooms: Vec<Room> = serde_json::from_str(rooms).ok()?;
        let map = Map::from_rooms(rooms).ok()?;
        let next = Arc::new(AtomicU64::new(0));
        let ids = move || CommandId(next.fetch_add(1, Ordering::Relaxed));
        let stop = CancellationToken::new();
        // Boxed: the driver's future is large now that routines recurse.
        let travelled = Box::pin(travel(
            &handle,
            &stop,
            ids,
            AuthorityToken(1),
            (snapshot, events),
            &map,
            RoomId(goal),
            &mut notes,
            |_| {},
        ))
        .await;
        Some(travelled)
    });
    (walk, transcript, session_cancel)
}

/// The guild door 1 -> 2, and a long way round by 3.
const DOOR: &str = r#"[
  {"id":1,"uid":[1001],"exits":[
     {"to":2,"kind":"scripted","routine":{"name":"guild_password"},"cost":1},
     {"to":3,"kind":"cardinal","cmd":"east","cost":50}]},
  {"id":3,"uid":[1003],"exits":[{"to":2,"kind":"cardinal","cmd":"north","cost":50}]},
  {"id":2,"uid":[1002]}
]"#;

fn with_password() -> TravelNotes {
    let mut notes = TravelNotes::default();
    notes
        .settings
        .insert("rogue_password".into(), "kick, slap".into());
    notes
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_routine_is_run_and_the_trip_walks_on_from_where_it_landed() {
    let (walk, transcript, session) = set_out(DOOR, 2, with_password());
    transcript.answer("go door", &arrival(1002));
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Arrived);
    assert_eq!(
        transcript.lines(),
        ["lean door", "kick door", "slap door", "go door"]
    );
    session.cancel();
}

/// The door does not open: the routine ran and the walker never moved, so
/// the exit is given up for the trip -- not tried twenty times.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_routine_that_moved_nobody_is_not_tried_again() {
    let (walk, transcript, session) = set_out(DOOR, 2, with_password());
    transcript.answer("east", &arrival(1003));
    transcript.answer("north", &arrival(1002));
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Arrived);
    let lines = transcript.lines();
    assert_eq!(
        lines.iter().filter(|line| *line == "lean door").count(),
        1,
        "{lines:?}"
    );
    assert_eq!(lines[lines.len() - 2..], ["east", "north"]);
    session.cancel();
}

/// A circuit 1 -> 4 -> 1, and a thread that shows in 4 and leads to 2 --
/// though the map's exit says 9, because nobody knows where a thread lands.
const RIFT: &str = r#"[
  {"id":1,"uid":[1001],"exits":[
     {"to":9,"kind":"scripted","cost":1,"routine":{"name":"patrol",
        "starts":[1,4],"dirs":["north","south"],
        "landmarks":[{"noun":"thread","enter":"climb thread"}],"after":["stand"]}}]},
  {"id":4,"uid":[1004]},
  {"id":2,"uid":[1002],"exits":[{"to":9,"kind":"cardinal","cmd":"out","cost":1}]},
  {"id":9,"uid":[1009]}
]"#;

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_patrol_goes_round_until_it_sees_the_way_out_and_then_plans_again() {
    let (walk, transcript, session) = set_out(RIFT, 9, TravelNotes::default());
    transcript.answer("north", &arrival_showing(1004, "thread"));
    transcript.answer("climb thread", &arrival(1002));
    transcript.answer("out", &arrival(1009));
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Arrived);
    assert_eq!(
        transcript.lines(),
        ["north", "climb thread", "stand", "out"]
    );
    session.cancel();
}

/// 1 --north, fifty silver--> 2, and a bank east of 1.
const FERRY: &str = r#"[
  {"id":1,"uid":[1001],"tags":["silver-cost:2:50"],"exits":[
     {"to":2,"kind":"cardinal","cmd":"north","cost":5},
     {"to":3,"kind":"cardinal","cmd":"east","cost":1}]},
  {"id":3,"uid":[1003],"tags":["bank"],"exits":[{"to":1,"kind":"cardinal","cmd":"west","cost":1}]},
  {"id":2,"uid":[1002]}
]"#;

fn wealth(silver: u32) -> Vec<u8> {
    format!("You have {silver} silver with you.\n<prompt time=\"2\">&gt;</prompt>\n").into_bytes()
}

fn with(settings: &[&str]) -> TravelNotes {
    let mut notes = TravelNotes::default();
    for setting in settings {
        notes.settings.insert((*setting).into(), "true".into());
    }
    notes
}

/// `go2.lic:2217-2299`: short of the fare and allowed the bank, the trip
/// goes there first, withdraws the difference, and then sets out.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_walker_short_of_the_fare_goes_by_the_bank() {
    let (walk, transcript, session) = set_out(FERRY, 2, with(&["get_silvers"]));
    transcript.answer("wealth quiet", &wealth(10));
    transcript.answer("east", &arrival(1003));
    transcript.answer("wealth quiet", &wealth(10));
    transcript.answer("wealth quiet", &wealth(50));
    transcript.answer("west", &arrival(1001));
    transcript.answer("north", &arrival(1002));
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Arrived);
    assert_eq!(
        transcript.lines(),
        [
            "wealth quiet",
            "east",
            "wealth quiet",
            "withdraw 40 silvers",
            "wealth quiet",
            "west",
            "north"
        ]
    );
    session.cancel();
}

/// The bank has not enough: upstream exits, and so does this, saying why.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_bank_that_cannot_cover_the_fare_stops_the_trip() {
    let (walk, transcript, session) = set_out(FERRY, 2, with(&["get_silvers"]));
    transcript.answer("wealth quiet", &wealth(10));
    transcript.answer("east", &arrival(1003));
    transcript.answer("wealth quiet", &wealth(10));
    transcript.answer("wealth quiet", &wealth(10));
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Halted);
    assert!(
        travelled
            .halted
            .as_deref()
            .is_some_and(|why| why.contains("not enough silver")),
        "{:?}",
        travelled.halted
    );
    assert_eq!(
        transcript.lines().last().map(String::as_str),
        Some("wealth quiet")
    );
    session.cancel();
}

/// Enough in hand: the bank is never visited and nothing is withdrawn.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_walker_with_the_fare_just_goes() {
    let (walk, transcript, session) = set_out(FERRY, 2, with(&["get_silvers"]));
    transcript.answer("wealth quiet", &wealth(50));
    transcript.answer("north", &arrival(1002));
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Arrived);
    assert_eq!(transcript.lines(), ["wealth quiet", "north"]);
    session.cancel();
}

/// North is five seconds; the urchins' way is one, for a walker who has them.
const URCHINS: &str = r#"[
  {"id":1,"uid":[1001],"exits":[
     {"to":2,"kind":"cardinal","cmd":"north","cost":5},
     {"to":2,"kind":"other","cmd":"urchin guide 2","cost":{"when":{"flag":"urchin_access"},"then":1}}]},
  {"id":2,"uid":[1002]}
]"#;

/// `urchin_access` is a fact the map asks for and only the game can give:
/// asked once, before the first plan, and only of a profile that uses them.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn urchin_access_is_asked_before_the_plan_and_prices_it() {
    let (walk, transcript, session) = set_out(URCHINS, 2, with(&["use_urchins"]));
    transcript.answer(
        "urchin status",
        b"You have permanent access to the urchin guides.\n<prompt time=\"2\">&gt;</prompt>\n",
    );
    transcript.answer("urchin guide 2", &arrival(1002));
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Arrived);
    assert_eq!(transcript.lines(), ["urchin status", "urchin guide 2"]);
    session.cancel();
}

/// Voln's symbol, 1 -> 2, where 2 is the Red Forest.
const SEEKING: &str = r#"[
  {"id":1,"uid":[1001],"exits":[
     {"to":2,"kind":"scripted","routine":{"name":"seeking"},"cost":1}]},
  {"id":2,"uid":[1002],"title":["[Red Forest, Path]"]}
]"#;

fn vision(name: &str) -> Vec<u8> {
    format!(
        "Your vision is pulled away from you...\n<style id=\"roomName\" />{name}\n\
         <style id=\"\"/>Tall trees crowd the path.\n<prompt time=\"2\">&gt;</prompt>\n"
    )
    .into_bytes()
}

/// The vision's room name is told by its markup, through the real parser:
/// the wrong place is asked past, and the right one confirmed.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn seeking_reads_the_visions_room_name_off_the_wire() {
    let (walk, transcript, session) = set_out(SEEKING, 2, TravelNotes::default());
    transcript.answer("symbol of seeking", &vision("[Icemule Trace, South Gate]"));
    transcript.answer("symbol of seeking", &vision("[Red Forest, Path] (24715)"));
    let mut fog = b"Your surroundings blur into a white fog.\n".to_vec();
    fog.extend(arrival(1002));
    transcript.answer("symbol of seeking confirm", &fog);
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Arrived);
    assert_eq!(
        transcript.lines(),
        [
            "symbol of seeking",
            "symbol of seeking",
            "symbol of seeking confirm"
        ]
    );
    session.cancel();
}

/// 1 -> 2 -> 3, plainly.
const ROAD: &str = r#"[
  {"id":1,"uid":[1001],"exits":[{"to":2,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":2,"uid":[1002],"exits":[{"to":3,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":3,"uid":[1003]}
]"#;

/// `go2.lic:2406`: upstream pauses beside the dead; this stops and says why.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_body_on_the_road_stops_a_trip_that_was_asked_to() {
    let beside_the_dead = b"<nav rm='1002'/>\n<component id='room players'>Also here: \
        <a exist=\"-1\" noun=\"Demandred\">Demandred</a> who appears dead.</component>\n\
        <prompt time=\"2\">&gt;</prompt>\n";
    for (settings, ended, sent) in [
        (with(&["stop_for_dead"]), Ended::Halted, vec!["north"]),
        (
            TravelNotes::default(),
            Ended::Arrived,
            vec!["north", "north"],
        ),
    ] {
        let (walk, transcript, session) = set_out(ROAD, 3, settings);
        transcript.answer("north", beside_the_dead);
        transcript.answer("north", &arrival(1003));
        let travelled = walk.await.expect("the walk must not panic").unwrap();
        assert_eq!(travelled.ended, ended);
        assert_eq!(transcript.lines(), sent);
        session.cancel();
    }
}

/// `go2.lic:2405`: `sleep setting_delay` after each move.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn delay_is_waited_in_every_room_walked_into() {
    let mut notes = TravelNotes::default();
    notes.settings.insert("delay".into(), "7".into());
    let began = tokio::time::Instant::now();
    let (walk, transcript, session) = set_out(ROAD, 3, notes);
    transcript.answer("north", &arrival(1002));
    transcript.answer("north", &arrival(1003));
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Arrived);
    // Two rooms walked into, seven seconds each, in virtual time.
    let took = began.elapsed();
    assert!(
        took >= std::time::Duration::from_secs(14) && took < std::time::Duration::from_secs(20),
        "{took:?}"
    );
    session.cancel();
}

/// The long way to the Hinterwilds passes the encampment (29860); the Abbey's
/// teleporter is a step from the start.
const HINTERWILDS: &str = r#"[
  {"id":1,"uid":[1001],"location":"Icemule Trace","exits":[
     {"to":29860,"kind":"cardinal","cmd":"north","cost":500},
     {"to":31064,"kind":"cardinal","cmd":"east","cost":1}]},
  {"id":31064,"uid":[4132054],"location":"the Abbey","title":["[Abbey, Teleportation Chamber]"]},
  {"id":29860,"uid":[7503001],"location":"the Hinterwilds","exits":[
     {"to":29876,"kind":"cardinal","cmd":"south","cost":1}]},
  {"id":29876,"uid":[7503253],"location":"the Hinterwilds","exits":[
     {"to":29860,"kind":"cardinal","cmd":"out","cost":1}]}
]"#;

fn fragments(count: u32) -> Vec<u8> {
    format!(
        "You are carrying {count} gigas artifact fragments.\n<prompt time=\"2\">&gt;</prompt>\n"
    )
    .into_bytes()
}

/// `go2.lic:2191-2200`: with the fragments, the teleporter; without, the walk.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_hinterwilds_are_reached_by_fragments_when_there_are_enough() {
    let (walk, transcript, session) = set_out(HINTERWILDS, 29860, with(&["use_gigas_hwtravel"]));
    transcript.answer("wealth gigas", &fragments(6));
    transcript.answer("east", &arrival(4_132_054));
    transcript.answer("go sliver", &arrival(7_503_253));
    transcript.answer("out", &arrival(7_503_001));
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Arrived);
    assert_eq!(
        transcript.lines(),
        ["wealth gigas", "east", "go sliver", "go sliver", "out"]
    );
    session.cancel();

    let (walk, transcript, session) = set_out(HINTERWILDS, 29860, with(&["use_gigas_hwtravel"]));
    transcript.answer("wealth gigas", &fragments(3));
    transcript.answer("north", &arrival(7_503_001));
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Arrived);
    assert_eq!(transcript.lines(), ["wealth gigas", "north"]);
    session.cancel();
}
