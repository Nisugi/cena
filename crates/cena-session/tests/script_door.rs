//! A script's way in (`plan/46`, M7b step 1): what a script reads is the
//! game's line, whatever the player's triggers show; what it sends goes out
//! as the script's, and says the cursor a reader finds it at; and a line
//! marked as Hydra's never reaches the game.

use std::sync::Arc;
use std::time::Duration;

use cena_platform::{AnsweringSource, ReplaySource};
use cena_session::command::claimant::{Claimed, Desk, Runner};
use cena_session::script::Sending;
use cena_session::trigger::{Matcher, Pattern, Rule, Trigger};
use cena_session::{Event, Origin, Session};

const DEADLINE: Duration = Duration::from_secs(5);

const SWING: &str = concat!(
    "You swing a broadsword at <pushBold/><a exist=\"101\" noun=\"lizard\">a cave lizard</a><popBold/>!\n",
    "   ... and hit for 40 points of damage!\n",
    "<prompt time=\"1000\">&gt;</prompt>\n",
);

fn squelching(text: &str) -> Option<Matcher> {
    Matcher::new(vec![Trigger {
        name: "hide".into(),
        rule: Rule {
            pattern: Some(Pattern::Literal {
                text: text.into(),
                whole_word: true,
            }),
            squelch: true,
            ..Rule::default()
        },
    }])
    .ok()
}

/// Every event the session published over `SWING`, a trigger squelching the
/// swing, with a runner listening or not.
async fn published(listening: bool, triggers: Matcher) -> Vec<Event> {
    let session = Session::new(ReplaySource::from_bytes(SWING.as_bytes()));
    session.handle().set_triggers(triggers);
    session.handle().script_door().listen(listening);
    let (_, mut events) = session.subscribe();
    let _ = Box::pin(session.into_actor().run()).await;
    std::iter::from_fn(|| events.try_recv().ok()).collect()
}

/// **A squelch hides a line from the player, never from a script**: Lich's
/// scripts see each line before its hooks change what is shown
/// (`inventory/13` §1.6). Each heard line comes just before what the
/// viewers are given of it.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_script_hears_the_games_line_whatever_the_triggers_show() {
    let events = published(true, squelching("You swing").unwrap()).await;
    let heard: Vec<String> = events
        .iter()
        .filter_map(|event| match event {
            Event::Heard(line) => Some(line.text()),
            _ => None,
        })
        .collect();
    assert_eq!(
        heard,
        [
            "You swing a broadsword at a cave lizard!",
            "   ... and hit for 40 points of damage!"
        ]
    );
    let shown: Vec<String> = events
        .iter()
        .filter_map(|event| match event {
            Event::Line(line) => Some(line.text()),
            _ => None,
        })
        .collect();
    assert_eq!(
        shown,
        ["   ... and hit for 40 points of damage!"],
        "squelched"
    );
    let damage = |event: &Event| match event {
        Event::Heard(line) | Event::Line(line) => line.text().contains("40 points"),
        _ => false,
    };
    let order: Vec<&str> = events
        .iter()
        .filter(|event| damage(event))
        .map(|event| match event {
            Event::Heard(_) => "heard",
            _ => "shown",
        })
        .collect();
    assert_eq!(order, ["heard", "shown"]);
}

/// A character nobody scripts publishes each line once.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn nothing_is_heard_while_no_runner_listens() {
    let events = published(false, squelching("You swing").unwrap()).await;
    assert!(!events.iter().any(|event| matches!(event, Event::Heard(_))));
    assert!(events.iter().any(|event| matches!(event, Event::Line(_))));
}

/// A script's line goes to the game as the script's, and the cursor it
/// answers is the one its `Sent` carries on the numbered stream. A line
/// marked as Hydra's is Hydra's, known or not, and the game never hears it.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_scripts_line_goes_out_as_the_scripts_and_says_where() {
    let (source, transcript) = AnsweringSource::logged_in(b"<prompt time=\"1\">&gt;</prompt>\n");
    let session = Session::new(source);
    let handle = session.handle();
    let runner: Runner = Arc::new(|line: &str| {
        if line.trim() == "known" {
            Claimed::Done
        } else {
            Claimed::Unknown
        }
    });
    assert!(handle.set_desk(Desk::new(None, runner)));
    let observer = session.observer();
    tokio::spawn(session.into_actor().run());
    let (_, mut numbered) = observer.subscribe().await.unwrap();
    let door = handle.script_door();

    let Sending::Sent { cursor } = door.send("look").await else {
        panic!("not sent");
    };
    let found = tokio::time::timeout(DEADLINE, async {
        loop {
            let event = numbered.recv().await.unwrap();
            if let Event::Sent { line, origin, .. } = event.event {
                return (event.cursor, line, origin);
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(found, (cursor, "look".to_owned(), Origin::Script));

    assert_eq!(door.send(";known").await, Sending::Ran);
    assert_eq!(door.send(";nosuch").await, Sending::Unknown);
    assert_eq!(
        transcript.lines(),
        ["look"],
        "only the game's line was sent"
    );
}

/// A built-in a script started is the script's, not the agent's: the
/// agent's operations do not list it, and its progress goes nowhere.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_scripts_built_in_is_not_the_agents_operation() {
    use cena_session::operation::{Ended, Performer, Progress, Started, Work};
    let (source, _transcript) = AnsweringSource::logged_in(b"<prompt time=\"1\">&gt;</prompt>\n");
    let session = Session::new(source);
    let handle = session.handle();
    let (_, mut events) = session.subscribe();
    tokio::spawn(session.into_actor().run());
    let performer = Performer {
        allowed: "go2".to_owned(),
        allows: Arc::new(|line: &str| Ok(line.to_owned())),
        start: Arc::new(|_line: &str, reporter| {
            reporter.progress(Progress::default());
            Started {
                ended: Box::pin(std::future::ready(Ended::plainly(
                    Work::Completed,
                    "arrived",
                ))),
                steer: Arc::new(|_| Ok(())),
                token: None,
            }
        }),
        halt: Arc::new(|| {}),
    };
    assert!(handle.set_performer(performer));

    let run = handle.script_door().perform("go2 bank").unwrap();
    assert_eq!(run.line, "go2 bank");
    let ended = run.ended.await;
    assert_eq!(ended.reason, "arrived");
    assert!(handle.agent_door().operations().is_empty());
    assert!(
        !std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, Event::Agent(_))),
        "nothing said on the agent's side"
    );
}
