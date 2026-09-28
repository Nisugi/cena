//! The Lich relay (`plan/51`, `cena_agent::lich`) against a scripted game.
//!
//! In CI, a stand-in for Lich in pipe mode (`fixtures/standin_lich.rb`), which
//! does what the relay relies on and nothing of Lich's engine. The player's
//! real Lich is `the_real_lich`, ignored unless asked for: it needs a Lich
//! checkout, which CI does not have (`reference/` is not in the repository).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use cena_agent::lich::{Ended, Launch, run};
use cena_agent::scripts::runner::find_ruby;
use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::{Event, Origin, Outcome, Session, SessionHandle};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

/// Long enough for Ruby to start, and for the real Lich to start offline.
const DEADLINE: Duration = Duration::from_secs(90);

/// A character whose Lich is `launch`, started before the game speaks.
struct Character {
    handle: SessionHandle,
    transcript: TranscriptHandle,
    events: broadcast::Receiver<Event>,
    stop: CancellationToken,
    relay: tokio::task::JoinHandle<Ended>,
}

impl Character {
    fn start(launch: Launch) -> Self {
        let (source, transcript) =
            AnsweringSource::logged_in(b"<prompt time=\"1\">&gt;</prompt>\n");
        let session = Session::new(source);
        let handle = session.handle();
        let (_, events) = session.subscribe();
        let stop = CancellationToken::new();
        // Attached here, before the actor runs and the login comes.
        let relay = tokio::spawn(run(handle.lich_door(), launch, stop.clone()));
        tokio::spawn(session.into_actor().run());
        Self {
            handle,
            transcript,
            events,
            stop,
            relay,
        }
    }

    /// The player types `line` at a frontend.
    async fn types(&self, line: &str) -> Outcome {
        let generation = self.handle.generation();
        self.handle.send_typed_at(generation, line, DEADLINE).await
    }

    /// The first event `wanted` finds something in, from here on.
    async fn next<T>(&mut self, mut wanted: impl FnMut(Event) -> Option<T>) -> Option<T> {
        tokio::time::timeout(DEADLINE, async {
            loop {
                match self.events.recv().await {
                    Ok(event) => {
                        if let Some(found) = wanted(event) {
                            return Some(found);
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => return None,
                }
            }
        })
        .await
        .ok()
        .flatten()
    }

    /// Who sent `line` to the game, once it has been sent.
    async fn sent(&mut self, line: &str) -> Option<Origin> {
        self.next(|event| match event {
            Event::Sent { line: sent, origin } if sent == line => Some(origin),
            _ => None,
        })
        .await
    }

    /// What the player was told, the first time it has `words` in it.
    async fn told(&mut self, words: &str) -> Option<String> {
        self.next(|event| match event {
            Event::Notice(notice) => notice
                .lines()
                .iter()
                .find(|line| line.contains(words))
                .cloned(),
            _ => None,
        })
        .await
    }

    async fn stop(self) -> Option<Ended> {
        self.stop.cancel();
        tokio::time::timeout(DEADLINE, self.relay).await.ok()?.ok()
    }
}

fn standin(ruby: PathBuf) -> Launch {
    Launch {
        ruby,
        lich: Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/standin_lich.rb"),
        args: Vec::new(),
        env: Vec::new(),
    }
}

/// Lich takes the relay as its game: it is handed the login, and its
/// script's line goes to the game as Lich's. What the player types goes to
/// it: a `;` line starts one of its scripts, and a plain one meets its alias
/// and goes to the game as the player's. It stops when asked. Hydra's symbol
/// here is Lich's, and the player is told what that means.
#[tokio::test(flavor = "multi_thread")]
async fn lich_takes_hydra_as_its_game() {
    let ruby = find_ruby().expect("Ruby, which CI installs");
    let mut character = Character::start(standin(ruby));

    let told = character.told("Lich's commands can't be reached").await;
    assert!(told.is_some_and(|line| line.contains("such as .")));

    // Put once the login's prompt reached Lich: the copy missed nothing.
    assert_eq!(character.sent("look").await, Some(Origin::Lich));
    // Pipe mode's `unknown`, put back.
    assert_eq!(
        character.sent("frontend stormfront").await,
        Some(Origin::Lich)
    );

    // No Hydra commands run here, so `;` is Lich's.
    assert_eq!(character.types(";put look around").await, Outcome::Handled);
    assert_eq!(character.sent("look around").await, Some(Origin::Lich));
    assert_eq!(character.types("gg").await, Outcome::Handled, "Lich's");
    assert_eq!(character.sent("get gem").await, Some(Origin::Manual));

    assert_eq!(
        character.transcript.lines(),
        ["look", "frontend stormfront", "look around", "get gem"]
    );
    assert_eq!(character.stop().await, Some(Ended::Stopped));
}

/// One Lich per character: a second is refused while the first runs.
#[tokio::test(flavor = "multi_thread")]
async fn a_second_lich_for_a_character_is_refused() {
    let ruby = find_ruby().expect("Ruby, which CI installs");
    let character = Character::start(standin(ruby.clone()));
    let door = character.handle.lich_door();
    let second = run(door, standin(ruby), CancellationToken::new()).await;
    assert_eq!(second, Ended::Busy);
    assert_eq!(character.stop().await, Some(Ended::Stopped));
}

/// The player's own Lich, offline. Run it with the checkout's folder:
///
/// ```text
/// HYDRA_TEST_LICH=G:\dev\Cena\reference\lich-5 cargo test -p cena-agent --test lich_relay -- --ignored
/// ```
///
/// It runs from a fresh home in a temporary folder, never the checkout's
/// own, and with a proxy that refuses, since Lich reaches GitHub at every
/// login (`plan/51` §4, item 5).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a Lich checkout: HYDRA_TEST_LICH"]
async fn the_real_lich() {
    let ruby = find_ruby().expect("Ruby");
    let lich = PathBuf::from(std::env::var_os("HYDRA_TEST_LICH").expect("HYDRA_TEST_LICH"));
    let home = std::env::temp_dir().join(format!("cena-lich-relay-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let mut args = vec![OsString::from(format!(
        "--lib={}",
        lich.join("lib").display()
    ))];
    for name in ["home", "data", "temp", "scripts", "maps", "logs", "backup"] {
        let folder = if name == "home" {
            home.clone()
        } else {
            home.join(name)
        };
        std::fs::create_dir_all(&folder).unwrap();
        args.push(OsString::from(format!("--{name}={}", folder.display())));
    }
    let refuse = OsString::from("http://127.0.0.1:9");
    let mut character = Character::start(Launch {
        ruby,
        lich: lich.join("lich.rbw"),
        args,
        env: vec![
            ("https_proxy".into(), refuse.clone()),
            ("http_proxy".into(), refuse),
        ],
    });

    // No Hydra commands run here, so `;` is Lich's.
    character.types(r#";e put "frontend #{$frontend}""#).await;
    assert_eq!(
        character.sent("frontend stormfront").await,
        Some(Origin::Lich)
    );
    character.types("exp").await;
    assert_eq!(character.sent("exp").await, Some(Origin::Manual));

    assert_eq!(character.stop().await, Some(Ended::Stopped));
    let _ = std::fs::remove_dir_all(&home);
}
