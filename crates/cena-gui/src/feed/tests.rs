use super::*;
use cena_session::{GameState, SessionId};
use std::collections::VecDeque;
use std::sync::Mutex;

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

fn start(script: &Arc<Script>) -> (Arc<Seat>, tokio::task::JoinHandle<()>) {
    let seat = Arc::new(Seat::new(SessionId::FIRST, "Ashryn"));
    let answering = Arc::clone(script);
    let task = tokio::spawn(follow(
        move || {
            let script = Arc::clone(&answering);
            async move { script.answer() }
        },
        Arc::clone(&seat),
        Wake::default(),
    ));
    (seat, task)
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
