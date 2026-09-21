//! Travel **as a player calls it, while playing**: a typed line, read by
//! `parse_command`, done by the `Desk` against a session that is already
//! running -- each command from a fresh subscription, as a frontend has.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use cena_behavior::BehaviorError;
use cena_behavior::travel::{Desk, Ended, Travelled, parse_command};
use cena_map::{Map, Room};
use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::{
    AuthorityToken, CommandId, Event, NoticeKind, Origin, Session, SessionHandle, SessionObserver,
};
use tokio::sync::broadcast::Receiver;
use tokio::task::JoinHandle;

const PROMPT: &[u8] = b"<prompt time=\"1\">&gt;</prompt>\n";

/// ```text
///   1 --north-- 2 (bank) --north-- 3 --north-- 4 (gemshop)
///   1 --a rope, climbed with empty hands-- 5 (loft)
/// ```
const TOWN: &str = r#"[
  {"id":1,"uid":[1001],"title":["[Town, Gate]"],
   "exits":[{"to":2,"kind":"cardinal","cmd":"north","cost":1},
            {"to":5,"kind":"scripted","cost":1,
             "steps":[{"empty_hands":null},{"move":"climb rope"},{"fill_hands":null}]}]},
  {"id":2,"uid":[1002],"title":["[Town, Bank]"],"tags":["bank"],
   "exits":[{"to":3,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":3,"uid":[1003],"title":["[Town, Lane]"],
   "exits":[{"to":4,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":4,"uid":[1004],"title":["[Town, Gems]"],"tags":["gemshop"]},
  {"id":5,"uid":[1005],"title":["[Town, Loft]"],"tags":["loft"]}
]"#;

fn arrival(uid: u32) -> Vec<u8> {
    format!("<nav rm='{uid}'/>\n<prompt time=\"2\">&gt;</prompt>\n").into_bytes()
}

/// Someone playing: a running session, a desk, and what the player is told.
struct Playing {
    desk: Arc<Desk>,
    handle: SessionHandle,
    observer: SessionObserver,
    transcript: TranscriptHandle,
    told: Receiver<Event>,
    dir: PathBuf,
}

impl Playing {
    /// Logged in as Ashryn, standing at the gate. `None` is a broken fixture.
    async fn at_the_gate(test: &str) -> Option<Playing> {
        let dir = std::env::temp_dir().join(format!("cena-desk-test-{test}"));
        let _ = std::fs::remove_dir_all(&dir);
        let (source, transcript) = AnsweringSource::new(PROMPT);
        let session = Session::new(source);
        let (handle, observer) = (session.handle(), session.observer());
        let (_, told) = session.subscribe();
        tokio::spawn(session.into_actor().run());
        // The game says who and where, as a login does.
        let mut login = b"<app char=\"Ashryn\" game=\"GSIV\"/>\n".to_vec();
        login.extend(arrival(1001));
        transcript.answer("look", &login);
        let looked = handle.send_and_await(
            CommandId(1),
            "look",
            Origin::Manual,
            Duration::from_secs(5),
            |_| true,
        );
        looked.await;
        let rooms: Vec<Room> = serde_json::from_str(TOWN).ok()?;
        let map = Arc::new(Map::from_rooms(rooms).ok()?);
        let desk = Desk::new(map, dir.clone(), AuthorityToken(2));
        Some(Playing {
            desk,
            handle,
            observer,
            transcript,
            told,
            dir,
        })
    }

    /// The player types a line. `None`: it was the game's, or did not walk.
    async fn types(&self, line: &str) -> Option<JoinHandle<Travelled>> {
        let command = parse_command(line)?.ok()?;
        let joined = self.observer.subscribe().await.ok()?;
        self.desk.run(&self.handle, joined, command)
    }

    /// Everything the player has been told so far, as text.
    fn told(&mut self) -> String {
        let mut said = String::new();
        while let Ok(event) = self.told.try_recv() {
            if let Event::Notice(notice) = event {
                let _ = writeln!(said, "{:?} {:?}", notice.kind, notice.body);
            }
        }
        said
    }

    /// What went to the game, less what the test typed to set the scene.
    fn sent(&self) -> Vec<String> {
        let scene = |line: &String| line == "look" || line == "glance";
        self.transcript
            .lines()
            .into_iter()
            .filter(|line| !scene(line))
            .collect()
    }
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn go2_bank_typed_while_playing_walks_there_and_remembers_where_it_stopped() {
    let mut playing = Playing::at_the_gate("go").await.unwrap();
    playing.transcript.answer("north", &arrival(1002));
    let walk = playing.types(";go2 bank").await.unwrap();
    assert_eq!(walk.await.unwrap().ended, Ended::Arrived);
    assert_eq!(playing.sent(), ["north"]);
    let said = playing.told();
    assert!(
        said.contains("[Town, Bank]"),
        "the route is shown first:\n{said}"
    );
    let kept = std::fs::read_to_string(playing.dir.join("travel.json")).unwrap();
    assert!(kept.contains("\"last_room\": 2"), "{kept}");
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn route2_shows_the_way_and_sends_nothing() {
    let mut playing = Playing::at_the_gate("route").await.unwrap();
    assert!(playing.types(";route2 gemshop").await.is_none());
    assert_eq!(playing.sent(), [] as [&str; 0]);
    let said = playing.told();
    assert!(
        said.contains("[Town, Gems]") && said.contains("north"),
        "{said}"
    );
}

/// The walk is left walking, and `;go2 stop` -- or go2's own `;k go2` -- ends
/// it. The move is never answered, so only the stop can.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_walk_is_stopped_by_typing_so() {
    let mut playing = Playing::at_the_gate("stop").await.unwrap();
    let walk = playing.types(";go2 gemshop").await.unwrap();
    tokio::time::advance(Duration::from_millis(500)).await;
    assert!(playing.types(";k go2").await.is_none());
    let travelled = walk.await.unwrap();
    assert_eq!(travelled.ended, Ended::Stopped(BehaviorError::Cancelled));
    // ...and there is nothing left to stop.
    assert!(playing.types(";go2 stop").await.is_none());
    assert!(playing.told().contains("not walking"));
}

/// Naming a new place while walking means the new place: the first walk is
/// stopped, and **the second waits for it to let the authority go**. The
/// first has a sword stored, so stopping it means sending `get #11` before it
/// releases -- long enough that a second walk claiming at once is refused
/// (`CommandQueue::claim` refuses while anyone holds it, the same token too).
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_second_go2_replaces_the_first() {
    let playing = Playing::at_the_gate("replace").await.unwrap();
    let armed = b"<right exist=\"11\" noun=\"sword\">broadsword</right>
                  <prompt time=\"2\">&gt;</prompt>
";
    playing.transcript.answer("glance", armed);
    playing
        .handle
        .send_and_await(
            CommandId(2),
            "glance",
            Origin::Manual,
            Duration::from_secs(5),
            |_| true,
        )
        .await;
    playing.transcript.answer(
        "store right",
        b"<right>Empty</right>
<prompt time=\"3\">&gt;</prompt>
",
    );
    // Up the rope with empty hands; the climb is never answered.
    let first = playing.types(";go2 loft").await.unwrap();
    for _ in 0..40 {
        tokio::time::advance(Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
    }
    assert!(
        playing
            .transcript
            .lines()
            .iter()
            .any(|line| line == "climb rope"),
        "{:?}",
        playing.transcript.lines()
    );
    playing.transcript.answer("north", &arrival(1002));
    let second = playing.types(";go2 bank").await.unwrap();
    assert_eq!(
        first.await.unwrap().ended,
        Ended::Stopped(BehaviorError::Cancelled)
    );
    assert_eq!(second.await.unwrap().ended, Ended::Arrived);
    let sent = playing.sent();
    assert_eq!(sent[sent.len() - 2..], ["get #11", "north"], "{sent:?}");
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_name_is_saved_listed_walked_to_and_forgotten() {
    let mut playing = Playing::at_the_gate("names").await.unwrap();
    assert!(playing.types(";go2 save stash=3 --global").await.is_none());
    assert!(playing.types(";go2 save here").await.is_none());
    assert!(playing.types(";go2 list").await.is_none());
    let said = playing.told();
    assert!(
        said.contains("every character") && said.contains("this character"),
        "{said}"
    );
    assert!(
        said.contains("stash") && said.contains("[3]") && said.contains("[1]"),
        "{said}"
    );

    playing.transcript.answer("north", &arrival(1002));
    playing.transcript.answer("north", &arrival(1003));
    let walk = playing.types(";go2 stash").await.unwrap();
    assert_eq!(walk.await.unwrap().ended, Ended::Arrived);
    assert_eq!(playing.sent(), ["north", "north"]);

    // Forgotten, it is no place at all -- no room is called that either.
    assert!(playing.types(";go2 delete stash --global").await.is_none());
    assert!(playing.types(";route2 stash").await.is_none());
    assert!(playing.told().contains("do not know a room called"));
    // A room the map does not have stops the save, as go2 has it.
    assert!(playing.types(";go2 save nowhere=99999").await.is_none());
    assert!(playing.told().contains("not in the map"));
}

/// What is not a travel command is the game's, and nothing here touches it;
/// what is one, said wrongly, is not the game's either.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn only_travels_own_lines_are_taken() {
    let playing = Playing::at_the_gate("others").await.unwrap();
    for line in ["north", "say ;go2 bank", ";hunt"] {
        assert!(parse_command(line).is_none(), "{line}");
    }
    assert!(matches!(parse_command(";go2"), Some(Err(_))));
    assert!(playing.types(";go2 targets").await.is_none());
    assert_eq!(playing.sent(), [] as [&str; 0]);
    let _ = NoticeKind::Info;
}
