//! `;multi` and `;foreach` on Hydra's command line (`plan/30` §4, M6e): the
//! join only.
//!
//! What the words mean and how a batch is sent are `cena_behavior::batch`'s.
//! What is known here is every other command: a batch's `;sc 401` is run
//! through the same [`Commands`] a typed one is, and waited for until what
//! it started is over ([`Took::Started`]), which only the binary can do.
//!
//! Registered at startup with the other always-there families
//! (`crate::sorter`, `crate::loot`), since a batch needs no map and no login
//! state to be understood.
//!
//! # BUILT, NOT RUN
//!
//! As `hunt.rs` says: compiled and tested, never executed against the game.
//! The batch itself is tested against a scripted game in `cena-behavior`;
//! the waiting on a started command is tested here.

use std::sync::Arc;

use cena_behavior::batch::{self, Command, Desk, Hydra, Kind, Ran};
use cena_session::command::claimant::DEFAULT_SYMBOL;
use cena_session::{AuthorityToken, Notice, NoticeKind, SessionHandle, SessionObserver};

use crate::commands::{Commands, Took};

/// `;multi`'s claim on the authority. Travel's is 2, the hunt desk's and
/// the sync's 3; each only has to differ from what runs beside it.
const MULTI_TOKEN: AuthorityToken = AuthorityToken(4);

/// `;foreach`'s. Its own, so a `;multi` can run a `;foreach` and the other
/// way round.
const FOREACH_TOKEN: AuthorityToken = AuthorityToken(5);

/// Register `;multi` and `;foreach` on `handle`'s command line.
pub(crate) fn open(handle: &SessionHandle, observer: &SessionObserver, commands: &Commands) {
    let multi = Desk::new(Kind::Multi.name(), MULTI_TOKEN);
    let foreach = Desk::new(Kind::Foreach.name(), FOREACH_TOKEN);
    let hydra = through(commands.clone());
    let (told, observer) = (handle.clone(), observer.clone());
    commands.batch(Arc::new(move |line: &str| {
        let symbol = told.command_symbol().unwrap_or(DEFAULT_SYMBOL);
        let command = match batch::parse(line, symbol)? {
            Ok(command) => command,
            Err(why) => {
                told.say(Notice::line(NoticeKind::Error, why));
                return Some(Took::Done);
            }
        };
        let desk = |kind| match kind {
            Kind::Multi => Arc::clone(&multi),
            Kind::Foreach => Arc::clone(&foreach),
        };
        match command {
            Command::Help(kind) => {
                told.say(Notice::table(NoticeKind::Info, kind.usage()));
                Some(Took::Done)
            }
            Command::Stop(kind) => {
                if !desk(kind).stop() {
                    told.say(Notice::line(
                        NoticeKind::Info,
                        format!("{}: nothing is running.", kind.name()),
                    ));
                }
                Some(Took::Done)
            }
            Command::Run(job) => {
                let desk = desk(job.kind());
                let (handle, observer, hydra) = (told.clone(), observer.clone(), hydra.clone());
                Some(Took::Started(tokio::spawn(async move {
                    match observer.subscribe().await {
                        Ok(joined) => {
                            if let Some(run) = desk.run(&handle, joined, job, hydra) {
                                let _ = run.await;
                            }
                        }
                        Err(e) => handle.say(Notice::line(
                            NoticeKind::Error,
                            format!(
                                "{}: I could not read the session -- {e:?}.",
                                job.kind().name()
                            ),
                        )),
                    }
                })))
            }
        }
    }));
}

/// How a batch runs a Hydra command: routed as a typed one is, and waited
/// for until what it started is over.
fn through(commands: Commands) -> Hydra {
    Arc::new(move |line: &str| {
        let took = commands.route(line);
        Box::pin(async move {
            match took {
                None => Ran::Unknown,
                Some(Took::Done) => Ran::Done,
                Some(Took::Started(task)) => {
                    let _ = task.await;
                    Ran::Done
                }
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cena_platform::AnsweringSource;
    use cena_session::{CommandId, Event, Frame, Origin, Outcome, Session, State};
    use std::time::Duration;
    use tokio::sync::broadcast::Receiver;

    const PROMPT: &[u8] = b"<prompt time=\"1\">&gt;</prompt>\n";
    const DEADLINE: Duration = Duration::from_secs(5);
    /// What the stand-in walk claims, as travel's desk does.
    const WALK: AuthorityToken = AuthorityToken(2);

    async fn ready(mut events: Receiver<Event>) {
        while let Ok(event) = events.recv().await {
            if event == Event::StateChanged(State::Ready) {
                return;
            }
        }
    }

    fn told(events: &mut Receiver<Event>) -> Vec<String> {
        let mut said = Vec::new();
        while let Ok(event) = events.try_recv() {
            if let Event::Notice(notice) = event {
                said.extend(notice.lines().iter().cloned());
            }
        }
        said
    }

    /// Let virtual time pass in steps, so every loop turns.
    async fn pass(time: Duration) {
        let step = Duration::from_millis(100);
        let mut gone = Duration::ZERO;
        while gone < time {
            tokio::time::advance(step).await;
            tokio::task::yield_now().await;
            gone += step;
        }
    }

    /// `;multi 1,get gem,;go2 bank,put gem in sack`, typed: the walk the
    /// `;go2` starts claims the authority the multi gave back, and the `put`
    /// waits until the walk is over -- through the same command table a
    /// typed `;go2` goes through (`crate::commands`).
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn a_multi_waits_for_what_its_hydra_command_started() {
        let (source, transcript) = AnsweringSource::logged_in(PROMPT);
        let session = Session::new(source);
        let handle = session.handle();
        let observer = session.observer();
        let (_, waiting) = session.subscribe();
        let commands = Commands::install(&handle);
        open(&handle, &observer, &commands);
        let arrive = Arc::new(tokio::sync::Notify::new());
        let (walker, arrived) = (handle.clone(), Arc::clone(&arrive));
        // A stand-in for travel's desk: claim, step, wait to arrive, release.
        commands.travel(Arc::new(move |line: &str| {
            if !line.starts_with("go2") {
                return None;
            }
            let (walker, arrived) = (walker.clone(), Arc::clone(&arrived));
            Some(Took::Started(tokio::spawn(async move {
                if walker.claim(WALK).await.is_err() {
                    return;
                }
                walker
                    .send_and_await(
                        CommandId(7),
                        "north",
                        Origin::Behavior(WALK),
                        DEADLINE,
                        |frame| matches!(frame, Frame::Prompt { .. }),
                    )
                    .await;
                arrived.notified().await;
                walker.release(WALK);
            })))
        }));
        tokio::spawn(session.into_actor().run());
        ready(waiting).await;

        let typed = handle
            .send_manual_at(
                handle.generation(),
                ";multi 1,get gem,;go2 bank,put gem in sack",
                DEADLINE,
            )
            .await;
        assert_eq!(typed, Outcome::Handled);
        pass(Duration::from_secs(40)).await;
        assert_eq!(
            transcript.lines(),
            ["get gem", "north"],
            "the walk stepped with its own claim, and the put waits"
        );
        assert_eq!(handle.holder(), Some(WALK));
        arrive.notify_one();
        pass(Duration::from_secs(2)).await;
        assert_eq!(transcript.lines(), ["get gem", "north", "put gem in sack"]);
        assert_eq!(handle.holder(), None, "all given back");
    }

    /// Help, stop with nothing running, and a line that is not a batch, all
    /// answered without sending anything.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn help_stop_and_a_bad_line_are_answered() {
        let (source, transcript) = AnsweringSource::logged_in(PROMPT);
        let session = Session::new(source);
        let handle = session.handle();
        let observer = session.observer();
        let (_, waiting) = session.subscribe();
        let (_, mut events) = session.subscribe();
        let commands = Commands::install(&handle);
        open(&handle, &observer, &commands);
        tokio::spawn(session.into_actor().run());
        ready(waiting).await;
        let generation = handle.generation();
        for line in [
            ";multi",
            ";foreach stop",
            ";multi 3",
            ";foreach small in bag",
        ] {
            let typed = handle.send_manual_at(generation, line, DEADLINE).await;
            assert_eq!(typed, Outcome::Handled, "{line}");
        }
        let said = told(&mut events);
        assert!(
            said.iter().any(|s| s.starts_with("multi <times>")),
            "{said:?}"
        );
        assert!(said.iter().any(|s| s == "Foreach: nothing is running."));
        assert!(said.iter().any(|s| s.contains("times what")));
        assert!(
            said.iter()
                .any(|s| s.contains("no item type matches 'small'"))
        );
        assert!(transcript.lines().is_empty(), "{:?}", transcript.lines());
    }
}
