//! `plan/45` §4a: the session publishes each finished line, once, for every
//! viewer.
//!
//! What a viewer draws must be the line the classifiers and the player log
//! read, or a trigger's colour has nothing to land on. So these drive real
//! bytes through a real `SessionActor` and compare what is published with
//! what the model made and the log wrote.

use cena_platform::ReplaySource;
use cena_session::player_log::{Capture, LogSink};
use cena_session::trigger::{Look, Matcher, Pattern, Rule, Span, Trigger};
use cena_session::{Event, Frame, Line, PlayerLog, Runs, Session};
use std::sync::Arc;

/// Every event the session published over `wire`, in order.
async fn published(wire: &str) -> Vec<Event> {
    let session = Session::new(ReplaySource::from_bytes(wire.as_bytes()));
    let (_, mut events) = session.subscribe();
    let _ = Box::pin(session.into_actor().run()).await;
    std::iter::from_fn(|| events.try_recv().ok()).collect()
}

fn lines(events: &[Event]) -> Vec<Arc<Line>> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Line(line) => Some(Arc::clone(line)),
            _ => None,
        })
        .collect()
}

fn texts(events: &[Event]) -> Vec<(String, String)> {
    lines(events)
        .iter()
        .map(|line| (line.stream.clone(), line.text()))
        .collect()
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_line_split_at_a_link_is_published_once_with_its_link() {
    let events = published("You see <a exist=\"1\" noun=\"rock\">a rock</a> here.\n").await;
    let lines = lines(&events);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0].text(), "You see a rock here.");
    // The link survives: it is what a click, and a trigger, name.
    assert_eq!(lines[0].runs.links().count(), 1);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn each_line_follows_the_frame_that_finished_it() {
    let events = published("one\ntwo <b>bold</b> end\n").await;
    let mut seen = 0;
    for (index, event) in events.iter().enumerate() {
        if let Event::Line(line) = event {
            seen += 1;
            let Some(Event::Frame(frame)) = index.checked_sub(1).map(|i| &events[i]) else {
                panic!("{:?} did not follow a frame", line.text());
            };
            let Frame::Text(text) = frame.as_ref() else {
                panic!("{:?} followed {frame:?}", line.text());
            };
            assert!(
                text.ends_line,
                "{:?} followed a mid-line frame",
                line.text()
            );
        }
    }
    assert_eq!(seen, 2);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn an_interrupted_line_resumes_and_each_stream_keeps_its_own() {
    // A push interrupts the main line; the thought finishes first, then main
    // resumes on the next wire line and finishes as ONE line.
    let wire = concat!(
        "Story <pushStream id='thoughts'/>[OOC] someone: hi\n<popStream/>",
        "resumes\n",
    );
    assert_eq!(
        texts(&published(wire).await),
        vec![
            ("thoughts".to_owned(), "[OOC] someone: hi".to_owned()),
            (String::new(), "Story resumes".to_owned()),
        ]
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_frame_that_finishes_no_line_publishes_none() {
    let events = published("<prompt time=\"1\">&gt;</prompt>\n").await;
    assert!(lines(&events).is_empty(), "{:?}", lines(&events));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_viewers_line_is_the_one_the_player_log_writes() {
    let wire = concat!(
        "before\n",
        "You see <a exist=\"1\" noun=\"rock\">a rock</a> here.\n",
        "<pushStream id='thoughts'/>[OOC] someone: hi\n<popStream/>",
        "after\n",
    );
    let (log, mut sink) = PlayerLog::new();
    let session = Session::new(ReplaySource::from_bytes(wire.as_bytes())).with_player_log(
        log,
        Capture::default(),
        None,
    );
    let (_, mut events) = session.subscribe();
    let _ = Box::pin(session.into_actor().run()).await;
    let events: Vec<Event> = std::iter::from_fn(|| events.try_recv().ok()).collect();
    let logged = drain(&mut sink);

    let shown: Vec<String> = lines(&events).iter().map(|line| line.text()).collect();
    assert_eq!(
        shown,
        [
            "before",
            "You see a rock here.",
            "[OOC] someone: hi",
            "after"
        ]
    );
    // Every line the log wrote was published, with the same text.
    for text in &logged {
        assert!(
            shown.contains(text),
            "{text:?} was logged but not published"
        );
    }
    assert!(
        !logged.is_empty(),
        "the log wrote nothing, so this proved nothing"
    );
}

/// Five container looks from the author's logs; their provenance is in
/// `crates/cena-model/tests/sorter.rs`.
const LOOKS: &str = include_str!("../../cena-model/tests/fixtures/container_looks.xml");

/// The mahogany box, as the game sends it.
fn the_box() -> String {
    format!("{}\n", LOOKS.lines().next().unwrap_or_default())
}

/// The lines published over `wire`, and what the model kept on `stream`.
async fn run(wire: &str, sorting: bool, stream: &str) -> (Vec<String>, Option<String>) {
    let session = Session::new(ReplaySource::from_bytes(wire.as_bytes()));
    session.handle().sort_containers(sorting);
    let (_, mut events) = session.subscribe();
    let end = Box::pin(session.into_actor().run()).await;
    let events: Vec<Event> = std::iter::from_fn(|| events.try_recv().ok()).collect();
    let shown = lines(&events).iter().map(|line| line.text()).collect();
    let kept = end
        .state
        .stream(stream)
        .last()
        .map(cena_session::Runs::plain);
    (shown, kept)
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn with_sorter_on_a_look_is_published_sorted_and_the_model_keeps_it_whole() {
    // Sorted in the session, before any viewer, so M8's triggers will match
    // each sorted line (`plan/45` section 4a). The record stays the game's.
    let whole = "In the mahogany box you see a bright gold ingot, some silver coins, \
                 a smooth amber wand, a piece of brown jade, a steel lockpick, \
                 a pinch of electrum dust and a tar black tourmaline.";
    let (shown, kept) = run(&the_box(), true, "").await;
    assert_eq!(
        shown,
        [
            "In the mahogany box:",
            "  valuable (1): bright gold ingot",
            "  other (1): some silver coins",
            "  wand (1): smooth amber wand",
            "  gem (3): pinch of electrum dust, piece of brown jade, tar black tourmaline",
            "  lockpick (1): steel lockpick",
        ]
    );
    assert_eq!(kept.as_deref(), Some(whole));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn off_or_off_the_main_stream_a_look_is_published_as_it_came() {
    let (off, _) = run(&the_box(), false, "").await;
    assert_eq!(off.len(), 1, "{off:?}");
    assert!(off[0].starts_with("In the mahogany box you see"), "{off:?}");

    // The same look on another stream is not a look at a container.
    let pushed = format!("<pushStream id='thoughts'/>{}<popStream/>", the_box());
    let (elsewhere, _) = run(&pushed, true, "thoughts").await;
    assert_eq!(elsewhere.len(), 1, "{elsewhere:?}");
}

/// A trigger on these words that does what `edit` says.
fn trigger(name: &str, text: &str, edit: impl FnOnce(&mut Rule)) -> Trigger {
    let mut rule = Rule {
        pattern: Some(Pattern::Literal {
            text: text.into(),
            whole_word: true,
        }),
        ..Rule::default()
    };
    edit(&mut rule);
    Trigger {
        name: name.into(),
        rule,
    }
}

/// `combat_wiring.rs`'s swing: one attack on a cave lizard, 40 damage.
const SWING: &str = concat!(
    "You swing a broadsword at <pushBold/><a exist=\"101\" noun=\"lizard\">a cave lizard</a><popBold/>!\n",
    "  AS: +300 vs DS: +100 with AvD: +30 + d100 roll: +50 = +280\n",
    "   ... and hit for 40 points of damage!\n",
    "<prompt time=\"1000\">&gt;</prompt>\n",
);

/// **A trigger changes what a viewer is given, and nothing else.** The swing
/// is squelched and its damage rewritten for the viewer; the model's
/// scrollback, the player log, the combat event and the creature's damage
/// all keep the game's text (`plan/45` §4).
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn triggers_answer_the_published_line_and_the_record_keeps_the_games_text() {
    let (log, mut sink) = PlayerLog::new();
    let session = Session::new(ReplaySource::from_bytes(SWING.as_bytes())).with_player_log(
        log,
        Capture::default(),
        None,
    );
    session.handle().set_triggers(
        Matcher::new(vec![
            trigger("hide-swing", "You swing", |r| r.squelch = true),
            trigger("damage", "40 points", |r| {
                r.substitute = Some("LOTS".into());
            }),
        ])
        .unwrap(),
    );
    let (_, mut events) = session.subscribe();
    let end = Box::pin(session.into_actor().run()).await;
    let events: Vec<Event> = std::iter::from_fn(|| events.try_recv().ok()).collect();

    let shown: Vec<String> = lines(&events).iter().map(|line| line.text()).collect();
    assert!(
        !shown.iter().any(|line| line.starts_with("You swing")),
        "squelched: {shown:?}"
    );
    assert!(
        shown
            .iter()
            .any(|line| line == "   ... and hit for LOTS of damage!"),
        "substituted: {shown:?}"
    );

    let kept: Vec<String> = end.state.stream("").iter().map(Runs::plain).collect();
    assert!(
        kept.iter()
            .any(|line| line.starts_with("You swing a broadsword")),
        "{kept:?}"
    );
    assert!(
        kept.iter().any(|line| line.contains("40 points")),
        "{kept:?}"
    );
    let logged = drain(&mut sink);
    assert!(
        logged
            .iter()
            .any(|line| line.starts_with("You swing a broadsword")),
        "{logged:?}"
    );
    assert!(
        logged.iter().any(|line| line.contains("40 points")),
        "{logged:?}"
    );

    let damage: Vec<u32> = events
        .iter()
        .filter_map(|event| match event {
            Event::Combat(facts) => Some(
                facts
                    .events
                    .iter()
                    .map(cena_model::state::combat::AttackEvent::total_damage)
                    .sum(),
            ),
            _ => None,
        })
        .collect();
    assert_eq!(damage, [40], "the chunk the classifiers read is the game's");
    assert_eq!(
        end.state
            .creatures()
            .get(101)
            .map(cena_model::CreatureInstance::damage_taken),
        Some(40)
    );
}

/// With `;sorter` on, the triggers see each sorted line, as `VellumFE` sorts
/// before it highlights: a look on `gem` paints the gem line and no other.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn with_sorter_on_triggers_match_each_sorted_line() {
    let session = Session::new(ReplaySource::from_bytes(the_box().as_bytes()));
    session.handle().sort_containers(true);
    session.handle().set_triggers(
        Matcher::new(vec![trigger("gems", "gem", |r| {
            r.look = Some(Look {
                color: None,
                background: None,
                bold: true,
                span: Span::Line,
            });
        })])
        .unwrap(),
    );
    let (_, mut events) = session.subscribe();
    let _ = Box::pin(session.into_actor().run()).await;
    let events: Vec<Event> = std::iter::from_fn(|| events.try_recv().ok()).collect();
    let painted: Vec<(String, bool)> = lines(&events)
        .iter()
        .map(|line| (line.text(), !line.paint.is_empty()))
        .collect();
    assert_eq!(painted.len(), 6, "{painted:?}");
    for (text, is_painted) in &painted {
        assert_eq!(*is_painted, text.starts_with("  gem (3)"), "{text:?}");
    }
}

fn drain(sink: &mut LogSink) -> Vec<String> {
    std::iter::from_fn(|| sink.try_recv())
        .map(|line| line.text)
        .collect()
}
