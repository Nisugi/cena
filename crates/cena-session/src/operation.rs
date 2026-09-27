//! An operation: a Hydra command started for someone who watches it to its
//! end (`plan/35` §6, M7 step 3).
//!
//! An agent at [`Level::Behaviors`](crate::agent::Level::Behaviors) runs a
//! behavior with [`Door::perform`](crate::agent::Door::perform) and gets back
//! an operation's number, which it reads and steers by. **A ticket, not a
//! reply** (LAB's): a dropped connection is never a reason to send again,
//! because the operation goes on and can be read by its number.
//!
//! # Hydra does not know the commands; the binary does
//!
//! Which Hydra commands an agent may run, and how each is started, belong to
//! whoever knows the commands (`command/claimant.rs` says the same of the
//! command line). The binary registers a [`Performer`] once the behaviors
//! exist; this module keeps the operations it starts, and nothing else.
//!
//! # What an ending says, and what it does not (issue #19, point 3)
//!
//! [`Ended`] keeps three things apart that are easy to run together: what the
//! **work** came to, as the behavior judged it ([`Work`], with its own word for
//! why); what it **left** undone that someone may need to put right (an item
//! still stored, a stance not restored); and, beside it in [`Report`], whether
//! the **authority** it claimed was given back, as the session sees it. A hunt
//! that failed and then walked home cleanly is a failed hunt with nothing
//! left. **No effect is claimed**: a hunt's `completed` is the machine's
//! verdict that its stopping rule was met, never a count of kills; the
//! combat recorder has what was seen, attributed as it attributes it.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};

use crate::agent::Change;
use crate::queue::AuthorityToken;
use crate::{Event, SessionHandle};

/// How many ended operations are kept to be read; running ones are always
/// kept.
pub const KEPT_ENDED: usize = 32;

/// Where an operation is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifecycle {
    /// Under way.
    Running,
    /// Held: it defends itself and starts nothing, until resumed or stopped.
    Held,
    /// Walking to where it ends, as asked: a hunt to its resting room.
    Retreating,
    /// Asked to stop, and not yet stopped.
    Stopping,
    /// Over: [`Report::ended`] says how.
    Ended,
}

impl Lifecycle {
    /// Its word.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Held => "held",
            Self::Retreating => "retreating",
            Self::Stopping => "stopping",
            Self::Ended => "ended",
        }
    }
}

/// What the work came to, as the behavior judged it. Not what cleaning up
/// after it came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Work {
    /// It did what it was started to do: arrived, or met its stopping rule.
    Completed,
    /// It could not: the way was shut, the character died, a weapon was lost.
    Failed,
    /// It was stopped before it could finish: by a stop, a disconnect, the
    /// group ending.
    Interrupted,
    /// It never began: there was nothing to do, or the session was busy.
    NoOpportunity,
    /// Nobody can say.
    Unknown,
}

impl Work {
    /// Its word.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
            Self::NoOpportunity => "no_opportunity",
            Self::Unknown => "unknown",
        }
    }
}

/// How an operation ended, as the behavior that ran it reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ended {
    /// What the work came to.
    pub work: Work,
    /// The behavior's own word for why: `arrived`, `rested`, `dead`,
    /// `stopped`, `not_started`.
    pub reason: String,
    /// What it left undone that someone may need to put right, in words.
    /// Empty when nothing was.
    pub left: Vec<String>,
}

impl Ended {
    /// An ending with nothing left undone.
    #[must_use]
    pub fn plainly(work: Work, reason: &str) -> Self {
        Self {
            work,
            reason: reason.to_owned(),
            left: Vec::new(),
        }
    }
}

/// A way to steer an operation under way (`plan/35` §4; the author chose
/// what hold and retreat mean, 2026-09-27).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// Stop it, as the player's own stop does.
    Stop,
    /// Defend, and start nothing: what keeps the character alive still acts;
    /// nothing new is begun until [`Self::Resume`].
    Hold,
    /// Go on after a hold.
    Resume,
    /// Walk to where it ends -- a hunt's resting room -- and end there.
    Retreat,
}

impl Control {
    /// Every control.
    pub const ALL: [Self; 4] = [Self::Stop, Self::Hold, Self::Resume, Self::Retreat];

    /// Its word.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Stop => "stop",
            Self::Hold => "hold",
            Self::Resume => "resume",
            Self::Retreat => "retreat",
        }
    }

    /// Where an operation stands once this control is applied, from `now`.
    const fn leaves(self, now: Lifecycle) -> Lifecycle {
        match (self, now) {
            (_, Lifecycle::Ended) => Lifecycle::Ended,
            (Self::Stop, _) | (_, Lifecycle::Stopping) => Lifecycle::Stopping,
            (Self::Retreat, _) | (_, Lifecycle::Retreating) => Lifecycle::Retreating,
            (Self::Hold, _) => Lifecycle::Held,
            (Self::Resume, _) => Lifecycle::Running,
        }
    }

    /// The control a word names, ignoring case.
    #[must_use]
    pub fn named(word: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|control| control.word().eq_ignore_ascii_case(word.trim()))
    }
}

/// How one operation is steered: the behavior's own controls for that run
/// alone, never for whatever runs next.
pub type Steer = Arc<dyn Fn(Control) -> Result<(), String> + Send + Sync>;

/// What a [`Performer`] started.
pub struct Started {
    /// Resolves with how it ended. Spawned by the session; it must not need
    /// anything but the runtime.
    pub ended: Pin<Box<dyn Future<Output = Ended> + Send>>,
    /// How it is steered.
    pub steer: Steer,
    /// The authority it claims, so that its release can be checked when it
    /// ends; `None` for what claims none.
    pub token: Option<AuthorityToken>,
}

/// Whether an agent may run a line: the line as it is kept and compared, or
/// why not.
pub type Allows = Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>;

/// Start a line an [`Allows`] took.
pub type Start = Arc<dyn Fn(&str) -> Started + Send + Sync>;

/// Who runs the Hydra commands an agent may perform: the binary, which knows
/// them ([`SessionHandle::set_performer`]).
#[derive(Clone)]
pub struct Performer {
    /// What an agent may run, in words, for an agent to be told.
    pub allowed: String,
    /// Whether an agent may run a line.
    pub allows: Allows,
    /// Start a line [`Self::allows`] took.
    pub start: Start,
}

impl std::fmt::Debug for Performer {
    /// By hand: two closures, nothing to print.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Performer").finish_non_exhaustive()
    }
}

/// Where a session's [`Performer`] is kept, once the binary has registered
/// one: shared by every clone of its handle, as the command desk is.
pub(crate) type Slot = Arc<OnceLock<Performer>>;

/// Whether the authority an operation claimed was given back, as the session
/// sees it once the operation has ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Released {
    /// Nobody holds it under the operation's token.
    Yes,
    /// The operation's token holds it again: this run's release not seen, or
    /// a later run of the same behavior's claim. Nobody can tell which.
    Unknown,
    /// The operation claimed none.
    NotClaimed,
}

impl Released {
    /// Its word.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Yes => "released",
            Self::Unknown => "unknown",
            Self::NotClaimed => "not_claimed",
        }
    }
}

/// An operation, as it stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    /// Its number.
    pub id: u64,
    /// The Hydra command it runs, as kept.
    pub line: String,
    /// Where it is in its life.
    pub lifecycle: Lifecycle,
    /// The approval it was started on, when the player said yes to it.
    pub approval: Option<u64>,
    /// How it ended, once it has.
    pub ended: Option<Ended>,
    /// Whether its authority was given back, once it has ended.
    pub authority: Option<Released>,
}

/// One session's operations: every running one, and the last
/// [`KEPT_ENDED`] that ended.
#[derive(Default)]
pub(crate) struct Table {
    last_id: u64,
    kept: VecDeque<Kept>,
}

struct Kept {
    report: Report,
    steer: Steer,
}

impl std::fmt::Debug for Table {
    /// By hand: a [`Steer`] is a closure.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let reports: Vec<&Report> = self.kept.iter().map(|k| &k.report).collect();
        f.debug_struct("Table")
            .field("last_id", &self.last_id)
            .field("kept", &reports)
            .finish()
    }
}

impl Table {
    /// The operation numbered `id`, as it stands.
    pub(crate) fn report(&self, id: u64) -> Option<Report> {
        self.kept
            .iter()
            .find(|k| k.report.id == id)
            .map(|k| k.report.clone())
    }

    /// Every operation kept, oldest first.
    pub(crate) fn reports(&self) -> Vec<Report> {
        self.kept.iter().map(|k| k.report.clone()).collect()
    }

    fn add(&mut self, line: &str, approval: Option<u64>, steer: Steer) -> Report {
        self.last_id += 1;
        let report = Report {
            id: self.last_id,
            line: line.to_owned(),
            lifecycle: Lifecycle::Running,
            approval,
            ended: None,
            authority: None,
        };
        self.kept.push_back(Kept {
            report: report.clone(),
            steer,
        });
        report
    }

    fn end(&mut self, id: u64, ended: Ended, authority: Released) -> Option<Report> {
        let kept = self.kept.iter_mut().find(|k| k.report.id == id)?;
        kept.report.lifecycle = Lifecycle::Ended;
        kept.report.ended = Some(ended);
        kept.report.authority = Some(authority);
        let report = kept.report.clone();
        let over = |k: &Kept| k.report.lifecycle == Lifecycle::Ended;
        while self.kept.iter().filter(|k| over(k)).count() > KEPT_ENDED {
            if let Some(at) = self.kept.iter().position(over) {
                self.kept.remove(at);
            }
        }
        Some(report)
    }
}

/// Start `line` with the session's performer, as operation `approval`'s yes
/// or an allowed act; the operation is watched to its end here.
pub(crate) fn start(
    handle: &SessionHandle,
    line: &str,
    approval: Option<u64>,
) -> Result<Report, String> {
    let performer = handle
        .performer()
        .ok_or("Hydra cannot run behaviors for an agent yet: the character is still logging in")?;
    let started = (performer.start)(line);
    let report = handle
        .agent
        .with_operations(|table| table.add(line, approval, started.steer));
    handle.publish(Event::Agent(Change::Operation(report.clone())));
    let (watcher, id, token, ended) = (handle.clone(), report.id, started.token, started.ended);
    tokio::spawn(async move {
        let ended = ended.await;
        let authority = match token {
            None => Released::NotClaimed,
            Some(token) if watcher.holder() == Some(token) => Released::Unknown,
            Some(_) => Released::Yes,
        };
        let report = watcher
            .agent
            .with_operations(|table| table.end(id, ended, authority));
        if let Some(report) = report {
            watcher.publish(Event::Agent(Change::Operation(report)));
        }
    });
    Ok(report)
}

/// Steer operation `id` with `control`.
pub(crate) fn steer(handle: &SessionHandle, id: u64, control: Control) -> Result<Report, String> {
    let steer = handle.agent.with_operations(|table| {
        let kept = table.kept.iter().find(|k| k.report.id == id);
        match kept {
            None => Err(format!("there is no operation {id}")),
            Some(k) if k.report.lifecycle == Lifecycle::Ended => {
                Err(format!("operation {id} has already ended"))
            }
            Some(k) => Ok(Arc::clone(&k.steer)),
        }
    })?;
    steer(control)?;
    let report = handle.agent.with_operations(|table| {
        let kept = table.kept.iter_mut().find(|k| k.report.id == id)?;
        kept.report.lifecycle = control.leaves(kept.report.lifecycle);
        Some(kept.report.clone())
    });
    let report = report.ok_or_else(|| format!("operation {id} is no longer kept"))?;
    handle.publish(Event::Agent(Change::Operation(report.clone())));
    Ok(report)
}

/// The binary's side: who runs what an agent performs.
impl SessionHandle {
    /// Register who runs the Hydra commands an agent may perform. Once per
    /// session: a second call is ignored and answers `false`.
    #[must_use]
    pub fn set_performer(&self, performer: Performer) -> bool {
        self.performer.set(performer).is_ok()
    }

    /// The registered performer, once there is one.
    pub(crate) fn performer(&self) -> Option<Performer> {
        self.performer.get().cloned()
    }
}
