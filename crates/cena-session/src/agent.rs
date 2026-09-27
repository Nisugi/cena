//! An agent's control level, and the one door an agent acts through
//! (`plan/35` §3, M7 steps 2 and 3).
//!
//! # The level is the player's
//!
//! > **AUTHOR, 2026-09-24:** *"agent control level = this, this this, this
//! > this this, and it can be set. if an agent tries to send a command they
//! > don't have access to, they get a notice."* And, of whether a level lasts
//! > beyond the run: *"persistent"*.
//!
//! [`Level`] is kept per character in its settings file (the `agent`
//! section, [`Settings`]) and set by the player, with the binary's
//! `;agent level`. Nothing on [`Door`] changes it: an agent never sets its own
//! level.
//!
//! # Four levels now, six in the plan
//!
//! `plan/35` §3 names Off, Observe, Advise, Behaviors, Commands and Takeover.
//! Only the first four have anything to permit yet, so only they exist; each
//! later step adds its level beside the tools it permits. A level with nothing
//! behind it would be one a player could set today and have mean more after
//! an update: a permission given before it existed.
//!
//! # Checked where the level lives
//!
//! An agent reaches a session only through a [`Door`] (`plan/35` §3:
//! "enforced in the session, not in the agent crate"). Every act on it checks
//! the level itself, and `cena-agent` holds no [`SessionHandle`], which
//! `crates/cena-arch-tests/tests/layering.rs` asserts, so no path to an act
//! skips the check. A read is checked with [`Door::may`] by the tool that
//! reads: what an agent reads from is an observer, read-only by construction,
//! so a read changes nothing in the session to guard.
//!
//! # Above its level: a refusal, a notice, and a way to say yes
//!
//! `plan/35` §3. The agent is told which level it needed ([`Refused`]), so it
//! can tell the player "I need Advise for that" instead of guessing. The
//! player is told too, in the character's stream, because a refusal only the
//! agent sees is invisible to the one person who can change the level.
//!
//! An **act** above the level also waits for the player's yes
//! ([`SessionHandle::approve_agent`]): that one act, once, while the
//! connection it was asked on lasts, and for [`APPROVAL_LIFETIME`]. It grants
//! nothing further, and a change of level drops every act still waiting, so
//! lowering and raising the level never brings one back (LAB's rule). A
//! **read** has no yes to give: reading goes on, and the level is its answer.
//! Nor does anything at [`Level::Off`], where an agent has no standing to ask:
//! the player chose that it should do nothing, and a stream of questions is
//! something.
//!
//! A refusal that asks nothing is told to the player at most once every few
//! minutes, with how many there were: an agent polling a character at `off`
//! would otherwise fill its stream.
//!
//! # Each request admitted once (issue #19, point 4)
//!
//! Every act carries the caller's own request id ([`Call`]). The first time,
//! the act is admitted or refused; **the same id again, for the same act, is
//! answered as the first time was, and never done twice**, so a caller whose
//! reply was lost asks again safely. The same id for a different act is
//! refused. A request asked while the first is still being admitted is told
//! so rather than admitted beside it. An act that could not be done, or was
//! refused without asking the player, admitted nothing, and may be asked
//! again under the same id. The last [`KEPT_REQUESTS`] ids are kept, in
//! memory: a restart of Hydra forgets them, as it ends every operation they
//! could have named.

mod denylist;
mod door;

pub use denylist::refused;
pub use door::{Call, Door};

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::time::Instant;

use crate::lifecycle::Generation;
use crate::notice::{Body, Notice, NoticeKind};
use crate::operation::{Control, Report, Table};
use crate::{Event, SessionHandle};

/// What an agent may do with one character (`plan/35` §3). Each includes
/// everything below it.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// Nothing: an agent sees the character's name and this level, and no
    /// more. The default, for a character whose player never chose.
    #[default]
    Off,
    /// Read the character: its state, what happens to it, its records.
    Observe,
    /// Also put a message in front of the player. Nothing reaches the game.
    Advise,
    /// Also start, steer and stop behaviors, through the Hydra commands the
    /// binary allows ([`Door::perform`]). Never a game command of its own.
    Behaviors,
    /// Also send game commands of its own, one line at a time, through the
    /// same queue as the player's typing, and never a line the denylist
    /// refuses ([`Door::command`], [`refused`]).
    Commands,
}

impl Level {
    /// Every level, lowest first.
    pub const ALL: [Self; 5] = [
        Self::Off,
        Self::Observe,
        Self::Advise,
        Self::Behaviors,
        Self::Commands,
    ];

    /// The word the player types and the settings file keeps.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Observe => "observe",
            Self::Advise => "advise",
            Self::Behaviors => "behaviors",
            Self::Commands => "commands",
        }
    }

    /// The level a word names, ignoring case.
    #[must_use]
    pub fn named(word: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|level| level.word().eq_ignore_ascii_case(word.trim()))
    }

    /// What an agent may do at this level, as the player is told it.
    #[must_use]
    pub const fn allows(self) -> &'static str {
        match self {
            Self::Off => "an agent may do nothing with this character",
            Self::Observe => "an agent may read this character, and do nothing else",
            Self::Advise => {
                "an agent may read this character and put a message in front of you; nothing it does reaches the game"
            }
            Self::Behaviors => {
                "an agent may also start, steer and stop go2, hunt, heal, keep and waggle; never a game command of its own"
            }
            Self::Commands => {
                "an agent may also send game commands of its own, one line at a time, but never drop, give, sell or destroy anything"
            }
        }
    }
}

/// The name of [`Settings`]' section in a character's settings file.
pub const SECTION: &str = "agent";

/// The `agent` section of a character's settings (`settings_store`):
///
/// ```json
/// { "agent": { "level": "observe" } }
/// ```
///
/// A level this build does not know is a malformed section, not `off` said
/// quietly: the caller tells the player and keeps [`Level::Off`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// The level the player last chose.
    #[serde(default)]
    pub level: Level,
}

/// How long an act waits for the player's yes.
pub const APPROVAL_LIFETIME: Duration = Duration::from_mins(2);

/// How many acts may wait for the player at once. Past it, an act is refused
/// without asking: an agent cannot bury the player in requests.
pub const MAX_WAITING: usize = 3;

/// The longest message [`Door::tell_player`] carries, in characters.
pub const MAX_TOLD: usize = 2_000;

/// The longest reason an act carries, in characters.
pub const MAX_BECAUSE: usize = 300;

/// How many request ids are remembered ([`Call`]).
pub const KEPT_REQUESTS: usize = 256;

/// Something an agent does that changes what the player sees or what the
/// character does, as against reading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    /// Put a message in front of the player ([`Door::tell_player`]).
    TellPlayer {
        /// The message.
        text: String,
    },
    /// Run a Hydra command an agent may run ([`Door::perform`]).
    Perform {
        /// The command, as the binary keeps it.
        line: String,
    },
    /// Send one line to the game ([`Door::command`]).
    Command {
        /// The line, as it goes.
        line: String,
    },
    /// Steer an operation an agent started ([`Door::control`]).
    Control {
        /// The operation's number.
        operation: u64,
        /// What to do to it.
        control: Control,
    },
}

impl Act {
    /// The level that permits it.
    #[must_use]
    pub const fn needs(&self) -> Level {
        match self {
            Self::TellPlayer { .. } => Level::Advise,
            Self::Perform { .. } | Self::Control { .. } => Level::Behaviors,
            Self::Command { .. } => Level::Commands,
        }
    }

    /// What it is, as the player is asked about it. **Not a message's
    /// words**: showing them in the question would put them in front of the
    /// player at a level that does not allow that. A command is shown
    /// whole: the player approves exactly it.
    fn described(&self) -> String {
        match self {
            Self::TellPlayer { text } => {
                format!("tell you something ({} characters)", text.chars().count())
            }
            Self::Perform { line } => format!("run `{line}`"),
            Self::Command { line } => format!("send `{line}` to the game"),
            Self::Control { operation, control } => {
                format!("{} its operation {operation}", control.word())
            }
        }
    }
}

/// Why an agent was refused by the level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused {
    /// The level it needed.
    pub needed: Level,
    /// The character's level.
    pub level: Level,
    /// Whether the player was asked to let it through.
    pub approval: Approval,
}

/// Whether a refusal asked the player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Approval {
    /// Not asked, and never will be: a read, which the level alone answers;
    /// or anything at [`Level::Off`], where an agent has no standing to ask.
    NotAsked,
    /// The player was asked: `;agent approve <id>` lets it through, once.
    Asked {
        /// The request's number.
        id: u64,
        /// How long it still waits.
        expires_in: Duration,
    },
    /// The player was not asked: [`MAX_WAITING`] requests already wait.
    Full,
}

/// An act waiting for the player, as `;agent` lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending {
    /// Its number, for `;agent approve` and `;agent deny`.
    pub id: u64,
    /// What it would do.
    pub act: String,
    /// The agent's reason.
    pub because: String,
    /// How long it still waits.
    pub expires_in: Duration,
}

/// What an act that was let through did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Admitted {
    /// The message is in front of the player.
    Told,
    /// An operation was started or steered: as it stands now.
    Operation(Report),
}

/// Why an act was not done.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Denied {
    /// The level does not allow it.
    Level(Refused),
    /// It cannot be done as asked, whatever the level: why, in words.
    Invalid(String),
}

/// What changed about the agent's side of a session, published as
/// [`Event::Agent`] so an agent learns it in order with everything else.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// The player set the level.
    Level(Level),
    /// The player answered a request: let through, or not (denied, lapsed,
    /// could not be done, or dropped when the level changed).
    Answered {
        /// The request's number.
        id: u64,
        /// Whether the act was done.
        approved: bool,
    },
    /// An operation started, was steered, got on, or ended. Boxed: a report
    /// is large beside every other event.
    Operation(Box<Report>),
}

/// One session's level, the acts waiting on the player, the requests
/// admitted and the operations started, shared by every clone of its handle.
#[derive(Clone, Debug, Default)]
pub(crate) struct Access(Arc<Mutex<Inner>>);

#[derive(Debug, Default)]
struct Inner {
    level: Level,
    waiting: Vec<Waiting>,
    last_id: u64,
    /// The last time the player was told of a refusal that asked them
    /// nothing, and how many there were since.
    told_of_reads: Option<Instant>,
    reads_untold: u32,
    /// Every request id admitted, oldest first.
    requests: VecDeque<Admission>,
    operations: Table,
}

#[derive(Debug)]
struct Waiting {
    id: u64,
    act: Act,
    because: String,
    generation: Generation,
    expires: Instant,
}

/// A request id, the act it asked for, and what became of it.
#[derive(Debug)]
struct Admission {
    request: String,
    act: Act,
    answer: Answer,
}

/// What became of an admitted request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Answer {
    /// Being done now.
    InFlight,
    /// The message was put in front of the player.
    Told,
    /// This operation was started or steered.
    Operation(u64),
    /// The player was asked, under this number.
    Asked(u64),
    /// The player was asked under this number, and it was not done.
    NotApproved(u64),
}

impl Answer {
    fn of(admitted: &Admitted) -> Self {
        match admitted {
            Admitted::Told => Self::Told,
            Admitted::Operation(report) => Self::Operation(report.id),
        }
    }
}

/// A request id looked up.
enum Recalled {
    /// Never seen.
    New,
    /// Seen, for a different act.
    Other,
    /// Seen, for this act.
    Answer(Answer),
}

impl Access {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The session's operations, for `crate::operation`.
    pub(crate) fn with_operations<T>(&self, f: impl FnOnce(&mut Table) -> T) -> T {
        f(&mut self.lock().operations)
    }
}

impl Inner {
    /// Take out every request past its time, and mark it not done; the
    /// caller tells the agent.
    fn lapse(&mut self, now: Instant) -> Vec<u64> {
        let mut lapsed = Vec::new();
        self.waiting.retain(|w| {
            let live = w.expires > now;
            if !live {
                lapsed.push(w.id);
            }
            live
        });
        for &id in &lapsed {
            self.settle_asked(id, Answer::NotApproved(id));
        }
        lapsed
    }

    fn recall(&self, request: &str, act: &Act) -> Recalled {
        match self.requests.iter().find(|a| a.request == request) {
            None => Recalled::New,
            Some(seen) if seen.act != *act => Recalled::Other,
            Some(seen) => Recalled::Answer(seen.answer),
        }
    }

    fn remember(&mut self, request: &str, act: &Act, answer: Answer) {
        self.requests.push_back(Admission {
            request: request.to_owned(),
            act: act.clone(),
            answer,
        });
        while self.requests.len() > KEPT_REQUESTS {
            self.requests.pop_front();
        }
    }

    fn settle(&mut self, request: &str, answer: Answer) {
        if let Some(seen) = self.requests.iter_mut().find(|a| a.request == request) {
            seen.answer = answer;
        }
    }

    fn forget(&mut self, request: &str) {
        self.requests.retain(|a| a.request != request);
    }

    /// The request that asked the player under `id` now has `answer`.
    fn settle_asked(&mut self, id: u64, answer: Answer) {
        if let Some(seen) = self
            .requests
            .iter_mut()
            .find(|a| a.answer == Answer::Asked(id))
        {
            seen.answer = answer;
        }
    }
}

/// The player's side of the agent: the binary's `;agent`.
impl SessionHandle {
    /// This character's agent level.
    #[must_use]
    pub fn agent_level(&self) -> Level {
        self.agent.lock().level
    }

    /// Set this character's agent level. Every act still waiting is dropped
    /// and answered as not approved, so a level lowered and raised again
    /// never brings one back. Saving it is the caller's.
    ///
    /// **It stops nothing that runs.** An operation an agent started goes on
    /// as if the player had started it; below [`Level::Behaviors`] the agent
    /// can no longer steer it, and the player's own stop ends it.
    pub fn set_agent_level(&self, level: Level) {
        let dropped = {
            let mut inner = self.agent.lock();
            if inner.level == level {
                return;
            }
            inner.level = level;
            inner.told_of_reads = None;
            inner.reads_untold = 0;
            let dropped = std::mem::take(&mut inner.waiting);
            for waiting in &dropped {
                inner.settle_asked(waiting.id, Answer::NotApproved(waiting.id));
            }
            dropped
        };
        for waiting in dropped {
            self.decided(Change::Answered {
                id: waiting.id,
                approved: false,
            });
        }
        self.decided(Change::Level(level));
    }

    /// The acts waiting for the player, oldest first.
    #[must_use]
    pub fn agent_requests(&self) -> Vec<Pending> {
        let now = Instant::now();
        let (lapsed, pending) = {
            let mut inner = self.agent.lock();
            let lapsed = inner.lapse(now);
            let pending = inner
                .waiting
                .iter()
                .map(|w| Pending {
                    id: w.id,
                    act: w.act.described(),
                    because: w.because.clone(),
                    expires_in: w.expires - now,
                })
                .collect();
            (lapsed, pending)
        };
        self.answer_lapsed(lapsed);
        pending
    }

    /// Let request `id` through: its act is done now, once.
    ///
    /// # Errors
    ///
    /// Nothing waits under `id`, or it lapsed, or the connection it was asked
    /// on has ended, or the act can no longer be done (an operation that has
    /// since ended). Nothing was done; the words say which.
    pub fn approve_agent(&self, id: u64) -> Result<(), String> {
        let waiting = self.take_request(id)?;
        let done = if waiting.generation == self.generation() {
            self.perform(&waiting.act, &waiting.because, Some(id))
        } else {
            Err("it was asked on a connection that has since ended".to_owned())
        };
        let answer = done.as_ref().map_or(Answer::NotApproved(id), Answer::of);
        self.agent.lock().settle_asked(id, answer);
        self.decided(Change::Answered {
            id,
            approved: done.is_ok(),
        });
        done.map(drop)
            .map_err(|why| format!("request {id} was not done: {why}"))
    }

    /// Refuse request `id`.
    ///
    /// # Errors
    ///
    /// Nothing waits under `id`, or it already lapsed.
    pub fn deny_agent(&self, id: u64) -> Result<(), String> {
        self.take_request(id)?;
        self.agent.lock().settle_asked(id, Answer::NotApproved(id));
        self.decided(Change::Answered {
            id,
            approved: false,
        });
        Ok(())
    }

    fn take_request(&self, id: u64) -> Result<Waiting, String> {
        let now = Instant::now();
        let (lapsed, found) = {
            let mut inner = self.agent.lock();
            let lapsed = inner.lapse(now);
            let found = inner
                .waiting
                .iter()
                .position(|w| w.id == id)
                .map(|at| inner.waiting.remove(at));
            (lapsed, found)
        };
        let was_lapsed = lapsed.contains(&id);
        self.answer_lapsed(lapsed);
        found.ok_or_else(|| {
            if was_lapsed {
                format!("request {id} lapsed unanswered; nothing was done")
            } else {
                format!("no request {id} is waiting")
            }
        })
    }

    fn answer_lapsed(&self, lapsed: Vec<u64>) {
        for id in lapsed {
            self.decided(Change::Answered {
                id,
                approved: false,
            });
        }
    }

    fn decided(&self, change: Change) {
        self.publish(Event::Agent(change));
    }

    /// Do an act the level, or the player, allowed, as the approval numbered
    /// `approval` when it was one. The player is told what the agent did and
    /// why: the audit trail, in their stream and their log.
    fn perform(&self, act: &Act, because: &str, approval: Option<u64>) -> Result<Admitted, String> {
        match act {
            Act::TellPlayer { text } => {
                let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
                if let Some(first) = lines.first_mut() {
                    *first = format!("Agent: {first}");
                }
                lines.push(format!("  (because: {because})"));
                self.say(Notice {
                    kind: NoticeKind::Info,
                    body: Body::Lines(lines),
                });
                Ok(Admitted::Told)
            }
            Act::Perform { line } => {
                let report = crate::operation::start(self, line, approval)?;
                self.say(Notice::line(
                    NoticeKind::Info,
                    format!(
                        "Agent: `{line}` (operation {}), because: {because}",
                        report.id
                    ),
                ));
                Ok(Admitted::Operation(report))
            }
            Act::Command { line } => {
                let report = crate::operation::send(self, line, approval);
                self.say(Notice::line(
                    NoticeKind::Info,
                    format!(
                        "Agent: sent `{line}` (operation {}), because: {because}",
                        report.id
                    ),
                ));
                Ok(Admitted::Operation(report))
            }
            Act::Control { operation, control } => {
                let report = crate::operation::steer(self, *operation, *control)?;
                self.say(Notice::line(
                    NoticeKind::Info,
                    format!(
                        "Agent: {} operation {operation} (`{}`), because: {because}",
                        control.word(),
                        report.line
                    ),
                ));
                Ok(Admitted::Operation(report))
            }
        }
    }

    /// The player's command symbol, for the words that name a command.
    fn symbol(&self) -> char {
        self.command_symbol()
            .unwrap_or(crate::command::claimant::DEFAULT_SYMBOL)
    }
}
