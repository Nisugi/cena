//! Detached read access. Requests are answered by the current state owner in
//! one synchronous turn, so a snapshot and its stream share an exact fence.

use crate::{Event, GameState, Generation, GenerationCell, SessionId, Snapshot, State};
use cena_model::trigger::{Act, Attention, Cooldowns, Edges, Matcher, Pace};
use std::sync::{
    Arc, Mutex, PoisonError,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, oneshot, watch};

/// One occurrence, located in a session's ordered observation stream.
#[derive(Clone, Debug, PartialEq)]
pub struct ObservedEvent {
    /// The session that published this occurrence.
    pub session: SessionId,
    /// The connection that published this occurrence.
    pub generation: Generation,
    /// Strictly increasing across reconnects, starting at one.
    pub cursor: u64,
    /// Native typed occurrence; frontends map this into their view vocabulary.
    pub event: Event,
}

/// Most recent retry decision, retained for subscribers joining during backoff.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetryStatus {
    /// Attempt number, starting at one.
    pub attempt: u32,
    /// The scheduled delay, not a remaining-time countdown.
    pub delay: Duration,
    /// The connector's already-redacted reason for retrying.
    pub detail: String,
}

/// Why a fresh observation could not be obtained.
/// `Busy` and `Timeout` are retryable read failures, not proof the owner died.
/// Retry with bounded backoff; never spin or substitute a stale snapshot.
/// `Closed` is terminal for this observer: reacquire one from a new owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObserveError {
    /// The bounded observation inbox is full; retry after yielding/backoff.
    Busy,
    /// The owner vanished without completing shutdown.
    Closed,
    /// The owner did not answer within the request budget; retry with backoff.
    Timeout,
}

type Subscription = (Snapshot, broadcast::Receiver<ObservedEvent>);
type Request = oneshot::Sender<Subscription>;

// A caller wait budget, not a measured game/network deadline or failure detector.
// The owner answers in a synchronous select-loop arm (no game round trip), so
// five seconds deliberately tolerates scheduling/load while bounding a hung
// owner's effect on a viewer. Expiry abandons only this read; it cannot stop the
// owner or prove death. The paused-clock unresponsive-owner test pins this policy.
const SUBSCRIBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Cloneable read-only access that survives consuming the session in `run`.
/// It has no command sender and cannot mutate the character.
#[derive(Clone, Debug)]
pub struct SessionObserver {
    requests: mpsc::Sender<Request>,
    final_snapshot: watch::Receiver<Option<Snapshot>>,
}

impl SessionObserver {
    /// Obtain a fresh snapshot and every event strictly after its cursor.
    /// On broadcast lag, discard the old receiver and call this again.
    ///
    /// # Errors
    /// Reports inbox saturation, an unresponsive owner, or an owner that
    /// disappeared without clean shutdown. After normal shutdown, returns
    /// the final `Closed` snapshot with an already closed event receiver.
    /// `Busy` and `Timeout` may be retried with bounded backoff; `Closed` cannot.
    pub async fn subscribe(&self) -> Result<Subscription, ObserveError> {
        let (reply, answer) = oneshot::channel();
        match self.requests.try_send(reply) {
            Ok(()) => match tokio::time::timeout(SUBSCRIBE_TIMEOUT, answer).await {
                Ok(Ok(subscription)) => Ok(subscription),
                Ok(Err(_)) => self.closed_snapshot(),
                Err(_) => Err(ObserveError::Timeout),
            },
            Err(mpsc::error::TrySendError::Full(_)) => Err(ObserveError::Busy),
            Err(mpsc::error::TrySendError::Closed(_)) => self.closed_snapshot(),
        }
    }

    fn closed_snapshot(&self) -> Result<Subscription, ObserveError> {
        let snapshot = self
            .final_snapshot
            .borrow()
            .clone()
            .ok_or(ObserveError::Closed)?;
        let (sender, events) = broadcast::channel(1);
        drop(sender);
        Ok((snapshot, events))
    }
}

/// Owned by the supervisor between connections, by the actor during one.
#[derive(Debug)]
pub(crate) struct ObservationRequests {
    pub(crate) requests: mpsc::Receiver<Request>,
    final_snapshot: watch::Sender<Option<Snapshot>>,
    observer: SessionObserver,
}

impl ObservationRequests {
    pub(crate) fn new() -> Self {
        let (requests, receiver) = mpsc::channel(32);
        let (final_snapshot, terminal) = watch::channel(None);
        Self {
            requests: receiver,
            final_snapshot,
            observer: SessionObserver {
                requests,
                final_snapshot: terminal,
            },
        }
    }

    pub(crate) fn observer(&self) -> SessionObserver {
        self.observer.clone()
    }

    pub(crate) fn finish(&mut self, snapshot: Snapshot) {
        self.final_snapshot.send_replace(Some(snapshot));
        self.requests.close();
        while self.requests.try_recv().is_ok() {}
    }
}

/// Both legacy and fenced streams share one publication point. Clones pass
/// ownership between the supervisor and actor, and the owner publishes
/// everything **except notices**, which a [`SessionHandle`](crate::SessionHandle)
/// publishes from whatever task calls `say`.
///
/// # Why there is a lock, and what it costs
///
/// Notices used to go to the legacy sender alone, so they never reached the
/// fenced stream a `SessionObserver` reads -- a frontend attached through
/// `observer().subscribe()` never saw "Travel: no route" (review finding 7).
/// Routing them through [`Self::send`] fixes that, but makes a second task a
/// publisher, and the fence is only exact while one task at a time can
/// publish or take a snapshot: a notice that took its cursor between another
/// task's snapshot and its `subscribe` would be missed by that subscriber, or
/// arrive numbered at or below the snapshot's cursor and be discarded as
/// already seen.
///
/// So publication and `answer` share one mutex. It is per session, so 25
/// sessions never contend with each other, and within one session the second
/// party is a notice -- a handful per session. Uncontended, it costs one
/// lock and unlock per event.
#[derive(Clone, Debug)]
pub(crate) struct EventPublisher {
    legacy: broadcast::Sender<Event>,
    observed: broadcast::Sender<ObservedEvent>,
    cursor: Arc<AtomicU64>,
    generation: GenerationCell,
    session: SessionId,
    retry: Arc<Mutex<Option<RetryStatus>>>,
    /// Held while a cursor is taken and its event sent, and while a snapshot
    /// is paired with a subscription. See the type's docs.
    fence: Arc<Mutex<()>>,
    /// `;sorter`: whether a container look is published sorted
    /// (`cena_model::sorter`). Here because this is what every viewer is
    /// given, and because every connection's actor shares this publisher, so
    /// the switch outlives a reconnect. Off until asked, `VellumFE`'s default.
    sorting: Arc<AtomicBool>,
    /// Whether each finished line is also published as the game sent it
    /// ([`Event::Heard`]), for a script runner. Here for `sorting`'s reasons.
    hearing: Arc<AtomicBool>,
    /// A script runner's hooks (`crate::script`): whether each line shown
    /// waits for its display hooks, their answers, and its input hooks. Here
    /// for `sorting`'s reasons.
    hooks: Arc<crate::script::Hooks>,
    /// This character's triggers, compiled (`plan/45`): what each finished
    /// line is answered with before it is published, and what its conditions
    /// last read. Here for `sorting`'s reasons, which is also why a reconnect
    /// keeps the conditions' memory. None until the binary reads the file.
    triggers: Arc<Mutex<Answering>>,
    /// The player's Lich, while one is attached (`crate::script::lich`):
    /// where the game's bytes are copied, and the player's typing handed.
    /// Here for `sorting`'s reasons: Lich stays up through a reconnect, and
    /// sees the new login as more of the stream.
    lich: Arc<Mutex<Option<crate::script::lich::Tap>>>,
}

/// A character's triggers and their memory, replaced together: new
/// triggers start new memory, so their conditions' first reading is silent
/// (`cena_model::trigger::Edges`) and their attention starts cool
/// (`Cooldowns`).
#[derive(Debug, Default)]
struct Answering {
    matcher: Arc<Matcher>,
    edges: Edges,
    cooldowns: Cooldowns,
    pace: Pace,
}

/// What the fired triggers may do beyond the line now: their attention and
/// sends past each trigger's cooldown, and the sends the pace held back.
pub(crate) struct Admitted {
    pub(crate) attention: Vec<Attention>,
    pub(crate) acts: Vec<Act>,
    pub(crate) held: Vec<Act>,
    /// The held sends are to be said: once a window.
    pub(crate) say_held: bool,
}

impl EventPublisher {
    pub(crate) fn new(bound: usize, generation: GenerationCell, session: SessionId) -> Self {
        Self {
            legacy: broadcast::channel(bound).0,
            observed: broadcast::channel(bound).0,
            cursor: Arc::new(AtomicU64::new(0)),
            generation,
            session,
            retry: Arc::new(Mutex::new(None)),
            fence: Arc::new(Mutex::new(())),
            sorting: Arc::new(AtomicBool::new(false)),
            hearing: Arc::new(AtomicBool::new(false)),
            hooks: Arc::default(),
            triggers: Arc::default(),
            lich: Arc::default(),
        }
    }

    /// Attach the player's Lich from now on. False, and `tap` unused, while
    /// another is still attached: one Lich per character.
    pub(crate) fn attach_lich(&self, tap: crate::script::lich::Tap) -> bool {
        let mut lich = self.lich.lock().unwrap_or_else(PoisonError::into_inner);
        if lich.as_ref().is_some_and(crate::script::lich::Tap::is_open) {
            return false;
        }
        *lich = Some(tap);
        true
    }

    /// Copy a chunk of the game's bytes, as it arrived, to the Lich attached,
    /// if one is. A Lich that stopped, or fell behind, is let go here.
    pub(crate) fn wire(&self, chunk: &[u8]) {
        let mut lich = self.lich.lock().unwrap_or_else(PoisonError::into_inner);
        if lich.as_ref().is_some_and(|tap| !tap.copy(chunk)) {
            *lich = None;
        }
    }

    /// Hand a line the player typed to the Lich attached: `None` with none
    /// attached, and `Some(false)` when it has too many waiting.
    pub(crate) fn hand_to_lich(&self, line: &str) -> Option<bool> {
        let lich = self.lich.lock().unwrap_or_else(PoisonError::into_inner);
        lich.as_ref()
            .filter(|tap| tap.is_open())
            .map(|tap| tap.hand(line))
    }

    /// A script runner's hooks.
    pub(crate) fn hooks(&self) -> &crate::script::Hooks {
        &self.hooks
    }

    /// Publish each finished line as the game sent it too, or stop.
    pub(crate) fn hear_lines(&self, on: bool) {
        self.hearing.store(on, Ordering::Relaxed);
    }

    /// Whether each finished line is published as the game sent it too.
    pub(crate) fn hears_lines(&self) -> bool {
        self.hearing.load(Ordering::Relaxed)
    }

    /// Publish container looks sorted, or as the game sent them.
    pub(crate) fn sort_containers(&self, on: bool) {
        self.sorting.store(on, Ordering::Relaxed);
    }

    /// Whether container looks are published sorted.
    pub(crate) fn sorts_containers(&self) -> bool {
        self.sorting.load(Ordering::Relaxed)
    }

    /// Answer each line published from now on with `triggers`.
    pub(crate) fn set_triggers(&self, triggers: Matcher) {
        *self.triggers.lock().unwrap_or_else(PoisonError::into_inner) = Answering {
            matcher: Arc::new(triggers),
            ..Answering::default()
        };
    }

    /// The triggers each line is answered with.
    pub(crate) fn triggers(&self) -> Arc<Matcher> {
        Arc::clone(
            &self
                .triggers
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .matcher,
        )
    }

    /// Read the conditions against `state`: the triggers they are in, and
    /// the ranks of those that became true.
    pub(crate) fn fire_conditions(&self, state: &GameState) -> (Arc<Matcher>, Vec<usize>) {
        let mut answering = self.triggers.lock().unwrap_or_else(PoisonError::into_inner);
        let Answering { matcher, edges, .. } = &mut *answering;
        let fired = if matcher.conditions().is_empty() {
            Vec::new()
        } else {
            edges.fire(matcher, state)
        };
        (Arc::clone(matcher), fired)
    }

    /// What `attention` and `acts`, from the triggers that fired, may do at
    /// game second `now`: each trigger once in its cooldown, for both
    /// together, then the sends at the character's pace.
    pub(crate) fn admit(
        &self,
        attention: Vec<Attention>,
        acts: Vec<Act>,
        now: Option<u32>,
    ) -> Admitted {
        let mut admitted = Admitted {
            attention: Vec::new(),
            acts: Vec::new(),
            held: Vec::new(),
            say_held: false,
        };
        if attention.is_empty() && acts.is_empty() {
            return admitted;
        }
        let mut answering = self.triggers.lock().unwrap_or_else(PoisonError::into_inner);
        let Answering {
            cooldowns, pace, ..
        } = &mut *answering;
        // One admission per trigger, whether it calls, sends, or both.
        let mut decided: Vec<(String, bool)> = Vec::new();
        let mut may = |trigger: &str, cooldown: u32| {
            if let Some((_, may)) = decided.iter().find(|(name, _)| name == trigger) {
                return *may;
            }
            let may = cooldowns.admit(trigger, cooldown, now);
            decided.push((trigger.to_owned(), may));
            may
        };
        admitted.attention = attention
            .into_iter()
            .filter(|call| may(&call.trigger, call.cooldown))
            .collect();
        let acts: Vec<Act> = acts
            .into_iter()
            .filter(|act| may(&act.trigger, act.cooldown))
            .collect();
        (admitted.acts, admitted.held) = pace.admit(acts, now);
        admitted.say_held = !admitted.held.is_empty() && pace.say_held(now);
        admitted
    }

    /// A publisher over a caller's own legacy channel, with a fenced stream
    /// nobody can subscribe to.
    ///
    /// For [`SessionHandle::new`](crate::SessionHandle::new), which is public
    /// and takes a raw `broadcast::Sender` -- tests build a handle with no
    /// session behind it. Such a handle has no observers, so an unread fenced
    /// stream loses nothing.
    pub(crate) fn from_legacy(
        legacy: broadcast::Sender<Event>,
        generation: GenerationCell,
    ) -> Self {
        // A handle with no session behind it names no real one; `FIRST` is
        // what every such handle has always carried.
        let mut publisher = Self::new(1, generation, SessionId::FIRST);
        publisher.legacy = legacy;
        publisher
    }

    pub(crate) fn send(&self, event: Event) -> Result<usize, broadcast::error::SendError<Event>> {
        self.publish(event).1
    }

    /// Publish `event`, and say the cursor it was published at: where a
    /// reader of the numbered stream finds it (a sent line's, for a script
    /// that reads what came after it, `plan/46` §3).
    pub(crate) fn numbered(&self, event: Event) -> u64 {
        self.publish(event).0
    }

    fn publish(&self, event: Event) -> (u64, Result<usize, broadcast::error::SendError<Event>>) {
        // Only control events touch this lock, never incoming frames. The
        // owner publishes the fact once and snapshots read that same fact.
        match &event {
            Event::ConnectFailed {
                attempt,
                delay,
                detail,
            } => {
                *self
                    .retry
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(RetryStatus {
                    attempt: *attempt,
                    delay: *delay,
                    detail: detail.clone(),
                });
            }
            Event::StateChanged(_) => {
                *self
                    .retry
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
            }
            _ => {}
        }
        let _fence = self
            .fence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let cursor = self.cursor.fetch_add(1, Ordering::Relaxed) + 1;
        if self.observed.receiver_count() > 0 {
            let _ = self.observed.send(ObservedEvent {
                session: self.session,
                generation: self.generation.get(),
                cursor,
                event: event.clone(),
            });
        }
        (cursor, self.legacy.send(event))
    }

    /// The session every event and snapshot from this publisher names.
    pub(crate) const fn session(&self) -> SessionId {
        self.session
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.legacy.subscribe()
    }

    pub(crate) fn snapshot(&self, state: &GameState, lifecycle: State) -> Snapshot {
        Snapshot {
            session: self.session,
            generation: self.generation.get(),
            cursor: self.cursor.load(Ordering::Relaxed),
            state: state.clone(),
            lifecycle,
            retry: self
                .retry
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
            stopped: None,
            triggers: self.triggers(),
        }
    }

    pub(crate) fn answer(&self, request: Request, state: &GameState, lifecycle: State) {
        if !request.is_closed() {
            // Under the fence: no notice can take a cursor between the
            // snapshot's and the subscription's start.
            let subscription = {
                let _fence = self
                    .fence
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                (self.snapshot(state, lifecycle), self.observed.subscribe())
            };
            let _ = request.send(subscription);
        }
    }
}

/// First wait after a retryable observation failure, doubled to the cap.
const RETRY_FIRST: Duration = Duration::from_millis(100);
/// The longest wait between observation attempts: bounded, never a spin.
const RETRY_CAP: Duration = Duration::from_secs(2);
/// The most events [`catch_up`] reads from an old receiver.
const MAX_CATCH_UP: usize = 4096;

/// Ask for a fresh observation with `subscribe`, retrying the failures that
/// are not an answer. `None` once `stop` is cancelled; otherwise the
/// subscription, or the error that ends observation. Every viewer that
/// follows a session asks this way (`cena-web`'s pump, `cena-gui`'s feed).
///
/// # Why a viewer does not stop at the first error
///
/// The web pump used to map **every** [`ObserveError`] to a fatal I/O error.
/// That ended the web server, disconnected every viewer, and was reported
/// only at shutdown -- for `Busy` (the owner's bounded inbox was momentarily
/// full) and `Timeout` (it did not answer within its budget), which are
/// *retryable read failures, not proof the owner died*. A busy owner under a
/// login burst is exactly when a viewer is most wanted.
///
/// So those two are retried with doubling backoff capped at two seconds --
/// bounded, never a spin -- and without a retry limit: a timeout cannot prove
/// death, and `stop` is what ends the wait. Only `Closed`, the owner gone
/// without a final snapshot, is terminal. Whatever the viewer last showed
/// stays up meanwhile: a stale view it refuses to *replace* with a guess.
pub async fn retrying<T, F, Fut>(
    stop: &tokio_util::sync::CancellationToken,
    mut subscribe: F,
) -> Option<Result<T, ObserveError>>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, ObserveError>>,
{
    let mut delay = RETRY_FIRST;
    loop {
        let result = tokio::select! {
            () = stop.cancelled() => return None,
            result = subscribe() => result,
        };
        match result {
            Err(ObserveError::Busy | ObserveError::Timeout) => {}
            answered => return Some(answered),
        }
        tokio::select! {
            () = stop.cancelled() => return None,
            () = tokio::time::sleep(delay) => {}
        }
        delay = (delay * 2).min(RETRY_CAP);
    }
}

/// The events still on `old` after cursor `seen`, up to and including
/// `fence` -- a fresh snapshot's cursor -- in order, and whether they were
/// all there.
///
/// A viewer that asks for a fresh snapshot while holding the receiver of its
/// last one reads what that receiver has up to the new snapshot's cursor:
/// those were published before the snapshot answered, and the new receiver
/// starts after them. It never waits for a missing event, and reads at most
/// a fixed budget; `false` means some were lost (lag, or the budget), and
/// the viewer says its story has a hole.
pub fn catch_up(
    old: &mut broadcast::Receiver<ObservedEvent>,
    seen: u64,
    fence: u64,
) -> (Vec<ObservedEvent>, bool) {
    let mut at = seen;
    let mut events = Vec::new();
    for _ in 0..MAX_CATCH_UP {
        if at >= fence {
            break;
        }
        match old.try_recv() {
            Ok(event) if event.cursor <= fence => {
                if event.cursor > at {
                    at = event.cursor;
                    events.push(event);
                }
            }
            _ => return (events, false),
        }
    }
    (events, at >= fence)
}
