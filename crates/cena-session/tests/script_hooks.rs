//! A script runner's hooks (`plan/46` §6.1, M7b step 4; the author: *"sure
//! ask the script, with a time limit"*): what a viewer is shown of a line
//! waits for the runner's display hooks to answer it, and goes as it came
//! past the deadline; what the player types is asked of its input hooks
//! first. Nothing but the showing and the typing waits on a script.

use std::sync::Arc;
use std::time::Duration;

use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::script::{Asker, Door, HOOK_DEADLINE};
use cena_session::trigger::{Color, Flag, Look, Matcher, Pattern, Rule, Trigger};
use cena_session::{Event, Frame, Line, ObservedEvent, Outcome, Session, SessionHandle};
use tokio::sync::{broadcast, oneshot};

const DEADLINE: Duration = Duration::from_secs(5);

/// What `look` draws: three lines, the first with a link, then its prompt.
const LOOK: &str = concat!(
    "A <a exist=\"7\" noun=\"lizard\">cave lizard</a> scurries past.\n",
    "Moss covers the ground.\n",
    "A breeze stirs the leaves.\n",
    "<prompt time=\"7\">&gt;</prompt>\n",
);

struct Hooked {
    handle: SessionHandle,
    door: Door,
    numbered: broadcast::Receiver<ObservedEvent>,
    transcript: TranscriptHandle,
}

/// A live session answering `look` with [`LOOK`], a runner listening with
/// display hooks, and the character's `triggers`.
async fn hooked(triggers: Vec<Trigger>) -> Option<Hooked> {
    let (source, transcript) = AnsweringSource::logged_in(b"<prompt time=\"1\">&gt;</prompt>\n");
    transcript.answer("look", LOOK.as_bytes());
    let session = Session::new(source);
    let handle = session.handle();
    handle.set_triggers(Matcher::new(triggers).ok()?);
    let door = handle.script_door();
    door.listen(true);
    door.hook_lines(true);
    let observer = session.observer();
    tokio::spawn(session.into_actor().run());
    let (_, numbered) = observer.subscribe().await.ok()?;
    Some(Hooked {
        handle,
        door,
        numbered,
        transcript,
    })
}

/// Everything published up to `look`'s prompt.
async fn looked(hooked: &mut Hooked) -> Vec<ObservedEvent> {
    let _ = hooked.door.send("look").await;
    let mut events = Vec::new();
    tokio::time::timeout(DEADLINE, async {
        loop {
            let Ok(event) = hooked.numbered.recv().await else {
                return;
            };
            let prompt = matches!(&event.event, Event::Frame(frame)
                if matches!(frame.as_ref(), Frame::Prompt { time, .. } if time == "7"));
            events.push(event);
            if prompt {
                return;
            }
        }
    })
    .await
    .ok();
    events
}

/// The lines heard in `events`, at their cursors.
fn heard(events: &[ObservedEvent]) -> Vec<(u64, String)> {
    events
        .iter()
        .filter_map(|event| match &event.event {
            Event::Heard(line) => Some((event.cursor, line.text())),
            _ => None,
        })
        .collect()
}

/// The lines shown in `events`.
fn shown(events: &[ObservedEvent]) -> Vec<Arc<Line>> {
    events
        .iter()
        .filter_map(|event| match &event.event {
            Event::Line(line) => Some(Arc::clone(line)),
            _ => None,
        })
        .collect()
}

/// The next `count` lines shown.
async fn next_shown(
    numbered: &mut broadcast::Receiver<ObservedEvent>,
    count: usize,
) -> Vec<Arc<Line>> {
    let mut lines = Vec::new();
    tokio::time::timeout(DEADLINE, async {
        while lines.len() < count {
            let Ok(event) = numbered.recv().await else {
                return;
            };
            if let Event::Line(line) = event.event {
                lines.push(line);
            }
        }
    })
    .await
    .ok();
    lines
}

/// Whatever is waiting, now.
fn drained(numbered: &mut broadcast::Receiver<ObservedEvent>) -> Vec<ObservedEvent> {
    std::iter::from_fn(|| numbered.try_recv().ok()).collect()
}

fn texts(lines: &[Arc<Line>]) -> Vec<String> {
    lines.iter().map(|line| line.text()).collect()
}

/// Kept as it came, link and all; hidden; changed.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_hooked_line_is_shown_as_the_hooks_answer_it() {
    let mut hooked = hooked(Vec::new()).await.unwrap();
    let events = looked(&mut hooked).await;
    let heard = heard(&events);
    assert_eq!(heard.len(), 3, "{events:?}");
    assert!(
        shown(&events).is_empty(),
        "nothing shown before the hooks answer"
    );

    let answered = tokio::time::Instant::now();
    hooked.door.shown([
        (heard[0].0, Some(heard[0].1.clone())),
        (heard[1].0, None),
        (heard[2].0, Some("A wind howls.".to_owned())),
    ]);
    let lines = next_shown(&mut hooked.numbered, 2).await;
    assert_eq!(
        texts(&lines),
        ["A cave lizard scurries past.", "A wind howls."]
    );
    assert!(
        answered.elapsed() < HOOK_DEADLINE,
        "shown when answered, not at the deadline"
    );
    assert!(
        lines[0].runs.objects().next().is_some(),
        "a kept line is the game's, its link and all"
    );
}

/// Unanswered, a line goes as it came at the deadline, and one answered
/// behind it waits for it: lines come in the game's order.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_line_the_hooks_do_not_answer_in_time_goes_as_it_came() {
    let mut hooked = hooked(Vec::new()).await.unwrap();
    let heard = heard(&looked(&mut hooked).await);
    hooked
        .door
        .shown([(heard[2].0, Some("A wind howls.".to_owned()))]);
    tokio::time::sleep(HOOK_DEADLINE.saturating_sub(Duration::from_millis(1))).await;
    assert!(shown(&drained(&mut hooked.numbered)).is_empty());

    let lines = next_shown(&mut hooked.numbered, 3).await;
    assert_eq!(
        texts(&lines),
        [
            "A cave lizard scurries past.",
            "Moss covers the ground.",
            "A wind howls."
        ]
    );
}

/// When the runner's display hooks go, what they held is shown at once.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_hooks_going_shows_what_they_held() {
    let mut hooked = hooked(Vec::new()).await.unwrap();
    let _ = looked(&mut hooked).await;
    let before = tokio::time::Instant::now();
    hooked.door.hook_lines(false);
    let lines = next_shown(&mut hooked.numbered, 3).await;
    assert_eq!(lines.len(), 3);
    assert!(before.elapsed() < HOOK_DEADLINE, "not at the deadline");
    let _ = &hooked.transcript;
}

/// The triggers' flags go when the line comes, not when it is shown; a
/// changed line is answered by the triggers again, painted or squelched;
/// and a line the triggers squelched stays squelched whatever the hooks
/// make of it.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn only_the_showing_waits_and_the_triggers_answer_a_changed_line() {
    let trigger = |name: &str, text: &str, rule: Rule| Trigger {
        name: name.into(),
        rule: Rule {
            pattern: Some(Pattern::Literal {
                text: text.into(),
                whole_word: true,
            }),
            ..rule
        },
    };
    let mut hooked = hooked(vec![
        trigger(
            "seen",
            "lizard",
            Rule {
                flag: Some(Flag {
                    name: "lizard".into(),
                    seconds: None,
                    clear: false,
                }),
                ..Rule::default()
            },
        ),
        trigger(
            "howl",
            "howls",
            Rule {
                look: Some(Look {
                    color: Some(Color {
                        red: 255,
                        green: 0,
                        blue: 0,
                    }),
                    background: None,
                    bold: false,
                    span: cena_session::trigger::Span::Match,
                }),
                ..Rule::default()
            },
        ),
        trigger(
            "hush",
            "breeze",
            Rule {
                squelch: true,
                ..Rule::default()
            },
        ),
        trigger(
            "wind",
            "gale",
            Rule {
                squelch: true,
                ..Rule::default()
            },
        ),
    ])
    .await
    .unwrap();
    let events = looked(&mut hooked).await;
    assert!(
        events
            .iter()
            .any(|event| matches!(&event.event, Event::Flag(_))),
        "the flag is set while the line waits"
    );
    let heard = heard(&events);
    hooked.door.shown([
        (heard[0].0, Some("A gale rises.".to_owned())),
        (heard[1].0, Some("A wind howls.".to_owned())),
        (heard[2].0, Some("Leaves fall.".to_owned())),
    ]);
    let lines = next_shown(&mut hooked.numbered, 1).await;
    assert_eq!(texts(&lines), ["A wind howls."]);
    assert!(!lines[0].paint.is_empty(), "painted by the triggers");
    tokio::time::sleep(HOOK_DEADLINE * 2).await;
    assert!(shown(&drained(&mut hooked.numbered)).is_empty());
}

/// What the player types is asked of the input hooks first: swallowed,
/// changed, or as typed when they do not answer in time. Hydra's own lines
/// on the manual path never meet them.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_players_typing_is_asked_of_the_input_hooks() {
    let hooked = hooked(Vec::new()).await.unwrap();
    let questions = Arc::new(std::sync::Mutex::new((Vec::new(), Vec::new())));
    let seen = Arc::clone(&questions);
    let asker: Asker = Arc::new(move |line: &str| {
        let mut seen = seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        seen.0.push(line.to_owned());
        let (answer, answered) = oneshot::channel();
        match line {
            "secret" => {
                let _ = answer.send(None);
            }
            "tt" => {
                let _ = answer.send(Some("target".to_owned()));
            }
            // Not answered: kept, so the question stays open.
            _ => seen.1.push(answer),
        }
        answered
    });
    hooked.door.hook_typing(Some(asker));
    let generation = hooked.handle.generation();

    let typed = |line: &'static str| hooked.handle.send_typed_at(generation, line, DEADLINE);
    assert_eq!(typed("secret").await, Outcome::Handled);
    let _ = typed("tt").await;
    let before = tokio::time::Instant::now();
    let _ = typed("stance defensive").await;
    assert!(before.elapsed() >= HOOK_DEADLINE, "waited for the hooks");
    let _ = hooked
        .handle
        .send_manual_at(generation, "hydras own", DEADLINE)
        .await;

    assert_eq!(
        hooked.transcript.lines(),
        ["target", "stance defensive", "hydras own"]
    );
    assert_eq!(
        questions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .0,
        ["secret", "tt", "stance defensive"]
    );
}
