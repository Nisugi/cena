use super::*;
use cena_session::{GameState, SessionId};
use std::collections::VecDeque;
use std::sync::{Mutex, PoisonError};

use cena_ui::{LifecycleView, SessionCard};

fn snapshot(cursor: u64, lifecycle: State, room: Option<&str>) -> Snapshot {
    let mut state = GameState::default();
    state.room.title = room.map(str::to_owned);
    Snapshot {
        session: SessionId::FIRST,
        generation: Generation::FIRST,
        cursor,
        state,
        lifecycle,
        retry: None,
        stopped: None,
        triggers: Arc::default(),
    }
}

fn observed(cursor: u64, event: Event) -> ObservedEvent {
    ObservedEvent {
        session: SessionId::FIRST,
        generation: Generation::FIRST,
        cursor,
        event,
    }
}

/// A session that answers each subscribe with the next scripted answer, and
/// `Closed` once they run out; every subscription hears `events`.
struct Script {
    answers: Mutex<VecDeque<Result<Snapshot, ObserveError>>>,
    events: broadcast::Sender<ObservedEvent>,
}

impl Script {
    fn new(answers: impl IntoIterator<Item = Result<Snapshot, ObserveError>>) -> Arc<Self> {
        Arc::new(Self {
            answers: Mutex::new(answers.into_iter().collect()),
            events: broadcast::channel(16).0,
        })
    }

    fn answer(&self) -> Result<Subscription, ObserveError> {
        let next = self
            .answers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front();
        match next {
            Some(Ok(snapshot)) => Ok((snapshot, self.events.subscribe())),
            Some(Err(error)) => Err(error),
            None => Err(ObserveError::Closed),
        }
    }
}

/// A feed for Ashryn over `script`, its merged lines into `merged`.
fn start_merging(
    script: &Arc<Script>,
    merged: &Arc<Mutex<MergedHistory>>,
) -> (Arc<Seat>, tokio::task::JoinHandle<()>) {
    let seat = Arc::new(Seat::new(
        handle(),
        "Ashryn",
        cena_session::DEFAULT_GAME_CODE,
    ));
    let answering = Arc::clone(script);
    let ears = Ears {
        seat: Arc::clone(&seat),
        merged: Arc::clone(merged),
    };
    let task = tokio::spawn(async move {
        follow(
            move || {
                let script = Arc::clone(&answering);
                async move { script.answer() }
            },
            &ears,
            Wake::default(),
        )
        .await;
    });
    (seat, task)
}

/// A handle whose session is not there: the feed never sends.
fn handle() -> cena_session::SessionHandle {
    cena_session::SessionHandle::new(
        tokio::sync::mpsc::channel(1).0,
        cena_session::GenerationCell::default(),
        broadcast::channel(1).0,
    )
}

fn start(script: &Arc<Script>) -> (Arc<Seat>, tokio::task::JoinHandle<()>) {
    start_merging(script, &Arc::default())
}

fn said(stream: &str, text: &str) -> Event {
    Event::Line(Arc::new(cena_session::Line::new(
        stream,
        cena_session::ChunkLine::plain(text).runs,
    )))
}

fn merged_text(merged: &Mutex<MergedHistory>) -> Vec<(String, Vec<String>)> {
    lock(merged)
        .lines()
        .map(|line| {
            let text = line.runs.iter().map(|run| run.text.as_str()).collect();
            (text, line.from.clone())
        })
        .collect()
}

fn card(seat: &Seat) -> SessionCard {
    seat.card
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

async fn settle() {
    tokio::time::sleep(Duration::from_secs(1)).await;
}

#[tokio::test(start_paused = true)]
async fn the_card_follows_its_session() {
    let script = Script::new([
        Ok(snapshot(0, State::Connecting, None)),
        Ok(snapshot(1, State::Ready, Some("Rawknuckle's"))),
    ]);
    let (seat, _task) = start(&script);
    settle().await;
    assert_eq!(card(&seat).lifecycle, LifecycleView::Connecting);
    assert_eq!(card(&seat).name, "Ashryn");

    // A change of connection is asked after at once, not at the next tick.
    let _ = script
        .events
        .send(observed(1, Event::StateChanged(State::Ready)));
    tokio::time::sleep(Duration::from_millis(50)).await;
    let now = card(&seat);
    assert_eq!(now.lifecycle, LifecycleView::Ready);
    assert_eq!(now.room.as_deref(), Some("Rawknuckle's"));
    assert_eq!(now.game, seat.game, "its game kept: the launcher reads it");
}

/// A busy owner is asked again; an owner gone ends the feed, and the card
/// keeps what was last seen rather than going blank.
#[tokio::test(start_paused = true)]
async fn a_busy_owner_is_asked_again_and_a_gone_one_is_remembered() {
    let script = Script::new([
        Err(ObserveError::Busy),
        Err(ObserveError::Timeout),
        Ok(snapshot(0, State::Ready, Some("Rawknuckle's"))),
    ]);
    let (seat, task) = start(&script);
    settle().await;
    assert_eq!(card(&seat).lifecycle, LifecycleView::Ready);

    // The next ask finds the owner gone.
    let _ = script.events.send(observed(1, Event::Quiet(true)));
    settle().await;
    assert!(task.is_finished(), "an owner gone ends the feed");
    assert_eq!(card(&seat).room.as_deref(), Some("Rawknuckle's"));
}

#[tokio::test(start_paused = true)]
async fn a_detached_seat_stops_following() {
    let script = Script::new([Ok(snapshot(0, State::Ready, None))]);
    let (seat, task) = start(&script);
    settle().await;
    assert!(!task.is_finished());
    seat.stop.cancel();
    settle().await;
    assert!(task.is_finished());
}

/// Roundtime counts down on the card with no event to prompt it: the feed
/// asks again every 100 ms while one runs.
#[tokio::test(start_paused = true)]
async fn a_roundtime_is_asked_after_without_an_event() {
    let mut first = snapshot(0, State::Ready, None);
    first.state.apply(&cena_session::Frame::Prompt {
        time: "1000".to_owned(),
        text: ">".to_owned(),
    });
    first.state.roundtime_ends = Some(1_003);
    let script = Script::new([
        Ok(first),
        Ok(snapshot(0, State::Ready, Some("after the tick"))),
    ]);
    let (seat, _task) = start(&script);
    settle().await;
    assert_eq!(card(&seat).room.as_deref(), Some("after the tick"));
}

/// A thought reaches the hub's merged streams, tagged with who heard it; a
/// line on the main stream does not.
#[tokio::test(start_paused = true)]
async fn a_thought_is_merged_and_the_story_is_not() {
    let script = Script::new([
        Ok(snapshot(0, State::Ready, None)),
        Ok(snapshot(2, State::Ready, None)),
    ]);
    let merged = Arc::default();
    let (_seat, _task) = start_merging(&script, &merged);
    settle().await;
    let _ = script
        .events
        .send(observed(1, said("thoughts", "[General] hello")));
    let _ = script.events.send(observed(2, said("", "You see a rock.")));
    settle().await;
    assert_eq!(
        merged_text(&merged),
        [("[General] hello".to_owned(), vec!["Ashryn".to_owned()])]
    );
}

/// A line published before a fresh snapshot, and still on the old receiver
/// when the feed asks again, is heard once: not lost at the switch, and not
/// heard twice from both receivers.
#[tokio::test(start_paused = true)]
async fn a_line_at_the_switch_is_heard_once() {
    let script = Script::new([
        Ok(snapshot(0, State::Ready, None)),
        Ok(snapshot(2, State::Ready, None)),
    ]);
    let merged = Arc::default();
    let (_seat, _task) = start_merging(&script, &merged);
    settle().await;
    // Both before the feed wakes: it reads the first, asks again, and finds
    // the second still waiting on the old receiver.
    let _ = script
        .events
        .send(observed(1, Event::StateChanged(State::Ready)));
    let _ = script
        .events
        .send(observed(2, said("thoughts", "[General] once")));
    settle().await;
    assert_eq!(
        merged_text(&merged),
        [("[General] once".to_owned(), vec!["Ashryn".to_owned()])]
    );
}

/// A session that stopped by itself says why on its card.
#[test]
fn a_stopped_session_says_why() {
    let mut closed = snapshot(0, State::Closed, None);
    closed.stopped = Some("not logged in: [auth] bad password".to_owned());
    assert_eq!(
        lifecycle(&closed),
        LifecycleView::Closed {
            detail: Some("not logged in: [auth] bad password".to_owned())
        }
    );
}

/// A feed that fell behind the session marks the hole in the story rather
/// than joining the lines on either side as if nothing were missing.
#[tokio::test(start_paused = true)]
async fn a_feed_that_fell_behind_marks_the_hole() {
    let script = Script::new([
        Ok(snapshot(0, State::Ready, None)),
        Ok(snapshot(40, State::Ready, None)),
    ]);
    let (seat, _task) = start(&script);
    settle().await;
    // More than the channel holds, before the feed can read one.
    for cursor in 1..=40 {
        let _ = script
            .events
            .send(observed(cursor, said("", &format!("line {cursor}"))));
    }
    settle().await;
    assert!(
        lock(&seat.story)
            .lines
            .iter()
            .any(|(_, shown)| *shown == crate::story::Shown::Gap)
    );
}

#[test]
fn a_retry_says_its_attempt_and_delay() {
    let mut retrying = snapshot(0, State::Reconnecting, None);
    retrying.retry = Some(cena_session::RetryStatus {
        attempt: 2,
        delay: Duration::from_secs(2),
        detail: "timed out".to_owned(),
    });
    assert_eq!(
        lifecycle(&retrying),
        LifecycleView::Reconnecting {
            attempt: Some(2),
            retry_delay_ms: Some(2_000),
            detail: Some("timed out".to_owned()),
        }
    );
}

/// The live `R>` is settled when a later snapshot says roundtime has run
/// out, with no event to say so: the game sends no prompt for its end
/// (`plan/15` §2a.1).
#[tokio::test(start_paused = true)]
async fn the_live_prompt_settles_as_roundtime_ends() {
    let ending = |cursor, ends| {
        let mut shot = snapshot(cursor, State::Ready, None);
        shot.state.apply(&cena_session::Frame::Prompt {
            time: "1000".to_owned(),
            text: "R>".to_owned(),
        });
        shot.state.roundtime_ends = Some(ends);
        shot
    };
    // Out of roundtime, so nothing is asked for until the events; then in
    // it, asked after each tick; then out again.
    let script = Script::new([
        Ok(ending(0, 1_000)),
        Ok(ending(3, 1_003)),
        Ok(ending(3, 1_000)),
    ]);
    let (seat, _task) = start(&script);
    settle().await;
    let _ = script.events.send(observed(1, said("", "You swing.")));
    let prompt = cena_session::Frame::Prompt {
        time: "1000".to_owned(),
        text: "R>".to_owned(),
    };
    let _ = script
        .events
        .send(observed(2, Event::Frame(Box::new(prompt))));
    let _ = script
        .events
        .send(observed(3, Event::Prompt("R>".to_owned())));
    settle().await;
    let last = lock(&seat.story)
        .lines
        .back()
        .map(|(_, shown)| shown.clone());
    assert_eq!(last, Some(crate::story::Shown::Prompt(">".to_owned())));
}
