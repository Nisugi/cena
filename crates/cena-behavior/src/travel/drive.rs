//! The thin `async` layer round [`Trip`] (`plan/24` stage 4c).
//!
//! Everything that decides is in [`Trip`], which has no socket and no clock.
//! This file is what has them: it holds the authority, keeps a [`GameState`]
//! by folding the session's events (a behavior has no other read of the
//! model), says where the character is, sends what the trip says to send, and
//! does the [`Deed`]s.
//!
//! # Stopping (`plan/12` §4.3, and the author's one exception)
//!
//! Every await is raced against the stop token, as `look` documents. A
//! stopped behavior's cleanup "cannot send commands" -- otherwise *stop*
//! becomes *send more*. The author, 2026-09-21, allows exactly one thing:
//! **one command per stored item, to take it back, and then it stops.** No
//! retry and no waiting to see, because the character may not be able to hold
//! a shield where it now stands. What did not come back is in
//! [`Travelled::still_stored`], for a frontend to say.
//!
//! The stance is **not** put back on a stop: that would be a second kind of
//! command, and the ruling was for one. [`Travelled::stance_before`] says
//! what it was.
//!
//! A dead or disconnected session sends nothing at all.

use std::time::Duration;

use cena_map::{Located, Map, Origin as Whence, RoomId, Sighting, Uid, title_from_subtitle};
use cena_session::{
    AuthorityToken, CommandId, Event, Frame, GameState, Gate, Origin, Outcome, Sent, SessionHandle,
    Snapshot, State,
};
use tokio::sync::broadcast::{Receiver, error::RecvError, error::TryRecvError};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use super::hands::{Stored, cast_commands, store_commands, take_back};
use super::{Deed, Now, Said, TravelNotes, Trip, Why, walker_from};
use crate::BehaviorError;

/// How long a [`Said::Hold`] waits for the game before ticking anyway: the
/// trip's own timeouts only run when it is ticked.
pub const BEAT: Duration = Duration::from_millis(250);

/// How long one command of a deed may go unanswered. Lich's `dothistimeout`
/// for a society power is 3 (`effect-list.xml`, 9704's `cast-proc`).
pub const DEED_DEADLINE: Duration = Duration::from_secs(3);

/// How long the walker waits for its group before going on without it.
pub const FOLLOW_WAIT: Duration = Duration::from_secs(30);

/// How a trip ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    Arrived,
    /// The trip gave up, and why.
    Failed(Why),
    /// A spell the map names is not in the spell table.
    UnknownSpell,
    /// Stopped, or the session went away.
    Stopped(BehaviorError),
}

/// What a finished trip leaves for whoever started it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Travelled {
    pub ended: Ended,
    /// Stored by the trip and not seen to come back.
    pub still_stored: Vec<Stored>,
    /// The stance the trip replaced and did not put back.
    pub stance_before: Option<String>,
    /// Exits the game said do not exist, as `(from, to)`.
    pub wrong_for_the_map: Vec<(RoomId, RoomId)>,
}

/// Walk to `goal`.
///
/// `joined` is `Session::subscribe`'s pair. `notes` is the character's travel
/// file; `wrote` is called each time a crossing changes it, so the caller can
/// save it then and not at the end -- a memory lost strands the character.
#[allow(clippy::too_many_arguments)]
pub async fn travel(
    handle: &SessionHandle,
    cancel: &CancellationToken,
    next_id: impl FnMut() -> CommandId,
    token: AuthorityToken,
    joined: (Snapshot, Receiver<Event>),
    map: &Map,
    goal: RoomId,
    notes: &mut TravelNotes,
    wrote: impl FnMut(&TravelNotes),
) -> Travelled {
    let mut trip = Trip::to(goal);
    if handle.claim(token).await.is_err() {
        return Travelled {
            ended: Ended::Stopped(BehaviorError::AuthorityHeld),
            still_stored: Vec::new(),
            stance_before: None,
            wrong_for_the_map: Vec::new(),
        };
    }
    let (snapshot, events) = joined;
    let mut driver = Driver {
        handle,
        cancel,
        token,
        next_id,
        state: snapshot.state,
        events,
        began: Instant::now(),
        was: None,
        stored: Vec::new(),
        stance_before: None,
        line: String::new(),
    };
    let ended = driver.walk(&mut trip, map, notes, wrote).await;
    if ended == Ended::Stopped(BehaviorError::Cancelled) {
        driver.take_back_once().await;
    }
    // Released on every exit. A release is not a command.
    handle.release(token);
    Travelled {
        ended,
        still_stored: driver.stored,
        stance_before: driver.stance_before,
        wrong_for_the_map: trip.wrong_for_the_map().to_vec(),
    }
}

struct Driver<'a, N> {
    handle: &'a SessionHandle,
    cancel: &'a CancellationToken,
    token: AuthorityToken,
    next_id: N,
    state: GameState,
    events: Receiver<Event>,
    began: Instant,
    /// The game's room id and the room it was located as, last tick.
    was: Option<(String, RoomId)>,
    stored: Vec<Stored>,
    stance_before: Option<String>,
    /// The main window's line so far.
    line: String,
}

impl<N: FnMut() -> CommandId> Driver<'_, N> {
    async fn walk(
        &mut self,
        trip: &mut Trip,
        map: &Map,
        notes: &mut TravelNotes,
        mut wrote: impl FnMut(&TravelNotes),
    ) -> Ended {
        loop {
            if self.cancel.is_cancelled() {
                return Ended::Stopped(BehaviorError::Cancelled);
            }
            if let Err(gone) = self.drain(trip) {
                return Ended::Stopped(gone);
            }
            let now = Now {
                here: self.locate(map),
                ms: u64::try_from(self.began.elapsed().as_millis()).unwrap_or(u64::MAX),
            };
            let server = self.state.game_time_now().unwrap_or(0);
            let walker = walker_from(&self.state, notes, server);
            let done = match trip.tick(map, &walker, now) {
                Said::Arrived => return Ended::Arrived,
                Said::Failed(why) => return Ended::Failed(why),
                Said::Hold => self.hold(trip).await,
                // A move causes no roundtime, and is verified by the room it
                // lands in: the trip's own timeout is the deadline.
                Said::Send(line) => self.send(&line).await,
                Said::Do(Deed::Remember(name, value)) => {
                    notes.memories.insert(name, value);
                    wrote(notes);
                    Ok(())
                }
                Said::Do(Deed::Forget(name)) => {
                    notes.memories.remove(&name);
                    wrote(notes);
                    Ok(())
                }
                Said::Do(deed) => self.deed(trip, deed).await,
            };
            if let Err(ended) = done {
                return ended;
            }
        }
    }

    /// Which room of the map the model's room is, by the game's number first.
    fn locate(&mut self, map: &Map) -> Option<RoomId> {
        let room = &self.state.room;
        let raw = room.id.clone()?;
        let whence = match &self.was {
            Some((id, at)) if *id == raw => Whence::Still(*at),
            Some((_, at)) => Whence::Left(*at),
            None => Whence::Nowhere,
        };
        let title = room.title.as_deref().map(title_from_subtitle);
        let sighting = Sighting {
            uid: raw.parse().ok().filter(|uid| *uid != 0).map(Uid),
            title: title.as_deref(),
            ..Sighting::default()
        };
        let Located::Here { room: here, .. } = map.locate(&sighting, whence) else {
            return None;
        };
        self.was = Some((raw, here));
        Some(here)
    }

    /// Fold every event already waiting, without waiting for more.
    fn drain(&mut self, trip: &mut Trip) -> Result<(), BehaviorError> {
        loop {
            match self.events.try_recv() {
                Ok(event) => self.fold(trip, &event)?,
                // Events were dropped: what is known is a little old, and the
                // game's next word mends it.
                Err(TryRecvError::Lagged(_)) => {}
                Err(TryRecvError::Empty) => return Ok(()),
                Err(TryRecvError::Closed) => return Err(BehaviorError::Dead),
            }
        }
    }

    fn fold(&mut self, trip: &mut Trip, event: &Event) -> Result<(), BehaviorError> {
        match event {
            Event::Frame(frame) => {
                self.state.apply(frame);
                match &**frame {
                    // A line arrives in pieces, one per link boundary, and
                    // Lich's patterns are over whole lines of the main window.
                    Frame::Text(text) if text.stream.is_empty() => {
                        self.line.push_str(&text.content);
                        if text.ends_line {
                            trip.heard(&std::mem::take(&mut self.line));
                        }
                    }
                    Frame::Prompt { .. } => trip.prompted(),
                    _ => {}
                }
                Ok(())
            }
            // `plan/12` §5.1: no automation runs without a transport. Only
            // the two states that mean it is gone: a session still on its
            // way up to `Ready` announces each step, and has lost nothing.
            Event::StateChanged(State::Reconnecting) => Err(BehaviorError::Disconnected),
            Event::StateChanged(State::Closed) => Err(BehaviorError::Dead),
            _ => Ok(()),
        }
    }

    /// Wait for the game's next word, or a beat, whichever is first.
    async fn hold(&mut self, trip: &mut Trip) -> Result<(), Ended> {
        let event = tokio::select! {
            biased;
            () = self.cancel.cancelled() => return Err(Ended::Stopped(BehaviorError::Cancelled)),
            event = self.events.recv() => event,
            () = tokio::time::sleep(BEAT) => return Ok(()),
        };
        match event {
            Ok(event) => self.fold(trip, &event).map_err(Ended::Stopped),
            Err(RecvError::Lagged(_)) => Ok(()),
            Err(RecvError::Closed) => Err(Ended::Stopped(BehaviorError::Dead)),
        }
    }

    async fn send(&mut self, line: &str) -> Result<(), Ended> {
        let sent = tokio::select! {
            biased;
            () = self.cancel.cancelled() => return Err(Ended::Stopped(BehaviorError::Cancelled)),
            sent = self.handle.send_now(line, Origin::Behavior(self.token), Gate::None) => sent,
        };
        match sent {
            // A refusal is a full queue: the trip's timeout sends it again.
            Sent::Ok { .. } | Sent::Refused(_) => Ok(()),
            Sent::Dead => Err(Ended::Stopped(BehaviorError::Dead)),
            Sent::Interrupted => Err(Ended::Stopped(BehaviorError::Disconnected)),
        }
    }

    /// Send one command of a deed and wait for the prompt that answers it.
    async fn exchange(&mut self, trip: &mut Trip, line: &str) -> Result<(), Ended> {
        let outcome = tokio::select! {
            biased;
            () = self.cancel.cancelled() => return Err(Ended::Stopped(BehaviorError::Cancelled)),
            outcome = self.handle.send_and_await(
                (self.next_id)(),
                line,
                Origin::Behavior(self.token),
                DEED_DEADLINE,
                |frame| matches!(frame, Frame::Prompt { .. }),
            ) => outcome,
        };
        match outcome {
            Outcome::Confirmed(_) | Outcome::Timeout | Outcome::Refused(_) => {
                self.drain(trip).map_err(Ended::Stopped)
            }
            Outcome::Interrupted => Err(Ended::Stopped(BehaviorError::Cancelled)),
            Outcome::Dead => Err(Ended::Stopped(BehaviorError::Dead)),
            Outcome::Disconnected => Err(Ended::Stopped(BehaviorError::Disconnected)),
        }
    }

    async fn deed(&mut self, trip: &mut Trip, deed: Deed) -> Result<(), Ended> {
        match deed {
            Deed::EmptyHands => {
                for (stored, command) in store_commands(&self.state) {
                    // Written down before it is sent: a stop between the two
                    // must still know what to take back.
                    self.stored.push(stored);
                    self.exchange(trip, command).await?;
                }
            }
            Deed::FillHands => {
                // Last stored, first back. One command each (`hands`).
                for stored in std::mem::take(&mut self.stored).into_iter().rev() {
                    let command = take_back(&self.state, &stored);
                    let asked = self.exchange(trip, &command).await;
                    if self.state.hand_holding(&stored.id).is_none() {
                        self.stored.push(stored);
                    }
                    asked?;
                }
            }
            Deed::Cast(spell) => self.cast(trip, &spell, None).await?,
            Deed::CastAt(spell, target) => self.cast(trip, &spell, Some(&target)).await?,
            Deed::Stance(stance) => {
                // The first one replaced is the one to go back to.
                if self.stance_before.is_none() {
                    self.stance_before.clone_from(&self.state.character.stance);
                }
                self.exchange(trip, &format!("stance {stance}")).await?;
            }
            Deed::RestoreStance => {
                if let Some(before) = self.stance_before.take() {
                    self.exchange(trip, &format!("stance {before}")).await?;
                }
            }
            Deed::AwaitFollowers => self.await_followers(trip).await?,
            // Handled where the notes are: `walk`.
            Deed::Remember(..) | Deed::Forget(_) => {}
        }
        Ok(())
    }

    async fn cast(&mut self, trip: &mut Trip, spell: &str, at: Option<&str>) -> Result<(), Ended> {
        let Some(commands) = cast_commands(spell, at) else {
            return Err(Ended::UnknownSpell);
        };
        for command in commands {
            self.exchange(trip, &command).await?;
        }
        Ok(())
    }

    /// Until everyone in the group is in the room, or [`FOLLOW_WAIT`].
    async fn await_followers(&mut self, trip: &mut Trip) -> Result<(), Ended> {
        let until = Instant::now() + FOLLOW_WAIT;
        while Instant::now() < until {
            let here = &self.state.room.players;
            let all_here = self
                .state
                .group
                .members()
                .iter()
                .all(|member| here.iter().any(|player| player.id == member.id));
            if all_here {
                break;
            }
            self.hold(trip).await?;
        }
        Ok(())
    }

    /// The author's one exception to "cleanup cannot send": one command per
    /// stored item, not waited for. They stay in `stored`, since nothing saw
    /// them come back.
    async fn take_back_once(&mut self) {
        for stored in self.stored.iter().rev() {
            let command = take_back(&self.state, stored);
            let origin = Origin::Behavior(self.token);
            let _ = self.handle.send_now(&command, origin, Gate::None).await;
        }
    }
}
