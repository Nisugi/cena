//! The characters an agent can see: the ones this Hydra is running, each with
//! its session's read-only observer and a log of what happened to it.
//!
//! The binary seats a character when it starts one and unseats it when it
//! stops, as it attaches and detaches Despana's pages (`crates/cena/src/play.rs`).
//! Seating starts a watcher that fills the character's [`Log`]; unseating
//! stops it.
//!
//! **What happens while the level forbids reading is not kept for later.**
//! The watcher runs whatever the level (it is Hydra's, not the agent's), but
//! while the character is below Observe it keeps only the level's own
//! changes; once reading is allowed again, the log forgets everything before,
//! so a `wait` from an older cursor answers `lagged` and the agent reads
//! `state` afresh rather than the stretch it was not allowed to see.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use cena_session::agent::{Change, Door, Level};
use cena_session::{Event, ObserveError, ObservedEvent, SessionId, SessionObserver, Snapshot};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use crate::happenings::{Happening, Log, changed, diff};
use crate::projection::{CharacterState, project};

/// A seated character, as the tools read it.
#[derive(Clone, Debug)]
pub struct Seat {
    /// The game it is on, by its code (`GS3`).
    pub game: String,
    /// The name Hydra runs it as.
    pub name: String,
    /// Read-only access to its session.
    pub observer: SessionObserver,
    /// The only way to act on it, each act checked against its level.
    pub door: Door,
    /// Its database (the combat recorder and the loot ledger), when known.
    pub database: Option<PathBuf>,
    /// Whether this run records combat and loot into it (`--record`);
    /// `None` when whoever seated it did not say.
    pub recording: Option<bool>,
    /// What happened to it.
    pub log: Arc<Log>,
    stop: CancellationToken,
}

impl Seat {
    /// How an agent names it among `seats`: its name, and `GAME:Name` when
    /// another seated character has the same name, as `;to` names it.
    #[must_use]
    pub fn label(&self, seats: &[Seat]) -> String {
        let twice = seats
            .iter()
            .filter(|other| other.name.eq_ignore_ascii_case(&self.name))
            .count()
            > 1;
        if twice {
            format!("{}:{}", self.game, self.name)
        } else {
            self.name.clone()
        }
    }
}

/// Every seated character. Cloned freely: one table behind it.
#[derive(Clone, Debug, Default)]
pub struct Characters {
    seats: Arc<Mutex<BTreeMap<SessionId, Seat>>>,
}

impl Characters {
    /// Seat a character, `name` on `game` (its code, `GS3`), and start
    /// watching it. Called inside a Tokio runtime.
    pub fn seat(
        &self,
        id: SessionId,
        (game, name): (&str, &str),
        observer: SessionObserver,
        door: Door,
        database: Option<PathBuf>,
        recording: Option<bool>,
    ) {
        let seat = Seat {
            game: game.to_owned(),
            name: name.to_owned(),
            observer,
            door,
            database,
            recording,
            log: Arc::default(),
            stop: CancellationToken::new(),
        };
        tokio::spawn(watch(seat.clone()));
        if let Some(old) = self.lock().insert(id, seat) {
            old.stop.cancel();
        }
    }

    /// Unseat a character, and stop watching it.
    pub fn unseat(&self, id: SessionId) {
        if let Some(seat) = self.lock().remove(&id) {
            seat.stop.cancel();
            seat.log.close();
        }
    }

    /// Every seated character, in the order they were seated.
    #[must_use]
    pub fn all(&self) -> Vec<Seat> {
        self.lock().values().cloned().collect()
    }

    /// A seated character by what an agent called it, ignoring case: its
    /// name, or `GAME:Name` ([`Seat::label`]).
    ///
    /// # Errors
    ///
    /// How each character it could be is named: two or more when a bare name
    /// is on two games, which picks neither, and none when nothing seated is
    /// called that.
    ///
    /// The first seat by that name was taken, so a name on two games reached
    /// whichever was seated first (the integrated crate review of
    /// 2026-09-28, I3), as `;to` did before it (`crates/cena/src/relay.rs`).
    pub fn named(&self, asked: &str) -> Result<Seat, Vec<String>> {
        let asked = asked.trim();
        let (game, name) = match asked.split_once(':') {
            Some((game, name)) => (Some(game.trim()), name.trim()),
            None => (None, asked),
        };
        let seats = self.all();
        let mut found: Vec<&Seat> = seats
            .iter()
            .filter(|seat| seat.name.eq_ignore_ascii_case(name))
            .filter(|seat| game.is_none_or(|game| seat.game.eq_ignore_ascii_case(game)))
            .collect();
        if found.len() == 1 {
            return Ok(found.remove(0).clone());
        }
        Err(found.iter().map(|seat| seat.label(&seats)).collect())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<SessionId, Seat>> {
        self.seats.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A fresh snapshot and the events after it, retrying the failures that are
/// not an answer (`Busy`, `Timeout`) with a bounded backoff, as Despana's pump
/// does. `None` when stopped or the session is gone.
pub async fn subscribe(
    observer: &SessionObserver,
    stop: &CancellationToken,
) -> Option<(Snapshot, broadcast::Receiver<ObservedEvent>)> {
    let mut backoff = Duration::from_millis(50);
    loop {
        match observer.subscribe().await {
            Ok(subscription) => return Some(subscription),
            Err(ObserveError::Busy | ObserveError::Timeout) => {
                tokio::select! {
                    () = stop.cancelled() => return None,
                    () = tokio::time::sleep(backoff) => {}
                }
                backoff = (backoff * 2).min(Duration::from_secs(2));
            }
            Err(_) => return None,
        }
    }
}

/// How long a tool waits on a session for a copy of it: a `wait`'s own
/// longest.
pub const READ_DEADLINE: Duration = Duration::from_secs(30);

/// [`subscribe`] for a tool answering a request: `None` too when the
/// session has not answered in [`READ_DEADLINE`].
///
/// `subscribe` tries again for as long as the session is busy, until it is
/// stopped, and the tools gave it a token nothing stops: with the session's
/// actor wedged, `state`, `characters`, `spell`, `route` and `seconds` never
/// answered, where `wait` answers in thirty seconds (the review of
/// 2026-09-29).
pub async fn read(
    observer: &SessionObserver,
) -> Option<(Snapshot, broadcast::Receiver<ObservedEvent>)> {
    let stop = CancellationToken::new();
    tokio::time::timeout(READ_DEADLINE, subscribe(observer, &stop))
        .await
        .ok()
        .flatten()
}

/// The session's own events that are happenings as they are.
fn direct(event: &Event) -> Option<Happening> {
    match event {
        Event::Sent { line, origin, .. } => Some(Happening::Sent {
            line: line.clone(),
            origin: origin.word().to_owned(),
        }),
        Event::Notice(notice) => Some(Happening::Notice {
            text: notice.lines().join("\n"),
        }),
        Event::StateChanged(state) => Some(Happening::Lifecycle {
            state: format!("{state:?}").to_ascii_lowercase(),
        }),
        Event::Agent(Change::Level(level)) => Some(Happening::Level {
            level: level.word().to_owned(),
        }),
        Event::Agent(Change::Answered { id, approved }) => Some(Happening::Approval {
            id: *id,
            approved: *approved,
        }),
        Event::Agent(Change::Operation(report)) => Some(Happening::Operation {
            operation: crate::happenings::operation(report),
        }),
        _ => None,
    }
}

/// What goes into one character's log, by its level: everything while the
/// agent may read, only the level's changes while it may not, and a clean
/// break between.
///
/// **The level is followed through the stream, in order**, from the level's
/// own events: a happening is kept by the level at its place in the stream,
/// not by the level when the watcher reaches it. Read from the door instead,
/// a level lowered and raised again between two looks would hide nothing
/// (the first version did; its test caught it).
struct Keeper<'a> {
    log: &'a Log,
    level: Level,
    /// Something was held back since the agent last could read.
    hidden: bool,
}

impl<'a> Keeper<'a> {
    fn new(log: &'a Log, level: Level) -> Self {
        Self {
            log,
            level,
            hidden: level < Level::Observe,
        }
    }

    fn keep(&mut self, cursor: u64, happening: Happening) {
        if let Happening::Level { level } = &happening {
            self.level = Level::named(level).unwrap_or(Level::Off);
        }
        if self.visible(cursor) || matches!(happening, Happening::Level { .. }) {
            self.log.push(cursor, happening);
        }
    }

    /// A line of the game's text, kept by the same rule.
    fn keep_line(&mut self, cursor: u64, line: &cena_session::Line) {
        if self.visible(cursor) {
            self.log.line(cursor, &line.stream, line.text());
        }
    }

    /// Whether what happened at `cursor` may be kept: the level allows
    /// reading. The first thing kept after a stretch that was not lets go of
    /// everything before it.
    fn visible(&mut self, cursor: u64) -> bool {
        if self.level < Level::Observe {
            self.hidden = true;
            return false;
        }
        if std::mem::take(&mut self.hidden) {
            self.log.forget(cursor.saturating_sub(1));
        }
        true
    }
}

/// Whether the state may have changed enough to look again: a prompt closed
/// a chunk, combat was folded, or the lifecycle moved.
fn worth_a_look(event: &Event) -> bool {
    match event {
        Event::Frame(frame) => matches!(**frame, cena_session::Frame::Prompt { .. }),
        Event::Combat(_) | Event::StateChanged(_) => true,
        _ => false,
    }
}

/// Watch one character until stopped: its direct events into the log as
/// they come, and after each prompt a fresh snapshot, compared with the last.
async fn watch(seat: Seat) {
    let Seat {
        name,
        observer,
        door,
        log,
        stop,
        ..
    } = seat;
    let Some((snapshot, mut events)) = subscribe(&observer, &stop).await else {
        log.close();
        return;
    };
    // Read after subscribing: a change made in between is in the stream
    // too, and arrives again as the same level.
    let mut keeper = Keeper::new(&log, door.level());
    let mut last: CharacterState = project(&name, &snapshot);
    log.reached(snapshot.cursor);
    loop {
        let received = tokio::select! {
            () = stop.cancelled() => return,
            received = events.recv() => received,
        };
        let mut gap = false;
        let look = match received {
            Ok(event) => {
                if let Some(happening) = direct(&event.event) {
                    keeper.keep(event.cursor, happening);
                }
                if let Event::Line(line) = &event.event {
                    keeper.keep_line(event.cursor, line);
                }
                log.reached(event.cursor);
                worth_a_look(&event.event)
            }
            Err(broadcast::error::RecvError::Lagged(_)) => {
                gap = true;
                true
            }
            Err(broadcast::error::RecvError::Closed) => {
                log.close();
                return;
            }
        };
        if !look {
            continue;
        }
        let Some((snapshot, fresh)) = subscribe(&observer, &stop).await else {
            log.close();
            return;
        };
        // The events up to the new snapshot's fence are still in the old
        // receiver: take their direct happenings before letting it go.
        // Some lost on the way are said to have been, as the script's
        // watch says it (`scripts/watch.rs`): this stopped at a loss,
        // silently, and an `approval` among them was never heard of (the
        // review of 2026-09-29).
        loop {
            let event = match events.try_recv() {
                Ok(event) if event.cursor <= snapshot.cursor => event,
                Err(broadcast::error::TryRecvError::Lagged(_)) => {
                    gap = true;
                    continue;
                }
                Ok(_) | Err(_) => break,
            };
            if let Some(happening) = direct(&event.event) {
                keeper.keep(event.cursor, happening);
            }
            if let Event::Line(line) = &event.event {
                keeper.keep_line(event.cursor, line);
            }
        }
        events = fresh;
        if gap {
            keeper.keep(snapshot.cursor, Happening::Gap);
        }
        let now = project(&name, &snapshot);
        for happening in diff(&last, &now).into_iter().chain(changed(&last, &now)) {
            keeper.keep(snapshot.cursor, happening);
        }
        log.reached(snapshot.cursor);
        last = now;
    }
}
