//! The player's own commands, through the session a frontend uses
//! (`command::claimant`). The rule under test is the author's, 2026-09-21:
//! **the symbol decides**, so a mistyped command is never spoken in the room.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use cena_platform::AnsweringSource;
use cena_session::command::claimant::{Claimed, Desk, Runner};
use cena_session::{Event, Session};

const PROMPT: &[u8] = b"<prompt time=\"1\">&gt;</prompt>\n";
const DEADLINE: Duration = Duration::from_secs(5);

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_mistyped_command_is_answered_and_the_game_never_hears_it() {
    let (source, transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let (_, mut told) = session.subscribe();
    let generation = handle.generation();
    tokio::spawn(session.into_actor().run());

    let ran: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let kept = Arc::clone(&ran);
    let runner: Runner = Arc::new(move |line: &str| {
        kept.lock().map(|mut ran| ran.push(line.to_owned())).ok();
        if line.starts_with("go2 ") {
            Claimed::Done
        } else {
            Claimed::Unknown
        }
    });
    assert!(handle.set_desk(Desk::new(None, runner)));
    assert_eq!(handle.command_symbol(), Some(';'));

    // The game's, and it goes to the game.
    handle.send_manual_at(generation, "north", DEADLINE).await;
    // Hydra's, and known.
    handle
        .send_manual_at(generation, ";go2 bank", DEADLINE)
        .await;
    // Hydra's, and NOT known -- the line the author named.
    handle
        .send_manual_at(generation, ";go22 bank", DEADLINE)
        .await;

    assert_eq!(
        transcript.lines(),
        ["north"],
        "only the game's line reached the game"
    );
    assert_eq!(
        *ran.lock().unwrap(),
        ["go2 bank".to_owned(), "go22 bank".to_owned()],
        "both were offered, less their symbol"
    );

    let mut said = Vec::new();
    while let Ok(event) = told.try_recv() {
        if let Event::Notice(notice) = event {
            said.push(format!("{:?}", notice.body));
        }
    }
    assert_eq!(said.len(), 1, "only the unknown one is answered: {said:?}");
    assert!(said[0].contains(";go22 bank"), "{said:?}");
}

/// With nothing registered, a session behaves exactly as it did before: the
/// symbol means nothing and every line is the game's.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_session_with_no_desk_sends_everything_as_it_always_did() {
    let (source, transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let generation = handle.generation();
    tokio::spawn(session.into_actor().run());

    assert_eq!(handle.command_symbol(), None);
    assert_eq!(handle.typed(";go2 bank"), None);
    handle
        .send_manual_at(generation, ";go2 bank", DEADLINE)
        .await;
    assert_eq!(transcript.lines(), [";go2 bank"]);
}
