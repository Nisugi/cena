//! The door: the only way an agent acts on a session, each act checked
//! against the level here (`plan/35` §3).
//!
//! Moved down out of `agent.rs` when step 3's operations and request ids took
//! that file past its cap (`plan/05` Rule 4.1: move code down, do not raise
//! the cap). The parent keeps what the level is and the player's side of it;
//! this is the agent's side.

use tokio::time::Instant;

use super::{
    APPROVAL_LIFETIME, Act, Admitted, Answer, Approval, Denied, Level, MAX_BECAUSE, MAX_TOLD,
    MAX_WAITING, Recalled, Refused, Waiting,
};
use crate::SessionHandle;
use crate::lifecycle::Generation;
use crate::notice::{Body, Notice, NoticeKind};
use crate::operation::{Control, Lifecycle, Report};

/// How long after telling the player of a refusal that asked them nothing
/// before telling them of more: an agent polling every few seconds would
/// otherwise fill the stream.
const RETOLD_AFTER: std::time::Duration = std::time::Duration::from_mins(5);

/// Who asks for an act, so that it is admitted once (issue #19, point 4; the
/// parent module's docs).
#[derive(Clone, Copy, Debug)]
pub struct Call<'a> {
    /// The caller's own id for this request: the same id again, for the same
    /// act, is answered as the first time and never done twice.
    pub request: &'a str,
    /// The connection the caller believes it acts on; a stale one is refused.
    /// `None` for an act that is not bound to a connection.
    pub generation: Option<Generation>,
}

/// The only way an agent acts on a session: each act checks the level here.
///
/// Built by [`SessionHandle::agent_door`], for `cena-agent`, which holds this
/// and never the handle.
#[derive(Clone, Debug)]
pub struct Door {
    handle: SessionHandle,
}

impl SessionHandle {
    /// The door an agent acts on this session through.
    #[must_use]
    pub fn agent_door(&self) -> Door {
        Door {
            handle: self.clone(),
        }
    }
}

impl Door {
    /// The character's level now.
    #[must_use]
    pub fn level(&self) -> Level {
        self.handle.agent_level()
    }

    /// Whether the level allows a read that needs `needed`.
    ///
    /// # Errors
    ///
    /// [`Refused`], with [`Approval::NotAsked`]; the player is told, at most
    /// once every few minutes however often the agent asks.
    pub fn may(&self, needed: Level) -> Result<(), Refused> {
        let level = self.level();
        if level >= needed {
            return Ok(());
        }
        Err(self.refuse_unasked(needed, level))
    }

    /// The operation numbered `id`, as it stands. A read: the caller checks
    /// [`Self::may`].
    #[must_use]
    pub fn operation(&self, id: u64) -> Option<Report> {
        self.handle.agent.with_operations(|table| table.report(id))
    }

    /// What an agent may run here, in words; `None` until the binary has
    /// said.
    #[must_use]
    pub fn performs(&self) -> Option<String> {
        self.handle.performer().map(|performer| performer.allowed)
    }

    /// Every operation kept, oldest first. A read, as [`Self::operation`].
    #[must_use]
    pub fn operations(&self) -> Vec<Report> {
        self.handle.agent.with_operations(|table| table.reports())
    }

    /// Put `text` in front of the player, with the agent's reason. Needs
    /// [`Level::Advise`]; below it, and above [`Level::Off`], the player is
    /// asked.
    ///
    /// Control characters are taken out, and both are cut to [`MAX_TOLD`]
    /// and [`MAX_BECAUSE`]: the words reach a terminal as they are.
    ///
    /// # Errors
    ///
    /// [`Denied`]: the level, naming any approval the player was asked for;
    /// or the request id already used for another act.
    pub fn tell_player(
        &self,
        text: &str,
        because: &str,
        call: Call<'_>,
    ) -> Result<Admitted, Denied> {
        let text = clean(text, MAX_TOLD, true);
        self.act(Act::TellPlayer { text }, because, call)
    }

    /// Run a Hydra command, as an operation: one the binary allows an agent
    /// ([`crate::operation::Performer`]). Needs [`Level::Behaviors`].
    ///
    /// # Errors
    ///
    /// [`Denied`]: the level; or a command an agent may not run, a stale
    /// generation, a request id used for another act.
    pub fn perform(&self, line: &str, because: &str, call: Call<'_>) -> Result<Admitted, Denied> {
        self.act(
            Act::Perform {
                line: line.to_owned(),
            },
            because,
            call,
        )
    }

    /// Send `line` to the game, as an operation whose result is the game's
    /// answer. Needs [`Level::Commands`]. A line the denylist refuses is
    /// refused at every level, and never put to the player to approve
    /// ([`super::refused`]).
    ///
    /// # Errors
    ///
    /// [`Denied`]: the level; or a denied line, a stale generation, a
    /// request id used for another act.
    pub fn command(&self, line: &str, because: &str, call: Call<'_>) -> Result<Admitted, Denied> {
        self.act(
            Act::Command {
                line: line.to_owned(),
            },
            because,
            call,
        )
    }

    /// Steer operation `operation`. Needs [`Level::Behaviors`].
    ///
    /// **Admission is not application**: the report says the operation is
    /// stopping, and a later one that it has ended.
    ///
    /// # Errors
    ///
    /// As [`Self::perform`], or an operation that is not kept or has ended.
    pub fn control(
        &self,
        operation: u64,
        control: Control,
        because: &str,
        call: Call<'_>,
    ) -> Result<Admitted, Denied> {
        self.act(Act::Control { operation, control }, because, call)
    }

    /// Decide under one lock what becomes of `act`, then do it with the lock
    /// let go: a notice, an operation's start and its report all take locks
    /// of their own, and the agent's is one of them.
    fn act(&self, act: Act, because: &str, call: Call<'_>) -> Result<Admitted, Denied> {
        let handle = &self.handle;
        let because = clean(because, MAX_BECAUSE, false);
        if let Some(expected) = call.generation {
            let now = handle.generation();
            if expected != now {
                return Err(Denied::Invalid(format!(
                    "that was for connection {}; the character is on connection {} now",
                    expected.0, now.0
                )));
            }
        }
        let act = self.canonical(act)?;
        let needed = act.needs();
        let now = Instant::now();
        let (next, level, lapsed) = {
            let mut inner = handle.agent.lock();
            let lapsed = inner.lapse(now);
            let level = inner.level;
            let next = match inner.recall(call.request, &act) {
                Recalled::Other => Next::Other,
                Recalled::Answer(answer) => Next::Answered(answer),
                Recalled::New => match stale(&inner.operations, &act) {
                    Some(why) => Next::Stale(why),
                    None if level >= needed => {
                        inner.remember(call.request, &act, Answer::InFlight);
                        Next::Do
                    }
                    None if level == Level::Off => Next::Unasked,
                    None => Next::Ask((inner.waiting.len() < MAX_WAITING).then(|| {
                        inner.last_id += 1;
                        let id = inner.last_id;
                        inner.waiting.push(Waiting {
                            id,
                            act: act.clone(),
                            because: because.clone(),
                            generation: handle.generation(),
                            expires: now + APPROVAL_LIFETIME,
                        });
                        inner.remember(call.request, &act, Answer::Asked(id));
                        id
                    })),
                },
            };
            (next, level, lapsed)
        };
        handle.answer_lapsed(lapsed);
        match next {
            Next::Other => Err(Denied::Invalid(format!(
                "request id {:?} was used for a different act",
                call.request
            ))),
            Next::Answered(answer) => self.answered(call.request, &act, answer),
            Next::Stale(why) => Err(Denied::Invalid(why)),
            Next::Unasked => Err(Denied::Level(self.refuse_unasked(needed, level))),
            Next::Do => {
                let done = handle.perform(&act, &because, None);
                let mut inner = handle.agent.lock();
                match &done {
                    Ok(admitted) => inner.settle(call.request, Answer::of(admitted)),
                    // Not done, so not admitted: the same id may be asked again.
                    Err(_) => inner.forget(call.request),
                }
                done.map_err(Denied::Invalid)
            }
            Next::Ask(asked) => {
                let approval = match asked {
                    Some(id) => {
                        self.ask(id, &act, &because, level);
                        Approval::Asked {
                            id,
                            expires_in: APPROVAL_LIFETIME,
                        }
                    }
                    None => Approval::Full,
                };
                Err(Denied::Level(Refused {
                    needed,
                    level,
                    approval,
                }))
            }
        }
    }

    /// The act in the form it is kept and compared in: a command as the
    /// binary's performer keeps it, or why an agent may not run it.
    fn canonical(&self, act: Act) -> Result<Act, Denied> {
        if let Act::Command { line } = &act {
            if let Some(why) = super::refused(line, self.handle.symbol()) {
                return Err(Denied::Invalid(format!("never sent: {why}")));
            }
            let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
            if line.is_empty() {
                return Err(Denied::Invalid("the line is empty".to_owned()));
            }
            return Ok(Act::Command { line });
        }
        let Act::Perform { line } = act else {
            return Ok(act);
        };
        let performer = self.handle.performer().ok_or_else(|| {
            Denied::Invalid(
                "Hydra cannot run behaviors for an agent yet: the character is still logging in"
                    .to_owned(),
            )
        })?;
        let line = (performer.allows)(&line).map_err(Denied::Invalid)?;
        Ok(Act::Perform { line })
    }

    /// A request id seen before, answered as it was the first time.
    fn answered(&self, request: &str, act: &Act, answer: Answer) -> Result<Admitted, Denied> {
        match answer {
            Answer::InFlight => Err(Denied::Invalid(format!(
                "request id {request:?} is being admitted now; ask again in a moment"
            ))),
            Answer::Told => Ok(Admitted::Told),
            Answer::Operation(id) => self
                .operation(id)
                .map(Admitted::Operation)
                .ok_or_else(|| Denied::Invalid(format!("operation {id} is no longer kept"))),
            Answer::Asked(id) => {
                let now = Instant::now();
                let inner = self.handle.agent.lock();
                let level = inner.level;
                let left = inner
                    .waiting
                    .iter()
                    .find(|w| w.id == id)
                    .map(|w| w.expires - now);
                drop(inner);
                match left {
                    Some(expires_in) => Err(Denied::Level(Refused {
                        needed: act.needs(),
                        level,
                        approval: Approval::Asked { id, expires_in },
                    })),
                    None => Err(Denied::Invalid(format!(
                        "request {id} is no longer waiting"
                    ))),
                }
            }
            Answer::NotApproved(id) => Err(Denied::Invalid(format!(
                "the player did not let request {id} through; nothing was done"
            ))),
        }
    }

    /// Refuse without asking the player to approve, and tell them, at most
    /// once every [`RETOLD_AFTER`] with how many were refused since.
    fn refuse_unasked(&self, needed: Level, level: Level) -> Refused {
        let now = Instant::now();
        let tell = {
            let mut inner = self.handle.agent.lock();
            inner.reads_untold += 1;
            let due = inner
                .told_of_reads
                .is_none_or(|told| now.duration_since(told) >= RETOLD_AFTER);
            due.then(|| {
                inner.told_of_reads = Some(now);
                std::mem::take(&mut inner.reads_untold)
            })
        };
        if let Some(times) = tell {
            let symbol = self.handle.symbol();
            let tried = if times == 1 {
                "An agent was refused this character".to_owned()
            } else {
                format!("An agent was refused this character {times} times")
            };
            self.handle.say(Notice::line(
                NoticeKind::Warn,
                format!(
                    "{tried}: its agent level is {}. {symbol}agent level {} lets it; {symbol}agent help says more.",
                    level.word(),
                    needed.word()
                ),
            ));
        }
        Refused {
            needed,
            level,
            approval: Approval::NotAsked,
        }
    }

    /// Ask the player about request `id`.
    fn ask(&self, id: u64, act: &Act, because: &str, level: Level) {
        let symbol = self.handle.symbol();
        self.handle.say(Notice {
            kind: NoticeKind::Warn,
            body: Body::Lines(vec![
                format!("An agent asks to {}, because: {because}", act.described()),
                format!(
                    "That needs the {} level; this character's is {}.",
                    act.needs().word(),
                    level.word()
                ),
                format!(
                    "{symbol}agent approve {id} lets it, once. {symbol}agent deny {id} refuses. Unanswered, it lapses in {} minutes.",
                    APPROVAL_LIFETIME.as_secs() / 60
                ),
            ]),
        });
    }
}

/// What [`Door::act`] decided, under the lock, to do once it is let go.
enum Next {
    /// The request id was used for a different act.
    Other,
    /// The request id was seen: answer as the first time.
    Answered(Answer),
    /// The act can no longer be done at all.
    Stale(String),
    /// Refused without asking: the level is off.
    Unasked,
    /// Allowed: do it.
    Do,
    /// Above the level: the player was asked under this number, or not.
    Ask(Option<u64>),
}

/// Why a new act can no longer be done at all: an operation that is not
/// kept, or has ended.
fn stale(operations: &crate::operation::Table, act: &Act) -> Option<String> {
    let Act::Control { operation, .. } = act else {
        return None;
    };
    match operations.report(*operation) {
        None => Some(format!("there is no operation {operation}")),
        Some(report) if report.lifecycle == Lifecycle::Ended => {
            Some(format!("operation {operation} has already ended"))
        }
        Some(_) => None,
    }
}

/// `text` without control characters (a line break kept where `lines` says),
/// cut to `most` characters.
fn clean(text: &str, most: usize, lines: bool) -> String {
    text.chars()
        .filter(|&c| !c.is_control() || (lines && c == '\n'))
        .take(most)
        .collect::<String>()
        .trim()
        .to_owned()
}
