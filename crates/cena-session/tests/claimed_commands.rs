//! The player's own commands, through the session a frontend uses
//! (`command::claimant`). The rule under test is the author's, 2026-09-21:
//! **the symbol decides**, so a mistyped command is never spoken in the room.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use cena_platform::AnsweringSource;
use cena_session::command::claimant::{Claimed, Desk, Runner};
use cena_session::{Event, Origin, Session};

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
    let runner: Runner = Arc::new(move |line: &str, _: Origin| {
        kept.lock().map(|mut ran| ran.push(line.to_owned())).ok();
        if line.starts_with("go2 ") {
            Claimed::Done
        } else {
            Claimed::Unknown
        }
    });
    assert!(handle.set_desk(Desk::new(None, runner)));
    assert_eq!(handle.command_symbol(), Some('.'));

    // The game's, and it goes to the game.
    let north = handle.send_manual_at(generation, "north", DEADLINE).await;
    // Hydra's, and known.
    let known = handle
        .send_manual_at(generation, ".go2 bank", DEADLINE)
        .await;
    // Hydra's, and NOT known -- the line the author named.
    let unknown = handle
        .send_manual_at(generation, ".go22 bank", DEADLINE)
        .await;

    // Said by the outcome, not by a frame nobody sent: these were answered
    // `Confirmed(Prompt)`, and a frontend told them apart by the `;`.
    assert_ne!(north, cena_session::Outcome::Handled);
    assert_eq!(known, cena_session::Outcome::Handled);
    assert_eq!(unknown, cena_session::Outcome::Handled);

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
    assert!(said[0].contains(".go22 bank"), "{said:?}");
}

/// **A claimed line checks the generation before it runs.**
///
/// The actor's generation fence only sees what reaches the actor, and a
/// claimed line never does: `;go2 bank` typed into a browser still showing
/// the previous connection ran the desk anyway, and came back `Confirmed`
/// (review finding 8).
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_claimed_line_from_a_stale_generation_does_not_run() {
    let (source, transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let stale = handle.generation();
    tokio::spawn(session.into_actor().run());

    let ran: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let kept = Arc::clone(&ran);
    let runner: Runner = Arc::new(move |line: &str, _: Origin| {
        kept.lock().map(|mut ran| ran.push(line.to_owned())).ok();
        Claimed::Done
    });
    assert!(handle.set_desk(Desk::new(None, runner)));
    // What a reconnect does to every handle: the frontend is now a
    // generation behind.
    let _ = handle.generation_cell().advance();

    let outcome = handle.send_manual_at(stale, ".go2 bank", DEADLINE).await;
    assert_eq!(outcome, cena_session::Outcome::Disconnected);
    assert!(
        ran.lock().unwrap().is_empty(),
        "the desk ran a command addressed to a connection that is gone"
    );
    assert!(transcript.lines().is_empty());
}

/// With nothing registered, Hydra's symbol means nothing and every line is
/// the game's, but for Lich's: a line with `;` is the player's Lich's, never
/// the game's (`plan/51` §6, question 3, the author: *"command starting with
/// the lich command character ; get sent to lich"*).
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_session_with_no_desk_sends_everything_but_lichs() {
    let (source, transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let generation = handle.generation();
    tokio::spawn(session.into_actor().run());

    assert_eq!(handle.command_symbol(), None);
    assert_eq!(handle.typed("/go2 bank", Origin::Manual), None);
    let sent = handle
        .send_manual_at(generation, "/go2 bank", DEADLINE)
        .await;
    assert_ne!(sent, cena_session::Outcome::Handled);
    let lichs = handle
        .send_manual_at(generation, ";go2 bank", DEADLINE)
        .await;
    assert_eq!(lichs, cena_session::Outcome::Handled, "no Lich: told so");
    assert_eq!(transcript.lines(), ["/go2 bank"]);
}

/// **The desk goes in at startup and learns the character's symbol later.**
/// A character whose settings choose `/` gets `/` from then on, and `;` goes
/// back to being Lich's -- without a second desk, which `set_desk` refuses.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_symbol_can_change_after_the_desk_is_installed() {
    let (source, transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let generation = handle.generation();
    tokio::spawn(session.into_actor().run());

    let runner: Runner = Arc::new(|_: &str, _: Origin| Claimed::Done);
    assert!(handle.set_desk(Desk::new(None, runner)));
    assert_eq!(handle.command_symbol(), Some('.'));

    assert!(handle.set_command_symbol('/'));
    assert_eq!(handle.command_symbol(), Some('/'));
    assert_eq!(
        handle
            .send_manual_at(generation, "/go2 bank", DEADLINE)
            .await,
        cena_session::Outcome::Handled
    );
    let mut lich = handle.lich_door().attach().expect("a Lich");
    assert_eq!(
        handle
            .send_manual_at(generation, ";go2 bank", DEADLINE)
            .await,
        cena_session::Outcome::Handled
    );
    assert_eq!(
        lich.typing.next().await.as_deref(),
        Some(";go2 bank"),
        "once the symbol is `/`, `;` is Lich's"
    );
    assert!(transcript.lines().is_empty());
}
