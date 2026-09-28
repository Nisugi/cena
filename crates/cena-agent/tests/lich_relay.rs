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
use cena_session::{Event, Origin, Outcome, Session, SessionHandle, State};
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
    /// The game answers each of `answers`' commands with its bytes, and the
    /// rest with a prompt.
    fn start(launch: Launch, answers: &[(&str, &[u8])]) -> Self {
        let (source, transcript) =
            AnsweringSource::logged_in(b"<prompt time=\"1\">&gt;</prompt>\n");
        for (command, reply) in answers {
            transcript.answer(command, reply);
        }
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

    /// A character already logged in, named, and in a room, when its Lich
    /// starts, as when the player switches Lich on mid-session.
    async fn start_late(launch: Launch) -> Option<Self> {
        let (source, transcript) =
            AnsweringSource::logged_in(b"<prompt time=\"1\">&gt;</prompt>\n");
        let session = Session::new(source);
        let handle = session.handle();
        let (_, mut events) = session.subscribe();
        tokio::spawn(session.into_actor().run());
        let ready = async {
            loop {
                match events.recv().await {
                    Ok(Event::StateChanged(State::Ready)) => return true,
                    Err(broadcast::error::RecvError::Closed) => return false,
                    Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                }
            }
        };
        if !tokio::time::timeout(DEADLINE, ready).await.ok()? {
            return None;
        }
        // The game names the character, as a login does, so a Lich started
        // now has something to be told (`GameState::login`). The instance's
        // name is assembled: Rule 3.4's scan flags it spelled.
        let named = format!(
            "<playerID id='5'/><settingsInfo instance='{}'/><app char=\"Tester\" game=\"Prime\"/>
             <nav rm='7'/><streamWindow id='room' subtitle=' - [Town Square]'/>             <left exist=\"1\" noun=\"gem\">a gem</left>
<prompt time=\"2\">&gt;</prompt>
",
            concat!("GS", "4")
        );
        transcript.answer("look", named.as_bytes());
        let _ = handle
            .send_manual_at(handle.generation(), "look", DEADLINE)
            .await;
        // What Lich does is read from here on.
        let events = events.resubscribe();
        let stop = CancellationToken::new();
        let relay = tokio::spawn(run(handle.lich_door(), launch, stop.clone()));
        Some(Self {
            handle,
            transcript,
            events,
            stop,
            relay,
        })
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

    /// The lines a viewer is shown, from here on until one says `words`.
    async fn shown_until(&mut self, words: &str) -> Vec<String> {
        let mut shown = Vec::new();
        self.next(|event| match event {
            Event::Line(line) => {
                shown.push(line.text());
                line.text().contains(words).then_some(())
            }
            _ => None,
        })
        .await;
        shown
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
    let mut character = Character::start(standin(ruby), &[]);

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

/// What Lich shows is the character's text: the game's, less the line it
/// hid, and what its script says. The game's own copy is not shown too.
#[tokio::test(flavor = "multi_thread")]
async fn what_lich_shows_is_the_characters_text() {
    let ruby = find_ruby().expect("Ruby, which CI installs");
    let look = b"The room.\r\nA line to hide me.\r\n<prompt time=\"2\">&gt;</prompt>\r\n";
    // Its script looks once it has seen the login's prompt.
    let mut character = Character::start(standin(ruby), &[("look", look)]);
    assert_eq!(character.shown_until("The room").await, ["The room."]);
    character.types(";echo Hello from Lich.").await;
    assert_eq!(
        character.shown_until("Hello").await,
        ["Hello from Lich."],
        "the hidden line never, and the game's copy of neither"
    );
    assert_eq!(character.stop().await, Some(Ended::Stopped));
}

/// A Lich started once the character is logged in is handed a login built
/// from what the session knows: it sees that login's prompt, and its script
/// looks.
#[tokio::test(flavor = "multi_thread")]
async fn a_lich_started_late_is_handed_a_login() {
    let ruby = find_ruby().expect("Ruby, which CI installs");
    let mut character = Character::start_late(standin(ruby))
        .await
        .expect("logged in");
    assert_eq!(character.sent("look").await, Some(Origin::Lich));
    assert_eq!(character.stop().await, Some(Ended::Stopped));
}

/// One Lich per character: a second is refused while the first runs.
#[tokio::test(flavor = "multi_thread")]
async fn a_second_lich_for_a_character_is_refused() {
    let ruby = find_ruby().expect("Ruby, which CI installs");
    let character = Character::start(standin(ruby.clone()), &[]);
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
    // Started once the character is logged in, so it is handed the login.
    let mut character = Character::start_late(Launch {
        ruby,
        lich: lich.join("lich.rbw"),
        args,
        env: vec![
            ("https_proxy".into(), refuse.clone()),
            ("http_proxy".into(), refuse),
        ],
    })
    .await
    .expect("logged in");

    // Lich has read the login it was handed once it answers its `<app>`
    // (`reference/lich-5/lib/common/xmlparser.rb:966`); it starts before
    // its parser catches up, and runs what is typed meanwhile.
    assert_eq!(
        character.sent("_flag Display Inventory Boxes 1").await,
        Some(Origin::Lich)
    );
    // No Hydra commands run here, so `;` is Lich's.
    character.types(r#";e put "frontend #{$frontend}""#).await;
    assert_eq!(
        character.sent("frontend stormfront").await,
        Some(Origin::Lich)
    );
    character.types("exp").await;
    assert_eq!(character.sent("exp").await, Some(Origin::Manual));
    // It knows the character from the login it was handed: who, which
    // game, where, and what is in hand. Asked until it says, since loading
    // its game's modules after the first line takes it seconds.
    let mut said = None;
    for _ in 0..30 {
        character
            .types(r#";e put "is #{XMLData.name} #{XMLData.game} #{XMLData.room_id} #{GameObj.left_hand.noun}""#)
            .await;
        said = character
            .next(|event| match event {
                Event::Sent { line, .. } if line.starts_with("is ") => Some(line),
                _ => None,
            })
            .await;
        if said.as_deref() == Some("is Tester GSIV 7 gem") {
            break;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    assert_eq!(said.as_deref(), Some("is Tester GSIV 7 gem"));
    // What its script shows is the character's text.
    character.types(r#";e respond "Hello from Lich.""#).await;
    let shown = character.shown_until("Hello from Lich.").await;
    assert!(
        shown
            .last()
            .is_some_and(|line| line.contains("Hello from Lich.")),
        "{shown:?}"
    );

    assert_eq!(character.stop().await, Some(Ended::Stopped));
    let _ = std::fs::remove_dir_all(&home);
}
