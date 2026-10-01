//! An agent holding the character: the Takeover level (`plan/35` §4, M7
//! step 5).
//!
//! # What a takeover is
//!
//! **An operation that holds the command authority.** Taking over stops what
//! runs -- the binary's behaviors, through its performer's halt, and then
//! the session's own preempt, which takes the authority from a holder that
//! does not let go (`plan/12` §4.3) -- and claims the authority under the
//! agent's [`TOKEN`]. While it holds, no behavior can start, and what the
//! agent sends is sent as the holder's ([`Origin::Agent`] with the token),
//! queued behind the player's typing, which still interleaves (author,
//! 2026-09-24).
//!
//! **Nothing resumes by itself afterwards** (`plan/35` §4): the agent starts
//! what it wants next, once it has let go.
//!
//! # How it ends, and which ending it is (issue #19, point 5)
//!
//! Each ending is said, and they are not the same:
//!
//! | Reason | Who | |
//! |---|---|---|
//! | `released` | the agent, `control stop` | graceful: the agent gave it back |
//! | `revoked` | the player, `;agent stop` | hard: the authority is taken back at once, and whatever the agent still had queued is refused, never sent |
//! | `level_lowered` | the player, below Takeover | as `revoked` |
//! | `disconnected`, `session_ended` | the connection | the authority is let go |
//! | `dead` | the game | the character died |
//! | `owner_idle` | nobody | the agent touched the character not once in [`OWNER_IDLE`]: a model that hangs does not keep the character |
//! | `authority_held` | -- | it never began: the authority could not be taken |
//!
//! **A takeover, or any run the agent started, that ends in death, a
//! disconnect or the watchdog firing drops the level to Observe**
//! (`plan/35` §4), until the player raises it again.
//!
//! **One at a time.** A second takeover while one holds is refused, naming
//! it: two callers cannot both hold the character.
//!
//! [`Origin::Agent`]: crate::Origin::Agent

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use super::Level;
use crate::lifecycle::State;
use crate::notice::{Notice, NoticeKind};
use crate::operation::{Control, Ended, Report, Started, Work};
use crate::queue::AuthorityToken;
use crate::{Event, Frame, SessionHandle};

/// The authority an agent holds the character under. Clear of the binary's
/// behaviors' (travel 2, hunt 3, the batches 4 and 5).
pub const TOKEN: AuthorityToken = AuthorityToken(7);

/// How long a takeover lasts with the agent not touching the character --
/// reading or acting through its door -- before it lets go.
pub const OWNER_IDLE: Duration = Duration::from_mins(5);

/// How often a takeover looks at whether its agent is still there.
const LOOK_EVERY: Duration = Duration::from_secs(5);

/// A takeover under way: its operation, and how it is ended.
#[derive(Clone, Debug)]
pub(crate) struct Holding {
    pub(crate) operation: u64,
    ending: Ending,
}

/// How a takeover is ended, and why: the first reason given wins.
#[derive(Clone, Debug, Default)]
struct Ending {
    token: CancellationToken,
    why: Arc<OnceLock<&'static str>>,
}

impl Ending {
    fn end(&self, why: &'static str) {
        let _ = self.why.set(why);
        self.token.cancel();
    }
}

impl Holding {
    /// End it now, for `why`, and take the authority back at once: whatever
    /// the agent still has queued is refused, not sent.
    pub(crate) fn take_back(&self, handle: &SessionHandle, why: &'static str) {
        handle.authority.release(TOKEN);
        self.ending.end(why);
    }
}

/// Whether a run that ended for `reason` ended badly (`plan/35` §4): the
/// level drops.
pub(crate) fn ended_badly(reason: &str) -> bool {
    matches!(reason, "dead" | "disconnected" | "wedged" | "trouble")
}

/// A run the agent started ended badly: down to Observe, and the player told.
pub(crate) fn drop_level(handle: &SessionHandle, report: &Report, reason: &str) {
    if handle.agent_level() <= Level::Observe {
        return;
    }
    handle.set_agent_level(Level::Observe);
    let symbol = handle.symbol();
    handle.say(Notice::line(
        NoticeKind::Warn,
        format!(
            "The agent's `{}` (operation {}) ended {reason}: its level is now observe, until {symbol}agent level raises it.",
            report.line, report.id
        ),
    ));
}

/// The token an agent's line is sent under: the agent's while it holds the
/// character, none while it does not.
pub(crate) fn holding(handle: &SessionHandle) -> Option<AuthorityToken> {
    handle.agent.lock().takeover.as_ref().map(|_| TOKEN)
}

/// Take the character over, as operation `approval`'s yes or an allowed act.
pub(crate) fn take_over(handle: &SessionHandle, approval: Option<u64>) -> Result<Report, String> {
    let ending = Ending::default();
    // Checked and taken under one lock: two takeovers at once both passed
    // the check before either was recorded, and the one that lost the claim
    // cleared the winner's record, leaving the authority held with nothing
    // a lowered level could take back (the crate review of 2026-10-01,
    // SE-B-3). Its operation is named once it has one, just below.
    {
        let mut inner = handle.agent.lock();
        if let Some(held) = &inner.takeover {
            return Err(format!(
                "the agent already holds this character (operation {})",
                held.operation
            ));
        }
        inner.takeover = Some(Holding {
            operation: 0,
            ending: ending.clone(),
        });
    }
    let holder = handle.clone();
    let report = crate::operation::begin(handle, "take over", approval, true, |reporter| {
        if let Some(held) = holder
            .agent
            .lock()
            .takeover
            .as_mut()
            .filter(|held| held.ending.token == ending.token)
        {
            held.operation = reporter.id();
        }
        let released = ending.clone();
        Started {
            ended: Box::pin(hold(holder.clone(), ending, reporter)),
            steer: Arc::new(move |control: Control| {
                if control == Control::Stop {
                    released.end("released");
                    Ok(())
                } else {
                    Err(format!(
                        "a takeover cannot be told to {}; `stop` gives the character back",
                        control.word()
                    ))
                }
            }),
            token: Some(TOKEN),
        }
    });
    Ok(report)
}

/// Stop what runs, take the authority, hold it until the takeover ends,
/// and give it back.
async fn hold(
    handle: SessionHandle,
    ending: Ending,
    reporter: crate::operation::Reporter,
) -> Ended {
    // Before anything is stopped, so a death or a drop is not missed.
    let mut events = handle.events();
    if let Some(performer) = handle.performer() {
        (performer.halt)();
    }
    let _ = handle.preempt(&CancellationToken::new()).await;
    let ended = if handle.claim(TOKEN).await.is_err() {
        Ended::plainly(Work::NoOpportunity, "authority_held")
    } else {
        reporter.progress(crate::operation::Progress {
            doing: "holding the character".to_owned(),
            ..crate::operation::Progress::default()
        });
        let why = held(&handle, &ending, &mut events).await;
        handle.release(TOKEN);
        let work = match why {
            "released" => Work::Completed,
            "dead" => Work::Failed,
            _ => Work::Interrupted,
        };
        Ended::plainly(work, why)
    };
    let mut inner = handle.agent.lock();
    if inner
        .takeover
        .as_ref()
        .is_some_and(|held| held.ending.token == ending.token)
    {
        inner.takeover = None;
    }
    ended
}

/// Hold until something ends the takeover, and say what.
async fn held(
    handle: &SessionHandle,
    ending: &Ending,
    events: &mut tokio::sync::broadcast::Receiver<Event>,
) -> &'static str {
    use tokio::sync::broadcast::error::RecvError;
    let mut look = tokio::time::interval(LOOK_EVERY);
    loop {
        tokio::select! {
            () = ending.token.cancelled() => {
                return ending.why.get().copied().unwrap_or("released");
            }
            event = events.recv() => match event {
                Ok(Event::StateChanged(State::Reconnecting)) => return "disconnected",
                Ok(Event::StateChanged(State::Closed)) | Err(RecvError::Closed) => {
                    return "session_ended";
                }
                Ok(Event::Frame(frame))
                    if matches!(&*frame, Frame::StatusIndicator { id, active: true } if id == "IconDEAD") =>
                {
                    return "dead";
                }
                _ => {}
            },
            _ = look.tick() => {
                let seen = handle.agent.lock().last_seen;
                if seen.is_none_or(|seen| Instant::now().duration_since(seen) >= OWNER_IDLE) {
                    return "owner_idle";
                }
            }
        }
    }
}

/// The player's stop: everything the agent is doing.
impl SessionHandle {
    /// Stop every operation the agent has running and take back the
    /// character if it holds it: the player's explicit stop, which outranks
    /// the agent (`plan/35` §4). The level stays as it is. Answers how many
    /// operations were told to stop, and whether a takeover was revoked.
    #[must_use]
    pub fn stop_agent(&self) -> (usize, bool) {
        let running: Vec<u64> = self
            .agent
            .with_operations(|table| table.reports())
            .into_iter()
            .filter(|report| report.lifecycle != crate::operation::Lifecycle::Ended)
            .map(|report| report.id)
            .collect();
        let held = self.agent.lock().takeover.clone();
        if let Some(held) = &held {
            held.take_back(self, "revoked");
        }
        let stopped = running
            .into_iter()
            .filter(|id| held.as_ref().is_none_or(|h| h.operation != *id))
            .filter(|id| crate::operation::steer(self, *id, Control::Stop).is_ok())
            .count();
        (stopped, held.is_some())
    }

    /// The operation holding the character for the agent, if one does.
    #[must_use]
    pub fn agent_holds(&self) -> Option<u64> {
        self.agent
            .lock()
            .takeover
            .as_ref()
            .map(|held| held.operation)
    }
}

/// A change of level, as a takeover sees it: below Takeover, it is taken
/// back at once.
pub(crate) fn level_changed(handle: &SessionHandle, held: Option<Holding>, level: Level) {
    if level < Level::Takeover
        && let Some(held) = held
    {
        held.take_back(handle, "level_lowered");
    }
}
