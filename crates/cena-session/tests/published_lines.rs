//! `plan/45` §4a: the session publishes each finished line, once, for every
//! viewer.
//!
//! What a viewer draws must be the line the classifiers and the player log
//! read, or a trigger's colour has nothing to land on. So these drive real
//! bytes through a real `SessionActor` and compare what is published with
//! what the model made and the log wrote.

use cena_platform::ReplaySource;
use cena_session::player_log::{Capture, LogSink};
use cena_session::{Event, Frame, Line, PlayerLog, Session};
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

fn drain(sink: &mut LogSink) -> Vec<String> {
    std::iter::from_fn(|| sink.try_recv())
        .map(|line| line.text)
        .collect()
}
