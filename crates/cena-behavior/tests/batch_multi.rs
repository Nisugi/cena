//! `;multi` (`plan/30` §7, M6e): multi.lic's list, parsed, and sent against
//! a scripted game through the batch desk.

mod ready;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use cena_behavior::BehaviorError;
use cena_behavior::batch::{self, Command, Desk, Halt, Hydra, Job, Kind, Line, Ran, multi};
use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::{
    AuthorityToken, CommandId, Event, Frame, Origin, Session, SessionHandle, SessionObserver,
};
use tokio::sync::broadcast::Receiver;

/// What the game says to a command nothing was scripted for.
const PROMPT: &[u8] = b"<prompt time=\"1\">&gt;</prompt>\n";
const TOKEN: AuthorityToken = AuthorityToken(4);
/// Another claimant: what a Hydra command in the list starts.
const OTHER: AuthorityToken = AuthorityToken(9);

fn send(text: &str) -> Line {
    Line::Send(text.to_owned())
}

fn parsed(line: &str) -> Result<Command, String> {
    batch::parse(line, ';').ok_or_else(|| format!("{line:?} was not a batch"))?
}

fn run_of(line: &str) -> Result<multi::Multi, String> {
    match parsed(line)? {
        Command::Run(Job::Multi(multi)) => Ok(multi),
        other => Err(format!("{line:?} parsed as {other:?}")),
    }
}

#[test]
fn the_count_first_then_the_list() {
    let multi = run_of("multi 3,get gem,sell gem").unwrap();
    assert_eq!(multi.times, 3);
    assert_eq!(multi.lines, [send("get gem"), send("sell gem")]);
}

/// `multi.lic:53`: the count may come last.
#[test]
fn the_count_may_come_last() {
    let multi = run_of("multi get gem,sell gem,2").unwrap();
    assert_eq!(multi.times, 2);
    assert_eq!(multi.lines, [send("get gem"), send("sell gem")]);
}

/// `multi.lic:50`: with no comma, semicolons are the commas.
#[test]
fn with_no_comma_semicolons_split() {
    let multi = run_of("multi 2;order 1;buy").unwrap();
    assert_eq!(multi.times, 2);
    assert_eq!(multi.lines, [send("order 1"), send("buy")]);
}

/// multi.lic's own example (`:44`), with a Hydra command where it ran a
/// script; and with a character whose symbol is not `;`.
#[test]
fn an_entry_with_the_symbol_is_a_hydra_command() {
    let multi = run_of("multi 2,get diamond in pouch,;sc 401,put diamond in sack").unwrap();
    assert_eq!(
        multi.lines,
        [
            send("get diamond in pouch"),
            Line::Hydra("sc 401".to_owned()),
            send("put diamond in sack"),
        ]
    );
    let Some(Ok(Command::Run(Job::Multi(slashed)))) = batch::parse("multi 1,/heal,look", '/')
    else {
        panic!("a / symbol did not parse");
    };
    assert_eq!(
        slashed.lines,
        [Line::Hydra("heal".to_owned()), send("look")]
    );
}

/// Trimmed, and a blank skipped: multi.lic sent ` sell gem` and blank lines.
#[test]
fn entries_are_trimmed_and_blanks_skipped() {
    let multi = run_of("multi 2, get gem ,, sell gem ").unwrap();
    assert_eq!(multi.lines, [send("get gem"), send("sell gem")]);
}

#[test]
fn what_cannot_run_is_refused_with_why() {
    for (line, says) in [
        ("multi get gem,sell gem", "how many times"),
        ("multi 0,look", "zero"),
        ("multi 3", "times what"),
        ("multi 99999999999,look", "more times"),
        ("multi 2,look,;multi 2,look", "inside this one"),
        ("multi 2,look,;", "names no command"),
    ] {
        let why = parsed(line).expect_err(line);
        assert!(why.contains(says), "{line}: {why}");
    }
}

#[test]
fn help_stop_and_other_words() {
    assert!(matches!(parsed("multi"), Ok(Command::Help(Kind::Multi))));
    assert!(matches!(
        parsed("multi help"),
        Ok(Command::Help(Kind::Multi))
    ));
    assert!(matches!(
        parsed("MULTI stop"),
        Ok(Command::Stop(Kind::Multi))
    ));
    assert!(batch::parse("multiply 3,look", ';').is_none());
    assert!(batch::parse("go2 bank", ';').is_none());
}

/// A character logged in on a scripted game, and a way in.
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
    /// Run `job` on `desk` from a fresh subscription, as the binary does.
    async fn run_on(
        &self,
        desk: &Arc<Desk>,
        job: Job,
        hydra: Hydra,
    ) -> Result<Option<tokio::task::JoinHandle<Result<(), Halt>>>, String> {
        let joined = self
            .observer
            .subscribe()
            .await
            .map_err(|e| format!("{e:?}"))?;
        Ok(desk.run(&self.handle, joined, job, hydra))
    }

    /// Run `job` on a fresh desk; the desk too, to stop it.
    async fn run(
        &self,
        job: Job,
        hydra: Hydra,
    ) -> Result<(Arc<Desk>, tokio::task::JoinHandle<Result<(), Halt>>), String> {
        let desk = Desk::new("Multi", TOKEN);
        let run = self
            .run_on(&desk, job, hydra)
            .await?
            .ok_or("the desk would not run it")?;
        Ok((desk, run))
    }

    /// Everything Hydra said, as text.
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

/// A Hydra command nothing knows.
fn nobody() -> Hydra {
    Arc::new(|_: &str| Box::pin(async { Ran::Unknown }))
}

/// Let virtual time run until `count` lines have been written. `false` if
/// they never are.
async fn until_written(transcript: &TranscriptHandle, count: usize) -> bool {
    for _ in 0..400 {
        if transcript.written_count() >= count {
            return true;
        }
        tokio::time::advance(Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
    }
    false
}

/// Let `time` go by in small steps, so every loop turns as it would.
async fn pass(time: Duration) {
    let step = Duration::from_millis(100);
    let mut gone = Duration::ZERO;
    while gone < time {
        tokio::time::advance(step).await;
        tokio::task::yield_now().await;
        gone += step;
    }
}

fn job(times: u32, lines: Vec<Line>) -> Job {
    Job::Multi(multi::Multi { times, lines })
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_list_is_sent_so_many_times_in_order() {
    let mut game = logged_in().await.unwrap();
    let (_, run) = game
        .run(job(2, vec![send("get gem"), send("sell gem")]), nobody())
        .await
        .unwrap();
    assert_eq!(run.await.unwrap(), Ok(()));
    assert_eq!(
        game.transcript.lines(),
        ["get gem", "sell gem", "get gem", "sell gem"]
    );
    assert_eq!(game.handle.holder(), None, "released at the end");
    assert!(
        game.said().iter().any(|s| s == "Multi: done, 2 times."),
        "said it was done"
    );
}

/// The heart of `;multi 2,get gem,;sc 401,put gem`: the Hydra command runs
/// with the authority given back -- what it starts claims its own -- and the
/// list goes on after, holding the authority again.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_hydra_command_runs_with_the_authority_given_back() {
    let game = logged_in().await.unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let (handle, kept) = (game.handle.clone(), Arc::clone(&seen));
    let hydra: Hydra = Arc::new(move |line: &str| {
        let (handle, kept, line) = (handle.clone(), Arc::clone(&kept), line.to_owned());
        Box::pin(async move {
            // What a started behavior does: claim, send, release.
            let claimed = handle.claim(OTHER).await.is_ok();
            handle
                .send_and_await(
                    CommandId(1),
                    &format!("incant {}", line.trim_start_matches("sc ")),
                    Origin::Behavior(OTHER),
                    Duration::from_secs(5),
                    |frame| matches!(frame, Frame::Prompt { .. }),
                )
                .await;
            handle.release(OTHER);
            kept.lock().unwrap().push((line, claimed));
            Ran::Done
        })
    });
    let lines = vec![
        send("get gem"),
        Line::Hydra("sc 401".to_owned()),
        send("put gem in sack"),
    ];
    let (_, run) = game.run(job(1, lines), hydra).await.unwrap();
    assert_eq!(run.await.unwrap(), Ok(()));
    assert_eq!(
        *seen.lock().unwrap(),
        [("sc 401".to_owned(), true)],
        "the command's own claim was granted"
    );
    assert_eq!(
        game.transcript.lines(),
        ["get gem", "incant 401", "put gem in sack"]
    );
}

/// The list does not go on until what the Hydra command started is over.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_next_line_waits_for_the_command_to_be_over() {
    let game = logged_in().await.unwrap();
    let (finish, finished) = tokio::sync::oneshot::channel::<()>();
    let finished = Arc::new(Mutex::new(Some(finished)));
    let hydra: Hydra = Arc::new(move |_: &str| {
        let finished = finished.lock().unwrap().take();
        Box::pin(async move {
            if let Some(finished) = finished {
                let _ = finished.await;
            }
            Ran::Done
        })
    });
    let lines = vec![
        send("get gem"),
        Line::Hydra("heal".to_owned()),
        send("put gem"),
    ];
    let (_, run) = game.run(job(1, lines), hydra).await.unwrap();
    assert!(until_written(&game.transcript, 1).await);
    // Long enough for a list that did not wait to have sent the next line,
    // and past the watchdog: the wait keeps beating.
    pass(Duration::from_secs(40)).await;
    assert_eq!(game.transcript.lines(), ["get gem"], "still waiting");
    finish.send(()).unwrap();
    assert_eq!(run.await.unwrap(), Ok(()));
    assert_eq!(game.transcript.lines(), ["get gem", "put gem"]);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn an_unknown_hydra_command_stops_the_list_and_says_so() {
    let mut game = logged_in().await.unwrap();
    let lines = vec![
        send("get gem"),
        Line::Hydra("nosuch".to_owned()),
        send("put gem"),
    ];
    let (_, run) = game.run(job(2, lines), nobody()).await.unwrap();
    let Err(Halt::Failed(why)) = run.await.unwrap() else {
        panic!("an unknown command did not stop the list");
    };
    assert!(why.contains(";nosuch"), "{why}");
    assert_eq!(game.transcript.lines(), ["get gem"]);
    assert!(
        game.said()
            .iter()
            .any(|s| s.contains("I do not know ;nosuch"))
    );
    assert_eq!(game.handle.holder(), None, "released after all");
}

/// The game's `...wait N` is waited out and the line sent again, as `fput`
/// does (`global_defs.rb:1556-1565`).
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_wait_from_the_game_is_waited_out_and_the_line_resent() {
    let game = logged_in().await.unwrap();
    game.transcript.answer(
        "sell gem",
        b"...wait 2 seconds.\n<prompt time=\"1\">&gt;</prompt>\n",
    );
    let (_, run) = game
        .run(job(1, vec![send("sell gem")]), nobody())
        .await
        .unwrap();
    assert_eq!(run.await.unwrap(), Ok(()));
    assert_eq!(game.transcript.lines(), ["sell gem", "sell gem"]);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn stopped_mid_list_it_sends_nothing_more() {
    let mut game = logged_in().await.unwrap();
    game.transcript.hold_replies();
    let lines = vec![send("get gem"), send("sell gem")];
    let (desk, run) = game.run(job(5, lines), nobody()).await.unwrap();
    assert!(until_written(&game.transcript, 1).await);
    assert!(desk.stop(), "something was running");
    assert_eq!(
        run.await.unwrap(),
        Err(Halt::Stopped(BehaviorError::Cancelled))
    );
    game.transcript.release_replies();
    pass(Duration::from_secs(5)).await;
    assert_eq!(game.transcript.lines(), ["get gem"]);
    assert!(game.said().iter().any(|s| s == "Multi: stopped."));
    assert!(!desk.stop(), "nothing left to stop");
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn nothing_is_sent_while_something_else_holds_the_session() {
    let mut game = logged_in().await.unwrap();
    game.handle.claim(OTHER).await.unwrap();
    let (_, run) = game
        .run(job(1, vec![send("look")]), nobody())
        .await
        .unwrap();
    assert_eq!(
        run.await.unwrap(),
        Err(Halt::Stopped(BehaviorError::AuthorityHeld))
    );
    assert!(game.transcript.lines().is_empty());
    assert!(
        game.said()
            .iter()
            .any(|s| s.contains("something else holds"))
    );
    assert_eq!(game.handle.holder(), Some(OTHER), "and it was not taken");
}

/// A second of the same kind, while the first runs, is refused and says
/// how to stop the first -- Lich's "already running", `VellumFE`'s too.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_second_of_the_kind_is_refused_while_one_runs() {
    let mut game = logged_in().await.unwrap();
    game.transcript.hold_replies();
    let desk = Desk::new("Multi", TOKEN);
    let first = game
        .run_on(&desk, job(1, vec![send("look")]), nobody())
        .await
        .unwrap()
        .unwrap();
    assert!(until_written(&game.transcript, 1).await);
    let second = game
        .run_on(&desk, job(1, vec![send("sell gem")]), nobody())
        .await
        .unwrap();
    assert!(second.is_none(), "refused");
    assert!(
        game.said()
            .iter()
            .any(|s| s.contains("already running") && s.contains("multi stop"))
    );
    assert!(desk.stop());
    assert_eq!(
        first.await.unwrap(),
        Err(Halt::Stopped(BehaviorError::Cancelled))
    );
    // Over, so the desk runs again.
    game.transcript.release_replies();
    let third = game
        .run_on(&desk, job(1, vec![send("sell gem")]), nobody())
        .await
        .unwrap();
    assert_eq!(third.unwrap().await.unwrap(), Ok(()));
    assert_eq!(game.transcript.lines(), ["look", "sell gem"]);
}

/// A roundtime past the settle cap: the line is tried, the gate refuses it,
/// and it is tried again -- not skipped -- until the roundtime ends (Lich's
/// `fput` resends; a list has no next tick to decide afresh).
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_refused_line_waits_and_is_sent_again() {
    let game = logged_in().await.unwrap();
    let generation = game.handle.generation();
    let manual = |line: &'static str| {
        let handle = game.handle.clone();
        async move {
            handle
                .send_manual_at(generation, line, Duration::from_secs(5))
                .await
        }
    };
    // Roundtime to a second the model's clock will not reach in this test.
    game.transcript.answer(
        "look",
        b"<roundTime value='100000'/>\n<prompt time=\"1\">&gt;</prompt>\n",
    );
    manual("look").await;
    let (_, run) = game
        .run(job(1, vec![send("sell gem")]), nobody())
        .await
        .unwrap();
    pass(Duration::from_secs(20)).await;
    assert_eq!(game.transcript.lines(), ["look"], "refused, and not sent");
    // The roundtime ends.
    game.transcript.answer(
        "glance",
        b"<roundTime value='1'/>\n<prompt time=\"2\">&gt;</prompt>\n",
    );
    manual("glance").await;
    pass(Duration::from_secs(2)).await;
    assert_eq!(run.await.unwrap(), Ok(()));
    assert_eq!(game.transcript.lines(), ["look", "glance", "sell gem"]);
}

/// Refused for [`batch::REFUSED_CAP`], a line is given up, and the rest with it.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_line_refused_too_long_is_given_up() {
    let mut game = logged_in().await.unwrap();
    game.transcript.answer(
        "look",
        b"<roundTime value='100000'/>\n<prompt time=\"1\">&gt;</prompt>\n",
    );
    game.handle
        .send_manual_at(game.handle.generation(), "look", Duration::from_secs(5))
        .await;
    let (_, run) = game
        .run(job(1, vec![send("sell gem"), send("look")]), nobody())
        .await
        .unwrap();
    pass(batch::REFUSED_CAP + Duration::from_secs(20)).await;
    let Err(Halt::Failed(why)) = run.await.unwrap() else {
        panic!("a line refused for ever was not given up");
    };
    assert!(
        why.contains("`sell gem` was refused") && why.contains("Roundtime"),
        "{why}"
    );
    assert_eq!(game.transcript.lines(), ["look"]);
    assert!(game.said().iter().any(|s| s.contains("was refused")));
}
