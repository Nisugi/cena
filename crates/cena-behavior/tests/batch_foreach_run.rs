//! `;foreach` (`plan/30` §7, M6e) against a scripted game: the looks it
//! sends, what it reads back, and the commands each item gets.
//!
//! The game answers `look in` with real looks: the five in
//! `crates/cena-ui/tests/fixtures/container_looks.xml`, cut from the
//! author's logs for `;sorter`. Each is the `<container>`, `<clearContainer>`
//! and `<inv>` feed, then the `In the ... you see` line, then its prompt.

mod ready;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use cena_behavior::batch::{self, Command, Desk, Halt, Hydra, Job, Ran};
use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::{AuthorityToken, Event, Outcome, Session, SessionHandle, SessionObserver};
use tokio::sync::broadcast::Receiver;

const PROMPT: &[u8] = b"<prompt time=\"1\">&gt;</prompt>\n";
const LOOKS: &str = include_str!("../../cena-ui/tests/fixtures/container_looks.xml");

/// Look `n` of the fixture (0: the box, 1: the cloak, 2: the backpack, 3:
/// the shelf, 4: the kit), with its prompt, as the game sent it.
fn look(n: usize) -> Vec<u8> {
    let lines: Vec<&str> = LOOKS.lines().collect();
    format!("{}\n{}\n", lines[2 * n], lines[2 * n + 1]).into_bytes()
}

const BACKPACK: usize = 2;
const CLOAK: usize = 1;
const SHELF: usize = 3;

struct Game {
    handle: SessionHandle,
    observer: SessionObserver,
    transcript: TranscriptHandle,
    told: Receiver<Event>,
}

async fn logged_in() -> Result<Game, String> {
    let (source, transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let observer = session.observer();
    let (_, told) = session.subscribe();
    let (_, ready) = session.subscribe();
    tokio::spawn(session.into_actor().run());
    ready::until_ready(ready).await?;
    Ok(Game {
        handle,
        observer,
        transcript,
        told,
    })
}

impl Game {
    /// Run `;<line>` as the binary does, to its end.
    async fn foreach(&self, line: &str, hydra: Hydra) -> Result<Result<(), Halt>, String> {
        let Some(Ok(Command::Run(job @ Job::Foreach(_)))) = batch::parse(line, ';') else {
            return Err(format!("{line:?} is not a foreach to run"));
        };
        let joined = self
            .observer
            .subscribe()
            .await
            .map_err(|e| format!("{e:?}"))?;
        let desk = Desk::new("Foreach", AuthorityToken(5));
        let run = desk
            .run(&self.handle, joined, job, hydra)
            .ok_or("the desk would not run it")?;
        run.await.map_err(|e| e.to_string())
    }

    fn said(&mut self) -> Vec<String> {
        let mut said = Vec::new();
        while let Ok(event) = self.told.try_recv() {
            if let Event::Notice(notice) = event {
                said.extend(notice.lines().iter().cloned());
            }
        }
        said
    }
}

fn nobody() -> Hydra {
    Arc::new(|_: &str| Box::pin(async { Ran::Unknown }))
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn with_no_commands_it_lists_what_matches_and_sends_only_the_look() {
    let mut game = logged_in().await.unwrap();
    game.transcript.answer("look in backpack", &look(BACKPACK));
    let ended = game.foreach("foreach in backpack", nobody()).await.unwrap();
    assert_eq!(ended, Ok(()));
    assert_eq!(game.transcript.lines(), ["look in backpack"]);
    let said = game.said();
    for line in [
        "[dwarf skin backpack]:",
        "#2376085 a tumbler of black cherry whiskey (no type)",
        "#2376083 a carved basalt teardrop (gem)",
        "#2376078 an alexandrite inset mithril circlet (jewelry)",
        "Total items: 8",
    ] {
        assert!(said.iter().any(|s| s == line), "{line:?} not in {said:#?}");
    }
}

/// `;foreach name=*feather in backpack; get item; put item in container`:
/// each feather, by its id, back into the backpack by its id.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn each_item_gets_the_commands_with_its_own_id() {
    let mut game = logged_in().await.unwrap();
    game.transcript.answer("look in backpack", &look(BACKPACK));
    let ended = game
        .foreach(
            "foreach name=*feather in backpack; get item; put item in container",
            nobody(),
        )
        .await
        .unwrap();
    assert_eq!(ended, Ok(()));
    assert_eq!(
        game.transcript.lines(),
        [
            "look in backpack",
            "get #2376084",
            "put #2376084 in #2376077",
            "get #2376082",
            "put #2376082 in #2376077",
        ]
    );
    let said = game.said();
    assert!(
        said.iter()
            .any(|s| s == "Foreach: item 1 of 2 (0% complete).")
    );
    assert!(said.iter().any(|s| s == "Foreach: done, 2 items."));
    assert_eq!(game.handle.holder(), None, "released at the end");
}

/// A type from the `gameobj` table on a real look, `unique`, and a bare
/// `sel` made `get item; sell item`.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_type_on_a_real_look() {
    let game = logged_in().await.unwrap();
    game.transcript.answer("look in backpack", &look(BACKPACK));
    let ended = game
        .foreach("foreach unique uncommon in backpack; sel", nobody())
        .await
        .unwrap();
    assert_eq!(ended, Ok(()));
    assert_eq!(
        game.transcript.lines(),
        [
            "look in backpack",
            "get #2376084",
            "sell #2376084",
            "get #2376081",
            "sell #2376081",
            "get #2376079",
            "sell #2376079",
        ]
    );
}

/// `on`, and the progress every tenth item: the shelf holds sixteen.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn on_a_shelf_with_its_progress_every_tenth_item() {
    let mut game = logged_in().await.unwrap();
    game.transcript.answer("look on shelf", &look(SHELF));
    let ended = game
        .foreach("foreach on shelf; look item", nobody())
        .await
        .unwrap();
    assert_eq!(ended, Ok(()));
    let lines = game.transcript.lines();
    assert_eq!(lines.len(), 17, "{lines:?}");
    assert_eq!(lines[0], "look on shelf");
    assert_eq!(lines[1], "look #53530");
    assert_eq!(lines[16], "look #53515");
    let progress: Vec<String> = game
        .said()
        .into_iter()
        .filter(|s| s.contains("complete"))
        .collect();
    assert_eq!(
        progress,
        [
            "Foreach: item 1 of 16 (0% complete).",
            "Foreach: item 11 of 16 (62% complete)."
        ]
    );
}

/// The cloak is the `stow` window, keyed `stow` and not by its id: found
/// by the window's `target`.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_stow_container_is_found_by_its_target() {
    let game = logged_in().await.unwrap();
    game.transcript.answer("look in cloak", &look(CLOAK));
    let ended = game
        .foreach("foreach noun=rod in cloak; get item", nobody())
        .await
        .unwrap();
    assert_eq!(ended, Ok(()));
    assert_eq!(
        game.transcript.lines(),
        ["look in cloak", "get #3265918", "get #2376050"]
    );
}

const NOT_FOUND: &[u8] =
    b"I could not find what you were referring to.\n<prompt time=\"1\">&gt;</prompt>\n";

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_missing_target_stops_it_before_anything_is_done() {
    let mut game = logged_in().await.unwrap();
    game.transcript.answer("look in sack", NOT_FOUND);
    let ended = game
        .foreach("foreach all in sack; sell item", nobody())
        .await
        .unwrap();
    let Err(Halt::Failed(why)) = ended else {
        panic!("a missing sack did not stop it: {ended:?}");
    };
    assert!(why.contains("not found") && why.contains("sack?"), "{why}");
    assert_eq!(game.transcript.lines(), ["look in sack"]);
    assert!(game.said().iter().any(|s| s.starts_with("Foreach: 'sack'")));
}

/// `:1872`: a trailing `?` skips a target that is not there.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_missing_target_with_a_question_mark_is_skipped() {
    let mut game = logged_in().await.unwrap();
    game.transcript.answer("look in sack", NOT_FOUND);
    game.transcript.answer("look in backpack", &look(BACKPACK));
    let ended = game
        .foreach(
            "foreach name=*feather in sack?, backpack; get item",
            nobody(),
        )
        .await
        .unwrap();
    assert_eq!(ended, Ok(()));
    assert_eq!(
        game.transcript.lines(),
        [
            "look in sack",
            "look in backpack",
            "get #2376084",
            "get #2376082"
        ]
    );
    assert!(game.said().iter().any(|s| s.contains("skipping 'sack?'")));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn an_empty_container_is_said_and_passed() {
    let mut game = logged_in().await.unwrap();
    game.transcript.answer(
        "look in sack",
        b"There is nothing in the leather sack.\n<prompt time=\"1\">&gt;</prompt>\n",
    );
    let ended = game
        .foreach("foreach all in sack; sell item", nobody())
        .await
        .unwrap();
    assert_eq!(ended, Ok(()));
    assert_eq!(game.transcript.lines(), ["look in sack"]);
    let said = game.said();
    assert!(
        said.iter().any(|s| s == "Foreach: 'sack' is empty."),
        "{said:?}"
    );
    assert!(
        said.iter()
            .any(|s| s == "Foreach: no matching items found.")
    );
}

/// The room's own list of what is on the ground: no look needed. A command
/// that names the container is refused before anything is sent.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_ground_needs_no_look() {
    let mut game = logged_in().await.unwrap();
    game.transcript.answer(
        "look",
        b"<component id='room objs'>You also see a <a exist=\"61\" noun=\"box\">battered box</a> \
          and a <a exist=\"62\" noun=\"coins\">pile of coins</a>.</component>\n\
          <prompt time=\"1\">&gt;</prompt>\n",
    );
    let looked = game
        .handle
        .send_manual_at(game.handle.generation(), "look", Duration::from_secs(5))
        .await;
    assert!(matches!(looked, Outcome::Confirmed(_)), "{looked:?}");
    let ended = game
        .foreach("foreach all in ground; get item", nobody())
        .await
        .unwrap();
    assert_eq!(ended, Ok(()));
    assert_eq!(game.transcript.lines(), ["look", "get #61", "get #62"]);

    let ended = game
        .foreach("foreach all in ground; put item in container", nobody())
        .await
        .unwrap();
    let Err(Halt::Failed(why)) = ended else {
        panic!("`container` on the ground was not refused: {ended:?}");
    };
    assert!(why.contains("on the ground"), "{why}");
    assert_eq!(game.transcript.written_count(), 3, "nothing more was sent");
    assert!(game.said().iter().any(|s| s.contains("on the ground")));
}

/// A Hydra command gets `item` filled in, and runs for each item.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_hydra_command_runs_for_each_item() {
    let game = logged_in().await.unwrap();
    game.transcript.answer("look in backpack", &look(BACKPACK));
    let ran = Arc::new(Mutex::new(Vec::new()));
    let kept = Arc::clone(&ran);
    let hydra: Hydra = Arc::new(move |line: &str| {
        kept.lock().unwrap().push(line.to_owned());
        Box::pin(async { Ran::Done })
    });
    let ended = game
        .foreach("foreach name=*feather in backpack; ;sc 704 item", hydra)
        .await
        .unwrap();
    assert_eq!(ended, Ok(()));
    assert_eq!(*ran.lock().unwrap(), ["sc 704 #2376084", "sc 704 #2376082"]);
    assert_eq!(game.transcript.lines(), ["look in backpack"]);
}
