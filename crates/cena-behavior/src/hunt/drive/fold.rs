//! The hunt driver's fold: each event from the session, applied to the
//! hunt's own state, with what the hunt hears and watches for taken on the
//! way, and a dropped connection marked and waited out (`plan/30` §7: "a
//! reconnect mid-hunt keeps the hunt").
//!
//! Moved down out of `drive.rs`, which had passed its cap (`plan/05` Rule 4.4:
//! a split parent's growth goes into its submodules).

use std::time::Duration;

use cena_session::{ChunkLine, CommandId, Event, Frame, GameState, Notice, NoticeKind, State};
use tokio::sync::broadcast::error::{RecvError, TryRecvError};

use super::{BEAT, Driver, HuntEnd};
use crate::error::BehaviorError;
use crate::travel::TravelNotes;

/// The main window's line being read: its text so far, and whether a
/// person said it, known at its first run with text.
#[derive(Debug, Default)]
pub(super) struct Reading {
    pub(super) text: String,
    spoken: Option<bool>,
}

impl Reading {
    /// Done with the line: the next run begins another.
    pub(super) fn clear(&mut self) {
        self.text.clear();
        self.spoken = None;
    }
}

impl<F: FnMut() -> CommandId, W: FnMut(&TravelNotes), L: FnMut(&[String])> Driver<'_, F, W, L> {
    /// Fold what has come, and take the state afresh if any was lost: a lag
    /// seen here was caught up only at the next turn, and an errand decided
    /// from a state with holes until then (the crate review of 2026-10-01,
    /// BE-A-10).
    pub(super) async fn drain(&mut self) -> Result<(), HuntEnd> {
        loop {
            match self.events.try_recv() {
                Ok(event) => self.fold(&event).map_err(HuntEnd::Stopped)?,
                Err(TryRecvError::Lagged(_)) => {}
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Closed) => return Err(HuntEnd::Stopped(BehaviorError::Dead)),
            }
        }
        self.caught_up().await.map_err(HuntEnd::Stopped)
    }

    /// Fold events for up to `for_`, or until stopped.
    pub(super) async fn hold(&mut self, for_: Duration) -> Result<(), HuntEnd> {
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
                Err(RecvError::Lagged(_)) => self.caught_up().await.map_err(HuntEnd::Stopped)?,
                Err(RecvError::Closed) => return Err(HuntEnd::Stopped(BehaviorError::Dead)),
            }
        }
    }

    /// Wait while the connection is away, before a send or a walk inside a
    /// turn: an errand's next step waits for the session to be back rather
    /// than run its steps against a connection that is not there. Waiting
    /// out a reconnect is progress, so it beats.
    pub(super) async fn back(&mut self) -> Result<(), HuntEnd> {
        while self.down {
            self.heartbeat.beat();
            self.hold(BEAT).await?;
        }
        Ok(())
    }

    pub(super) fn fold(&mut self, event: &Event) -> Result<(), BehaviorError> {
        // Every event folded is progress, inside a turn too: a walk, a
        // selling round or a heal is one turn, and may run past the
        // watchdog's limit (the crate review of 2026-10-01, BE-A-4).
        self.heartbeat.beat();
        match event {
            Event::StateChanged(State::Reconnecting) => self.link_lost(),
            Event::StateChanged(State::Ready) if self.down => {
                self.down = false;
                self.handle.say(Notice::line(
                    NoticeKind::Info,
                    "Hunt: reconnected; hunting on.".to_owned(),
                ));
            }
            _ => {}
        }
        if let Event::Frame(frame) = event
            && let Frame::Text(text) = &**frame
        {
            if text.stream.is_empty() {
                // **A line a person said is not the game's** (the crate
                // review of 2026-10-01, BE-A-8): `Pukk says, "...but it has
                // no effect"` ended the hunt. It is decided by the line's
                // first run with text, as the model decides it for the chunk
                // (`ChunkLine::is_spoken`), and a said line is neither the
                // transcript a reply is read from nor what the hunt hears.
                if self.said.spoken.is_none() && !text.content.trim().is_empty() {
                    self.said.spoken = Some(ChunkLine::is_speech(&text.style));
                }
                let game = self.said.spoken != Some(true);
                if game {
                    self.transcript.push_str(&text.content);
                    self.said.text.push_str(&text.content);
                }
                // A run carries no line end of its own; without one the
                // transcript was a single line, and a reply was read only
                // when it was the first thing in it.
                if text.ends_line {
                    if game {
                        self.transcript.push('\n');
                        let said = std::mem::take(&mut self.said.text);
                        // The whole line: a run ends at every link, so a
                        // creature's name and its swing were never in one run
                        // (the crate review of 2026-10-01, BE-A-1).
                        let now = self.state.game_time_now();
                        self.machine.heard(&said, now);
                    }
                    self.said.clear();
                }
            }
            // The monitor reads whole lines, from every window.
            self.line.push_str(&text.content);
            if text.ends_line {
                self.machine.watched(&self.line);
                self.line.clear();
                for alert in self.machine.take_alerts() {
                    self.handle.say(Notice::line(
                        NoticeKind::Warn,
                        format!("Hunt alert: {alert}"),
                    ));
                }
            }
        }
        fold_into(&mut self.state, event)
    }

    /// Events were lost ([`Heard::behind`](crate::travel::Heard::behind)):
    /// the state is taken afresh from the session before anything more is
    /// decided, and a stream that cannot be stops the hunt rather than hunt
    /// on a state with holes in it (the crate review of 2026-09-28, R1).
    pub(super) async fn caught_up(&mut self) -> Result<(), BehaviorError> {
        if !self.events.behind() {
            return Ok(());
        }
        let snapshot = self.events.again().await.ok_or(BehaviorError::FellBehind)?;
        self.state = snapshot.state;
        self.lifecycle = snapshot.lifecycle;
        self.generation = snapshot.generation;
        self.cursor = snapshot.cursor;
        self.transcript.clear();
        self.line.clear();
        self.said.clear();
        match snapshot.lifecycle {
            State::Ready => self.down = false,
            State::Reconnecting => self.link_lost(),
            State::Closed => return Err(BehaviorError::Dead),
            _ => {}
        }
        self.handle.say(Notice::line(
            NoticeKind::Info,
            "Hunt: fell behind the game; took its state afresh.".to_owned(),
        ));
        Ok(())
    }

    /// The connection dropped: what it made stale is forgotten, and the
    /// hunt waits for the session to be ready again (`plan/30` §7: "a
    /// reconnect mid-hunt keeps the hunt").
    pub(super) fn link_lost(&mut self) {
        if self.down {
            return;
        }
        self.down = true;
        self.said.clear();
        self.state.invalidate_for_reconnect();
        self.machine.link_lost();
        self.party_link_lost();
        self.handle.say(Notice::line(
            NoticeKind::Info,
            "Hunt: the connection dropped; waiting for it to come back.".to_owned(),
        ));
    }
}

/// Fold one event into a state: a frame is applied; a reconnect invalidates
/// what a reconnect invalidates and is waited out; a close ends the behavior;
/// a flag a trigger set is set here too, so a step's `flag` guard reads it
/// (`plan/45` Stage 2).
pub(super) fn fold_into(state: &mut GameState, event: &Event) -> Result<(), BehaviorError> {
    match event {
        Event::Frame(frame) => {
            state.apply(frame);
            Ok(())
        }
        // A drop is waited out, holding the authority (SE-4 (c)); the
        // driver marks it. Invalidating twice is harmless.
        Event::StateChanged(State::Reconnecting) => {
            state.invalidate_for_reconnect();
            Ok(())
        }
        Event::StateChanged(State::Closed) => Err(BehaviorError::Dead),
        Event::Flag(change) => {
            state.flags.apply(change);
            Ok(())
        }
        _ => Ok(()),
    }
}
