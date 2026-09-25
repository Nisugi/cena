//! The batch driver: what has the socket and the clock.
//!
//! A batch sends as the hunt sends (`plan/30` §3, `hunt/drive.rs`), because
//! there is one way to send: **settle** roundtime and cast roundtime -- and
//! here a stun and a web too, which the gate would refuse anyway -- then
//! **send** through [`Gate::Act`], and take the next prompt as the end of the
//! round trip. What a batch adds is Lich's `fput` answer to a refusal
//! (`reference/lich-5/lib/global_defs.rb:1556-1565`): a gate refusal is waited
//! out and the line sent again, rather than skipped, since a list of commands
//! has no next tick to decide afresh; and a `...wait N` the game says (the
//! model's clock was behind) is waited out and the line resent, up to
//! [`RESENDS`] times.
//!
//! # A Hydra command in the middle
//!
//! A line like `;sc 401` is run by whatever runs that command, which claims
//! the authority for itself. So the batch **gives the authority back** first,
//! runs it, keeps folding its own stream and beating its heartbeat while it
//! waits, and **takes the authority again** after; if something else has it
//! by then, the batch stops rather than queue behind it (`plan/12` §4.2).
//!
//! # Stopping (`plan/12` §4.3)
//!
//! Every await is raced against the stop token; a stopped batch sends nothing
//! more. The watchdog is the desk's.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use cena_session::{
    AuthorityToken, ChunkLine, CommandId, Event, Frame, GameState, Gate, MoveFeedback, Notice,
    NoticeKind, Origin, Outcome, Refusal, SessionHandle, State, movement,
};
use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use super::line::{Hydra, Line, Ran};
use crate::error::BehaviorError;
use crate::travel::Heard;
use crate::watchdog::Heartbeat;

/// How long a sent line may wait for its prompt: the hunt's.
pub const SEND_DEADLINE: Duration = Duration::from_secs(8);
/// How long roundtime and the rest are waited out before a line is tried
/// anyway, and the gate asked: the hunt's.
pub const SETTLE_CAP: Duration = Duration::from_secs(15);
/// How often a wait turns with nothing to fold.
pub const BEAT: Duration = Duration::from_millis(250);
/// How long one line may go on being refused by the gate before the batch
/// gives it up.
pub const REFUSED_CAP: Duration = Duration::from_mins(1);
/// How many times one line is resent for the game's `...wait N`.
pub const RESENDS: u32 = 5;

/// Why a batch ended before its last line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Halt {
    /// The session decided: stopped, disconnected, dead, or the authority
    /// held by something else.
    Stopped(BehaviorError),
    /// The batch gave up, and why, in words for the player.
    Failed(String),
}

impl From<BehaviorError> for Halt {
    fn from(why: BehaviorError) -> Halt {
        Halt::Stopped(why)
    }
}

/// One batch's hands on its session.
pub(super) struct Driver<'a> {
    pub(super) handle: &'a SessionHandle,
    pub(super) cancel: &'a CancellationToken,
    pub(super) token: AuthorityToken,
    pub(super) ids: &'a AtomicU64,
    pub(super) state: GameState,
    pub(super) events: Heard,
    pub(super) heartbeat: &'a Heartbeat,
    pub(super) hydra: &'a Hydra,
    /// `Multi` or `Foreach`, for what is said.
    pub(super) name: &'static str,
    /// The main window's lines since the last command went out: its answer.
    pub(super) answer: Vec<ChunkLine>,
    /// How many of the open chunk's lines have been heard.
    heard: usize,
}

impl<'a> Driver<'a> {
    /// A driver on `state` and its stream, holding the authority as `token`.
    #[allow(clippy::too_many_arguments)] // one of each thing a driver has
    pub(super) fn new(
        handle: &'a SessionHandle,
        cancel: &'a CancellationToken,
        token: AuthorityToken,
        ids: &'a AtomicU64,
        joined: (GameState, Heard),
        heartbeat: &'a Heartbeat,
        hydra: &'a Hydra,
        name: &'static str,
    ) -> Self {
        Driver {
            handle,
            cancel,
            token,
            ids,
            state: joined.0,
            events: joined.1,
            heartbeat,
            hydra,
            name,
            answer: Vec::new(),
            heard: 0,
        }
    }

    /// Say something, as this batch.
    pub(super) fn say(&self, kind: NoticeKind, text: &str) {
        self.handle
            .say(Notice::line(kind, format!("{}: {text}", self.name)));
    }

    /// Do one line.
    pub(super) async fn line(&mut self, line: &Line) -> Result<(), Halt> {
        self.heartbeat.beat();
        match line {
            Line::Send(text) => self.send(text).await,
            Line::Hydra(command) => self.hydra(command).await,
            Line::WaitRt => {
                self.wait_until(|s| s.state.in_roundtime() != Some(true))
                    .await
            }
            Line::WaitCastRt => {
                self.wait_until(|s| s.state.in_casttime() != Some(true))
                    .await
            }
            Line::Sleep(time) => self.hold(*time).await,
            Line::Echo(text) => {
                self.say(NoticeKind::Info, text);
                Ok(())
            }
            Line::WaitFor(phrase) => {
                let phrase = phrase.to_lowercase();
                self.wait_to_hear(|line| line.to_lowercase().contains(&phrase))
                    .await
            }
            Line::WaitRe(source) => {
                let pattern = regex::Regex::new(source)
                    .map_err(|e| Halt::Failed(format!("waitre {source}: {e}")))?;
                self.wait_to_hear(|line| pattern.is_match(line)).await
            }
            Line::WaitVital(pool, least) => {
                self.wait_until(|s| pool.current(&s.state).is_some_and(|have| have >= *least))
                    .await
            }
        }
    }

    /// Settle, send through the gate, take the prompt; wait out a refusal and
    /// a `...wait N` and send again.
    async fn send(&mut self, text: &str) -> Result<(), Halt> {
        let began = Instant::now();
        let mut resends = 0;
        loop {
            self.settle().await?;
            self.heartbeat.beat();
            self.answer.clear();
            let id = CommandId(self.ids.fetch_add(1, Ordering::Relaxed));
            let outcome = tokio::select! {
                biased;
                () = self.cancel.cancelled() => return Err(BehaviorError::Cancelled.into()),
                outcome = self.handle.send_gated(
                    id,
                    text,
                    Origin::Behavior(self.token),
                    SEND_DEADLINE,
                    |frame| matches!(frame, Frame::Prompt { .. }),
                    Gate::Act { target: None },
                ) => outcome,
            };
            if let Some(gone) = BehaviorError::from_outcome(&outcome) {
                return Err(gone.into());
            }
            self.drain()?;
            if let Outcome::Refused(refusal) = outcome {
                if refusal == Refusal::Permanent {
                    return Err(Halt::Failed(format!(
                        "`{text}` cannot be sent: the character is dead, or this batch lost the session."
                    )));
                }
                if began.elapsed() >= REFUSED_CAP {
                    return Err(Halt::Failed(format!(
                        "`{text}` was refused for {}s ({refusal:?}); the rest was not sent.",
                        REFUSED_CAP.as_secs()
                    )));
                }
                self.hold(BEAT).await?;
                continue;
            }
            let again =
                self.answer
                    .iter()
                    .find_map(|line| match movement::classify(line.text().trim()) {
                        Some(MoveFeedback::Wait(seconds)) => {
                            Some(Duration::from_secs(seconds.into()))
                        }
                        Some(MoveFeedback::TypeAhead) => Some(Duration::from_secs(1)),
                        _ => None,
                    });
            match again {
                Some(wait) if resends < RESENDS => {
                    resends += 1;
                    self.hold(wait).await?;
                }
                _ => return Ok(()),
            }
        }
    }

    /// Send `text` quietly -- a look Hydra makes for itself, kept out of the
    /// story as the character sync's commands are -- and hand back what the
    /// game answered.
    pub(super) async fn look(&mut self, text: &str) -> Result<Vec<ChunkLine>, Halt> {
        let began = Instant::now();
        loop {
            self.heartbeat.beat();
            self.answer.clear();
            let id = CommandId(self.ids.fetch_add(1, Ordering::Relaxed));
            let outcome = tokio::select! {
                biased;
                () = self.cancel.cancelled() => return Err(BehaviorError::Cancelled.into()),
                outcome = self.handle.send_quietly(
                    id,
                    text,
                    Origin::Behavior(self.token),
                    SEND_DEADLINE,
                    |frame| matches!(frame, Frame::Prompt { .. }),
                ) => outcome,
            };
            if let Some(gone) = BehaviorError::from_outcome(&outcome) {
                return Err(gone.into());
            }
            self.drain()?;
            match outcome {
                Outcome::Refused(Refusal::Permanent) => {
                    return Err(Halt::Failed(format!(
                        "`{text}` cannot be sent: this batch lost the session."
                    )));
                }
                Outcome::Refused(refusal) if began.elapsed() >= REFUSED_CAP => {
                    return Err(Halt::Failed(format!(
                        "`{text}` was refused for {}s ({refusal:?}).",
                        REFUSED_CAP.as_secs()
                    )));
                }
                Outcome::Refused(_) => self.hold(BEAT).await?,
                _ => return Ok(std::mem::take(&mut self.answer)),
            }
        }
    }

    /// Run a Hydra command and wait for it to be over, with the authority
    /// given back meanwhile (module docs).
    async fn hydra(&mut self, command: &str) -> Result<(), Halt> {
        self.settle().await?;
        self.handle.release(self.token);
        let mut run = (self.hydra)(command);
        let ran = loop {
            self.heartbeat.beat();
            let event = tokio::select! {
                biased;
                () = self.cancel.cancelled() => return Err(BehaviorError::Cancelled.into()),
                ran = &mut run => break ran,
                event = self.events.recv() => event,
                () = tokio::time::sleep(BEAT) => continue,
            };
            match event {
                Ok(event) => self.fold(&event)?,
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => return Err(BehaviorError::Dead.into()),
            }
        };
        if ran == Ran::Unknown {
            let symbol = self
                .handle
                .command_symbol()
                .unwrap_or(cena_session::command::claimant::DEFAULT_SYMBOL);
            return Err(Halt::Failed(format!(
                "I do not know {symbol}{command}, so the rest was not sent."
            )));
        }
        if self.handle.claim(self.token).await.is_err() {
            return Err(BehaviorError::AuthorityHeld.into());
        }
        self.drain()?;
        Ok(())
    }

    /// Wait until `done` says so, folding the stream.
    async fn wait_until(&mut self, done: impl Fn(&Self) -> bool) -> Result<(), Halt> {
        self.drain()?;
        while !done(self) {
            self.hold(BEAT).await?;
        }
        Ok(())
    }

    /// Wait for a main-window line, from now on, that `heard` takes.
    async fn wait_to_hear(&mut self, heard: impl Fn(&str) -> bool) -> Result<(), Halt> {
        self.drain()?;
        self.answer.clear();
        loop {
            if self.answer.iter().any(|line| heard(&line.text())) {
                return Ok(());
            }
            self.answer.clear();
            self.hold(BEAT).await?;
        }
    }

    /// Wait out roundtime, cast roundtime, a stun and a web, up to
    /// [`SETTLE_CAP`].
    async fn settle(&mut self) -> Result<(), Halt> {
        self.drain()?;
        let cap = Instant::now() + SETTLE_CAP;
        while busy(&self.state) && Instant::now() < cap {
            self.hold(BEAT).await?;
        }
        Ok(())
    }

    /// Fold the stream for `time`, or until stopped.
    pub(super) async fn hold(&mut self, time: Duration) -> Result<(), Halt> {
        let deadline = Instant::now() + time;
        loop {
            self.heartbeat.beat();
            let event = tokio::select! {
                biased;
                () = self.cancel.cancelled() => return Err(BehaviorError::Cancelled.into()),
                event = self.events.recv() => event,
                () = tokio::time::sleep_until(deadline) => return Ok(()),
            };
            match event {
                Ok(event) => self.fold(&event)?,
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => return Err(BehaviorError::Dead.into()),
            }
        }
    }

    /// Fold whatever the stream already holds.
    pub(super) fn drain(&mut self) -> Result<(), Halt> {
        loop {
            match self.events.try_recv() {
                Ok(event) => self.fold(&event)?,
                Err(TryRecvError::Lagged(_)) => {}
                Err(TryRecvError::Empty) => return Ok(()),
                Err(TryRecvError::Closed) => return Err(BehaviorError::Dead.into()),
            }
        }
    }

    /// One event: a frame is folded, and a main-window line heard; a
    /// reconnect or a close ends the batch.
    fn fold(&mut self, event: &Event) -> Result<(), Halt> {
        match event {
            Event::Frame(frame) => {
                self.state.apply(frame);
                match &**frame {
                    Frame::Prompt { .. } => self.heard = 0,
                    Frame::Text(_) => self.hear(),
                    _ => {}
                }
                Ok(())
            }
            Event::StateChanged(State::Reconnecting) => Err(BehaviorError::Disconnected.into()),
            Event::StateChanged(State::Closed) => Err(BehaviorError::Dead.into()),
            _ => Ok(()),
        }
    }

    /// Take the lines the model has finished since the last heard, as
    /// travel's driver does (`travel/drive.rs`, `hear`): the open chunk is
    /// the main window's lines since the prompt.
    fn hear(&mut self) {
        let chunk = self.state.open_chunk();
        let lines = chunk.lines();
        let total = chunk.dropped() + lines.len();
        let fresh = total.saturating_sub(self.heard).min(lines.len());
        self.answer
            .extend(lines[lines.len() - fresh..].iter().cloned());
        self.heard = total;
    }
}

/// Whether the character cannot act yet: roundtime, cast roundtime, stunned
/// or webbed, as far as the model knows.
fn busy(state: &GameState) -> bool {
    let status = state.status.known();
    state.in_roundtime() == Some(true)
        || state.in_casttime() == Some(true)
        || status.stunned() == Some(true)
        || status.webbed() == Some(true)
}
