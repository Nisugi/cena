//! An agent's control level, and the one door an agent acts through
//! (`plan/35` §3, M7 step 2).
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
//! # Three levels now, six in the plan
//!
//! `plan/35` §3 names Off, Observe, Advise, Behaviors, Commands and Takeover.
//! Only the first three have anything to permit yet, so only they exist; each
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

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::time::Instant;

use crate::lifecycle::Generation;
use crate::notice::{Body, Notice, NoticeKind};
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
}

impl Level {
    /// Every level, lowest first.
    pub const ALL: [Self; 3] = [Self::Off, Self::Observe, Self::Advise];

    /// The word the player types and the settings file keeps.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Observe => "observe",
            Self::Advise => "advise",
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

/// How long after telling the player of a refused read before telling them of
/// more: an agent polling every few seconds would otherwise fill the stream.
const RETOLD_AFTER: Duration = Duration::from_mins(5);

/// Something an agent does that changes what the player sees or what the
/// character does, as against reading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    /// Put a message in front of the player ([`Door::tell_player`]).
    TellPlayer {
        /// The message.
        text: String,
    },
}

impl Act {
    /// The level that permits it.
    #[must_use]
    pub const fn needs(&self) -> Level {
        match self {
            Self::TellPlayer { .. } => Level::Advise,
        }
    }

    /// What it is, as the player is asked about it. **Not the message
    /// itself**: showing it in the question would put it in front of the
    /// player at a level that does not allow that.
    fn described(&self) -> String {
        match self {
            Self::TellPlayer { text } => {
                format!("tell you something ({} characters)", text.chars().count())
            }
        }
    }
}

/// Why an agent was refused.
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
        /// How long it waits.
        expires_in: Duration,
    },
    /// The player was not asked: [`MAX_WAITING`] requests already wait.
    Full,
}

/// An act waiting for the player, as `;agent` lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    /// Its number, for `;agent approve` and `;agent deny`.
    pub id: u64,
    /// What it would do.
    pub act: String,
    /// The agent's reason.
    pub because: String,
    /// How long it still waits.
    pub expires_in: Duration,
}

/// What the player decided about the agent, published as
/// [`Event::Agent`] so an agent learns it in order with everything else.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// The level changed.
    Level(Level),
    /// A request was answered: let through, or not (denied, lapsed, or
    /// dropped when the level changed).
    Answered {
        /// The request's number.
        id: u64,
        /// Whether the act was done.
        approved: bool,
    },
}

/// One session's level and the acts waiting on the player, shared by every
/// clone of its handle.
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
}

#[derive(Debug)]
struct Waiting {
    id: u64,
    act: Act,
    because: String,
    generation: Generation,
    expires: Instant,
}

impl Access {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Inner {
    /// Take out every request past its time, for the caller to answer.
    fn lapse(&mut self, now: Instant) -> Vec<u64> {
        let mut lapsed = Vec::new();
        self.waiting.retain(|w| {
            let live = w.expires > now;
            if !live {
                lapsed.push(w.id);
            }
            live
        });
        lapsed
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
    pub fn set_agent_level(&self, level: Level) {
        let dropped = {
            let mut inner = self.agent.lock();
            if inner.level == level {
                return;
            }
            inner.level = level;
            inner.told_of_reads = None;
            inner.reads_untold = 0;
            std::mem::take(&mut inner.waiting)
        };
        for waiting in dropped {
            self.decided(Decision::Answered {
                id: waiting.id,
                approved: false,
            });
        }
        self.decided(Decision::Level(level));
    }

    /// The acts waiting for the player, oldest first.
    #[must_use]
    pub fn agent_requests(&self) -> Vec<Request> {
        let now = Instant::now();
        let (lapsed, requests) = {
            let mut inner = self.agent.lock();
            let lapsed = inner.lapse(now);
            let requests = inner
                .waiting
                .iter()
                .map(|w| Request {
                    id: w.id,
                    act: w.act.described(),
                    because: w.because.clone(),
                    expires_in: w.expires - now,
                })
                .collect();
            (lapsed, requests)
        };
        self.answer_lapsed(lapsed);
        requests
    }

    /// Let request `id` through: its act is done now, once.
    ///
    /// # Errors
    ///
    /// Nothing waits under `id`, or it lapsed, or the connection it was asked
    /// on has ended. Nothing was done; the words say which.
    pub fn approve_agent(&self, id: u64) -> Result<(), String> {
        let waiting = self.take_request(id)?;
        if waiting.generation != self.generation() {
            self.decided(Decision::Answered {
                id,
                approved: false,
            });
            return Err(format!(
                "request {id} was asked on a connection that has since ended; nothing was done"
            ));
        }
        self.perform(&waiting.act, &waiting.because);
        self.decided(Decision::Answered { id, approved: true });
        Ok(())
    }

    /// Refuse request `id`.
    ///
    /// # Errors
    ///
    /// Nothing waits under `id`, or it already lapsed.
    pub fn deny_agent(&self, id: u64) -> Result<(), String> {
        self.take_request(id)?;
        self.decided(Decision::Answered {
            id,
            approved: false,
        });
        Ok(())
    }

    /// The door an agent acts on this session through.
    #[must_use]
    pub fn agent_door(&self) -> Door {
        Door {
            handle: self.clone(),
        }
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
            self.decided(Decision::Answered {
                id,
                approved: false,
            });
        }
    }

    fn decided(&self, decision: Decision) {
        self.publish(Event::Agent(decision));
    }

    /// Do an act the level, or the player, allowed.
    fn perform(&self, act: &Act, because: &str) {
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
            }
        }
    }

    /// The player's command symbol, for the words that name a command.
    fn symbol(&self) -> char {
        self.command_symbol()
            .unwrap_or(crate::command::claimant::DEFAULT_SYMBOL)
    }
}

/// The only way an agent acts on a session: each act checks the level here.
///
/// Built by [`SessionHandle::agent_door`], for `cena-agent`, which holds this
/// and never the handle.
#[derive(Clone, Debug)]
pub struct Door {
    handle: SessionHandle,
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

    /// Put `text` in front of the player, with the agent's reason. Needs
    /// [`Level::Advise`]; below it, and above [`Level::Off`], the player is
    /// asked.
    ///
    /// Control characters are taken out, and both are cut to [`MAX_TOLD`]
    /// and [`MAX_BECAUSE`]: the words reach a terminal as they are.
    ///
    /// # Errors
    ///
    /// [`Refused`], naming the approval the player was asked for.
    pub fn tell_player(&self, text: &str, because: &str) -> Result<(), Refused> {
        let text = clean(text, MAX_TOLD, true);
        let because = clean(because, MAX_BECAUSE, false);
        self.act(&Act::TellPlayer { text }, &because)
    }

    fn act(&self, act: &Act, because: &str) -> Result<(), Refused> {
        let needed = act.needs();
        let now = Instant::now();
        let generation = self.handle.generation();
        let (level, asked, lapsed) = {
            let mut inner = self.handle.agent.lock();
            let level = inner.level;
            if level >= needed {
                drop(inner);
                self.handle.perform(act, because);
                return Ok(());
            }
            if level == Level::Off {
                drop(inner);
                return Err(self.refuse_unasked(needed, level));
            }
            let lapsed = inner.lapse(now);
            let asked = (inner.waiting.len() < MAX_WAITING).then(|| {
                inner.last_id += 1;
                let id = inner.last_id;
                inner.waiting.push(Waiting {
                    id,
                    act: act.clone(),
                    because: because.to_owned(),
                    generation,
                    expires: now + APPROVAL_LIFETIME,
                });
                id
            });
            (level, asked, lapsed)
        };
        self.handle.answer_lapsed(lapsed);
        let approval = match asked {
            Some(id) => {
                self.ask(id, act, because, level);
                Approval::Asked {
                    id,
                    expires_in: APPROVAL_LIFETIME,
                }
            }
            None => Approval::Full,
        };
        Err(Refused {
            needed,
            level,
            approval,
        })
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
