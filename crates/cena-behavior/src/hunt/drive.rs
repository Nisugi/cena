//! The thin `async` layer round [`Hunt`] (`plan/30` §3): what has the socket
//! and the clock.
//!
//! Everything that decides is in [`Hunt`]. This file holds the authority,
//! keeps a [`GameState`] by folding the session's events (a behavior has no
//! other read of the model, `plan/24` stage 4), says where the character is
//! by the map, and does what the machine says: settle roundtime, send the
//! line through the write-time gate, walk by running travel inside the hunt's
//! own authority, or wait.
//!
//! # The action contract, in this order (`plan/30` §3, the author)
//!
//! 1. **Settle** roundtime and cast roundtime, folding events meanwhile.
//! 2. **Check against the state as it is now.** The machine's guards read
//!    the folded state at the tick; the session's [`Gate::Act`] reads the
//!    live model at the moment the bytes go out (`actor/gate.rs`), and
//!    refuses a step whose target has gone, or a character stunned, webbed,
//!    dead or in roundtime.
//! 3. **Send**, and take the next prompt as the end of the round trip.
//!
//! A refusal is a skip: the next tick decides again from scratch.
//!
//! # A walk inside the hunt
//!
//! Rest and wander walk. The walk is travel's own driver, run through
//! [`travel_holding`] under the hunt's token, given a second listener on the
//! same stream ([`Heard::resubscribe`]) and a copy of the state. The hunt
//! keeps folding its own stream meanwhile, so that when the walk returns the
//! hunt's state is as current as the walk's, and nothing was missed. The
//! walk reads the character's travel notes and what it learns is written
//! back after each walk, as travel's own desk does.
//!
//! # Stopping (`plan/12` §4.3)
//!
//! Every await is raced against the stop token. A stopped hunt sends
//! nothing more. The watchdog is the caller's ([`crate::watch`]): this loop
//! beats the [`Heartbeat`] once a turn, and a turn that never comes round is
//! what the watchdog preempts.

mod errands;
mod loot;
mod selling;

use std::time::Duration;

use cena_map::{Map, Origin as Whence, RoomId};
use cena_session::{
    AuthorityToken, CommandId, Event, Frame, GameState, Gate, Notice, NoticeKind, Origin, Outcome,
    Refusal, SessionHandle, Snapshot, State,
};
use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use tokio_util::sync::CancellationToken;

use super::engine::{Ending, Here, Hunt, Said};
use crate::error::BehaviorError;
use crate::loot::Memory;
use crate::travel::{Ended, Heard, TravelNotes, room_of, travel_holding};
use crate::watchdog::Heartbeat;

/// How long a sent line may wait for its prompt.
const SEND_DEADLINE: Duration = Duration::from_secs(8);
/// How long roundtime is waited out before the tick is given up.
const SETTLE_CAP: Duration = Duration::from_secs(15);
/// The idle beat: how often the loop turns with nothing to do.
const BEAT: Duration = Duration::from_millis(250);
/// The most commands one visit's looting sends before it is given up on.
const LOOT_STEPS: usize = 64;
/// The most steps one selling round takes before it is given up on.
const SELL_STEPS: usize = 400;
/// The most steps one heal takes before it is given up on.
const HEAL_STEPS: usize = 120;
/// The most steps one stocking round takes before it is given up on.
const STOCK_STEPS: usize = 400;
/// The most steps one waggle run takes before it is given up on.
const WAGGLE_STEPS: usize = 400;

/// How a hunt ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HuntEnd {
    /// The machine decided ([`Ending`]).
    Finished(Ending),
    /// The session decided: stopped, refused, disconnected, dead.
    Stopped(BehaviorError),
}

/// Hunt on `profile` until the machine or the session ends it. The caller
/// holds `token` already (the desk claims and releases).
#[allow(clippy::too_many_arguments)]
pub async fn hunt(
    handle: &SessionHandle,
    cancel: &CancellationToken,
    next_id: impl FnMut() -> CommandId,
    token: AuthorityToken,
    joined: (Snapshot, Heard),
    map: &Map,
    machine: Hunt,
    heartbeat: &Heartbeat,
    notes: TravelNotes,
    wrote: impl FnMut(&TravelNotes) + Send,
    learned: impl FnMut(&[String]) + Send,
) -> HuntEnd {
    let (snapshot, events) = joined;
    // What the profile already says cannot be skinned; a name learned beyond
    // it is written back.
    let saved_unskinnable = machine
        .loot_profile()
        .map(|profile| profile.skin.unskinnable.iter().cloned().collect())
        .unwrap_or_default();
    let mut driver = Driver {
        handle,
        cancel,
        token,
        next_id,
        session: snapshot.session,
        lifecycle: snapshot.lifecycle,
        generation: snapshot.generation,
        cursor: snapshot.cursor,
        state: snapshot.state,
        events,
        map,
        machine,
        last_room: None,
        notes,
        wrote,
        learned,
        saved_unskinnable,
        memory: Memory::default(),
        transcript: String::new(),
    };
    let end = driver.run(heartbeat).await;
    let text = match end {
        HuntEnd::Finished(ending) => format!("Hunt: over: {ending}."),
        HuntEnd::Stopped(BehaviorError::Cancelled) => "Hunt: stopped.".to_owned(),
        HuntEnd::Stopped(why) => format!("Hunt: ended: {why:?}."),
    };
    handle.say(Notice::line(NoticeKind::Info, text));
    end
}

struct Driver<'a, F: FnMut() -> CommandId, W: FnMut(&TravelNotes), L: FnMut(&[String])> {
    handle: &'a SessionHandle,
    cancel: &'a CancellationToken,
    token: AuthorityToken,
    next_id: F,
    session: cena_session::SessionId,
    lifecycle: State,
    generation: cena_session::Generation,
    cursor: u64,
    state: GameState,
    events: Heard,
    map: &'a Map,
    machine: Hunt,
    last_room: Option<RoomId>,
    /// The character's travel file: what earlier crossings wrote down, read
    /// once and kept as a walk changes it.
    notes: TravelNotes,
    wrote: W,
    /// Told the creatures learned unskinnable, to write into the profile.
    learned: L,
    /// The unskinnable names the profile holds, and those already told.
    saved_unskinnable: std::collections::BTreeSet<String>,
    /// What looting learned: full bags, autoclosers, crumbly names.
    memory: Memory,
    /// The main window's text since the last loot command was sent, for
    /// reading its reply.
    transcript: String,
}

impl<F: FnMut() -> CommandId, W: FnMut(&TravelNotes), L: FnMut(&[String])> Driver<'_, F, W, L> {
    async fn run(&mut self, heartbeat: &Heartbeat) -> HuntEnd {
        loop {
            heartbeat.beat();
            if let Err(gone) = self.drain() {
                return HuntEnd::Stopped(gone);
            }
            let here = self.locate();
            let exits: Vec<RoomId> = here
                .and_then(|room| self.map.room(room))
                .map(|room| {
                    room.exits
                        .iter()
                        .filter(|exit| exit.crossing.is_crossable())
                        .map(|exit| exit.to)
                        .collect()
                })
                .unwrap_or_default();
            let now = self.state.game_time_now();
            let said = self.machine.tick(
                &self.state,
                Here {
                    room: here,
                    exits: &exits,
                },
                now,
            );
            for note in self.machine.take_notes() {
                self.handle
                    .say(Notice::line(NoticeKind::Info, format!("Hunt: {note}")));
            }
            let step = match said {
                Said::Nothing => self.hold(BEAT).await,
                Said::Wait(seconds) => self.hold(Duration::from_secs(u64::from(seconds))).await,
                Said::Send { line, target } => {
                    self.transcript.clear();
                    let sent = self.send(&line, target).await;
                    let now = self.state.game_time_now();
                    self.machine.replied(self.transcript.lines(), now);
                    sent
                }
                Said::Walk(to) => self.walk(to).await,
                Said::Loot(corpses) => self.loot(&corpses).await,
                Said::Sell => self.sell().await,
                Said::Heal => self.heal().await,
                Said::Stock(fill) => self.stock(fill).await,
                Said::Waggle(targets) => self.waggle(&targets).await,
                Said::Done(ending) => return HuntEnd::Finished(ending),
            };
            if let Err(end) = step {
                return end;
            }
        }
    }

    /// Where the character is, by the map: from the room the game names,
    /// else near where the hunt last was.
    fn locate(&mut self) -> Option<RoomId> {
        let whence = self.last_room.map_or(Whence::Nowhere, Whence::Still);
        let here = room_of(self.map, &self.state, whence)
            .or_else(|| room_of(self.map, &self.state, Whence::Nowhere));
        if here.is_some() {
            self.last_room = here;
        }
        here
    }

    fn drain(&mut self) -> Result<(), BehaviorError> {
        loop {
            match self.events.try_recv() {
                Ok(event) => self.fold(&event)?,
                Err(TryRecvError::Lagged(_)) => {}
                Err(TryRecvError::Empty) => return Ok(()),
                Err(TryRecvError::Closed) => return Err(BehaviorError::Dead),
            }
        }
    }

    fn fold(&mut self, event: &Event) -> Result<(), BehaviorError> {
        if let Event::Frame(frame) = event
            && let Frame::Text(text) = &**frame
            && text.stream.is_empty()
        {
            self.transcript.push_str(&text.content);
        }
        fold_into(&mut self.state, event)
    }

    /// Fold events for up to `for_`, or until stopped.
    async fn hold(&mut self, for_: Duration) -> Result<(), HuntEnd> {
        let deadline = tokio::time::Instant::now() + for_;
        loop {
            let event = tokio::select! {
                biased;
                () = self.cancel.cancelled() => return Err(HuntEnd::Stopped(BehaviorError::Cancelled)),
                event = self.events.recv() => event,
                () = tokio::time::sleep_until(deadline) => return Ok(()),
            };
            match event {
                Ok(event) => self.fold(&event).map_err(HuntEnd::Stopped)?,
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => return Err(HuntEnd::Stopped(BehaviorError::Dead)),
            }
        }
    }

    /// Wait out roundtime and cast roundtime, up to the cap.
    async fn settle(&mut self) -> Result<(), HuntEnd> {
        let cap = tokio::time::Instant::now() + SETTLE_CAP;
        while self.state.in_roundtime() == Some(true) || self.state.in_casttime() == Some(true) {
            if tokio::time::Instant::now() >= cap {
                return Ok(());
            }
            self.hold(BEAT).await?;
        }
        Ok(())
    }

    /// Settle, then send through the gate, then take the prompt.
    async fn send(&mut self, line: &str, target: Option<i64>) -> Result<(), HuntEnd> {
        self.settle().await?;
        let id = (self.next_id)();
        let outcome = tokio::select! {
            biased;
            () = self.cancel.cancelled() => return Err(HuntEnd::Stopped(BehaviorError::Cancelled)),
            outcome = self.handle.send_gated(
                id,
                line,
                Origin::Behavior(self.token),
                SEND_DEADLINE,
                |frame| matches!(frame, Frame::Prompt { .. }),
                Gate::Act { target },
            ) => outcome,
        };
        if let Some(gone) = BehaviorError::from_outcome(&outcome) {
            return Err(HuntEnd::Stopped(gone));
        }
        if let Outcome::Refused(refusal) = &outcome {
            if matches!(refusal, Refusal::TargetGone) {
                self.machine.target_gone();
            }
            // A refusal is a skip: the next tick decides again. A beat, so a
            // refusal that repeats does not spin.
            self.hold(BEAT).await?;
        }
        self.drain().map_err(HuntEnd::Stopped)
    }

    /// Walk to `to` with travel's driver under this authority, folding this
    /// hunt's own stream meanwhile.
    async fn walk(&mut self, to: RoomId) -> Result<(), HuntEnd> {
        let snapshot = Snapshot {
            session: self.session,
            state: self.state.clone(),
            lifecycle: self.lifecycle,
            generation: self.generation,
            cursor: self.cursor,
            retry: None,
        };
        let listener = self.events.resubscribe();
        let mut notes = std::mem::take(&mut self.notes);
        let travelled = {
            let handle = self.handle;
            let cancel = self.cancel;
            let token = self.token;
            let map = self.map;
            let next_id = &mut self.next_id;
            let mut walk = Box::pin(travel_holding(
                handle,
                cancel,
                next_id,
                token,
                (snapshot, listener),
                map,
                to,
                &mut notes,
                |_| {},
            ));
            // The walk holds `next_id`; this loop touches only the stream
            // and the state, so both borrows stand.
            let events = &mut self.events;
            let state = &mut self.state;
            loop {
                let event = tokio::select! {
                    biased;
                    travelled = &mut walk => break travelled,
                    event = events.recv() => event,
                };
                match event {
                    Ok(event) => {
                        if let Err(gone) = fold_into(state, &event) {
                            return Err(HuntEnd::Stopped(gone));
                        }
                    }
                    Err(RecvError::Lagged(_)) => {}
                    Err(RecvError::Closed) => return Err(HuntEnd::Stopped(BehaviorError::Dead)),
                }
            }
        };
        self.notes = notes;
        (self.wrote)(&self.notes);
        if let Some(room) = travelled.last_room {
            self.last_room = Some(room);
        }
        match travelled.ended {
            Ended::Arrived => Ok(()),
            Ended::Stopped(why) => Err(HuntEnd::Stopped(why)),
            other => {
                self.handle.say(Notice::line(
                    NoticeKind::Warn,
                    format!("Hunt: could not walk to room {}: {other:?}.", to.0),
                ));
                match self.machine.walk_failed(to) {
                    Some(ending) => Err(HuntEnd::Finished(ending)),
                    None => Ok(()),
                }
            }
        }
    }
}

/// Fold one event into a state: a frame is applied; a reconnect or a close
/// ends the behavior.
fn fold_into(state: &mut GameState, event: &Event) -> Result<(), BehaviorError> {
    match event {
        Event::Frame(frame) => {
            state.apply(frame);
            Ok(())
        }
        Event::StateChanged(State::Reconnecting) => Err(BehaviorError::Disconnected),
        Event::StateChanged(State::Closed) => Err(BehaviorError::Dead),
        _ => Ok(()),
    }
}
