//! A script's way in (`plan/46`, M7b): what a script runner may do to the
//! character it runs scripts for, through [`Door`], which `cena-agent`'s
//! scripts listener holds and never the handle.
//!
//! **A script is the player's own program**, run out of process in its own
//! language as Lich runs it (`plan/38`, `plan/46`): the player started it,
//! and it may do what a Lich script does. So it has no agent level and no
//! denylist -- eloot sells (`plan/46` §8). The script level the author asked
//! for (§10, question 8) decides what a script may do to a character that
//! did not start it, and comes with the first request that names another
//! character; today a runner acts on its own.
//!
//! Three acts:
//!
//! - [`Door::listen`]: publish each line as the game sent it
//!   ([`Event::Heard`](crate::Event::Heard)), while a runner reads it.
//! - [`Door::send`]: a line as if typed, Lich's `put`. Hydra's own command
//!   when it starts with the command symbol, as a trigger's line is
//!   (`plan/45` Stage 5), so nothing marked as Hydra's reaches the game --
//!   **save the commands only the player's own typing may give** (`agent`,
//!   `trigger`, `lich`, `to`, `all`; the binary's `commands.rs`), which a
//!   script's line is refused, as the player's consent is not a script's to
//!   give (the crate review of 2026-10-01, BI-B-2);
//!   otherwise to the game at once as [`Origin::Script`], with no roundtime
//!   gate and no queue, as `Game.puts` writes straight to the socket
//!   (`inventory/13` §1.2), and never counted as the player being there.
//! - [`Door::say`]: tell the player something, Lich's `respond`.
//! - [`Door::perform`]: start one of Hydra's built-in behaviors that a Lich
//!   script starts by name -- `go2` first (`plan/46` §7) -- and hand back how
//!   it ends and how it is stopped, so the script waits on it as
//!   `Script.run` waits on a script. Started as an agent's `perform` is,
//!   through the binary's performer, and **not** among the agent's
//!   operations: a walk a script began is neither the agent's to read or
//!   steer, nor a run whose bad end drops the agent's level.
//!
//! And a runner's **hooks** (`plan/46` §6.1; the author, §10 question 2:
//! *"sure ask the script, with a time limit"*), which change what the player
//! is shown and what the player's typing becomes, never what the game sent:
//!
//! - [`Door::hook_lines`]: each line a viewer would be shown waits for the
//!   runner's display hooks to answer the line it came from
//!   ([`Door::shown`]), or for [`HOOK_DEADLINE`], and goes as it came
//!   past it. The model, the log, the triggers' flags and sends, and every
//!   script have the line on time: only its showing waits.
//! - [`Door::hook_typing`]: each line the player types is asked of the
//!   runner's input hooks before Hydra's command line or the game sees it
//!   ([`SessionHandle::send_typed_at`]), and goes as typed past the deadline.
//!
//! The player's own Lich, run through the relay rather than a runner
//! (`plan/51`), has a door of its own: [`lich`].

pub mod lich;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tokio::sync::{Notify, oneshot};

use crate::command::Claimed;
use crate::operation::{Ended, Reporter, Steer};
use crate::{Gate, Notice, Origin, Refusal, Sent, SessionHandle};

/// How long a line waits for a runner's hooks before it is shown, or sent,
/// as it came.
///
/// A choice, not a measurement (`plan/46` §9 owes one): past the 250 ms a
/// runner's watcher holds a chunk with no prompt for its state, with room for
/// the runner to answer. Answered, a line waits only for the answer.
pub const HOOK_DEADLINE: Duration = Duration::from_millis(500);

/// Asks a runner's input hooks what becomes of a line the player typed: the
/// line to use instead, or `None` to swallow it. Installed by whoever holds
/// the door ([`Door::hook_typing`]); a receiver that is dropped, or late,
/// leaves the line as typed.
pub type Asker = Arc<dyn Fn(&str) -> oneshot::Receiver<Option<String>> + Send + Sync>;

/// A runner's hooks as the session keeps them: shared by the actor, every
/// handle and the door, and kept across a reconnect, as `;sorter`'s switch
/// is (`crate::observation`).
#[derive(Default)]
pub(crate) struct Hooks {
    /// Whether each line shown waits for the runner's display hooks.
    display: AtomicBool,
    /// Answers the actor has not taken: the cursor of the line heard, and
    /// what is shown of it (`None` hides it).
    answers: Mutex<Vec<(u64, Option<String>)>>,
    /// Rung when answers come, or the display hooks go.
    answered: Notify,
    /// The runner's input hooks, while it has any.
    typing: Mutex<Option<Asker>>,
}

impl std::fmt::Debug for Hooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hooks")
            .field("display", &self.display())
            .field("typing", &self.typing().is_some())
            .finish_non_exhaustive()
    }
}

impl Hooks {
    /// Whether each line shown waits for the runner's display hooks.
    pub(crate) fn display(&self) -> bool {
        self.display.load(Ordering::Relaxed)
    }

    /// The answers that came since the last take.
    pub(crate) fn take_answers(&self) -> Vec<(u64, Option<String>)> {
        std::mem::take(&mut *self.answers.lock().unwrap_or_else(PoisonError::into_inner))
    }

    /// Resolves when answers came, or the display hooks went, since it was
    /// last waited on.
    pub(crate) async fn answered(&self) {
        self.answered.notified().await;
    }

    /// The runner's input hooks, if it has any.
    pub(crate) fn typing(&self) -> Option<Asker> {
        self.typing
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// The only way a script runner acts on a session.
///
/// Built by [`SessionHandle::script_door`], for `cena-agent`, which holds
/// this and never the handle.
#[derive(Clone, Debug)]
pub struct Door {
    handle: SessionHandle,
}

/// What became of a script's line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sending {
    /// It went to the game.
    Sent {
        /// The cursor its [`Event::Sent`](crate::Event::Sent) was published
        /// at: the lines after it came after the line (`plan/46` §3).
        cursor: u64,
    },
    /// It was one of Hydra's own commands, and Hydra took it.
    Ran,
    /// It started with the command symbol, and Hydra has no such command.
    /// Nobody was told: the runner knows which script sent it.
    Unknown,
    /// The session would not send it.
    Refused(Refusal),
    /// No connection took it: the session is gone, or between connections.
    Lost,
}

/// A built-in behavior a script started: how it ends, and how it is
/// stopped.
pub struct Run {
    /// The command as the performer keeps it: `go2 bank`.
    pub line: String,
    /// Resolves with how it ended.
    pub ended: std::pin::Pin<Box<dyn std::future::Future<Output = Ended> + Send>>,
    /// Steers it: a script stops it.
    pub steer: Steer,
}

impl std::fmt::Debug for Run {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Run")
            .field("line", &self.line)
            .finish_non_exhaustive()
    }
}

impl SessionHandle {
    /// The door a script runner acts on this session through.
    #[must_use]
    pub fn script_door(&self) -> Door {
        Door {
            handle: self.clone(),
        }
    }
}

impl Door {
    /// Publish each line as the game sent it while `on`: a runner reads
    /// them, and a character nobody scripts publishes each line once.
    pub fn listen(&self, on: bool) {
        self.handle.hear_lines(on);
    }

    /// Send `line` as if typed: Hydra's own command, or the game's line.
    /// Hydra's command line hears it as [`Origin::Script`], so the commands
    /// only the player may give refuse it.
    pub async fn send(&self, line: &str) -> Sending {
        match self.handle.typed(line, Origin::Script) {
            Some(Claimed::Unknown) => Sending::Unknown,
            Some(_) => Sending::Ran,
            None => match self.handle.send_now(line, Origin::Script, Gate::None).await {
                Sent::Ok { cursor, .. } => Sending::Sent { cursor },
                Sent::Refused(refusal) => Sending::Refused(refusal),
                Sent::Dead | Sent::Interrupted => Sending::Lost,
            },
        }
    }

    /// Tell the player something.
    pub fn say(&self, notice: Notice) {
        self.handle.say(notice);
    }

    /// Start `line`, a Hydra command without its symbol (`go2 bank`), as one
    /// of Hydra's built-in behaviors.
    ///
    /// # Errors
    ///
    /// Hydra's behaviors are not ready yet (the binary registers them once
    /// logged in), or they do not run `line`: why, in words.
    pub fn perform(&self, line: &str) -> Result<Run, String> {
        let performer = self
            .handle
            .performer()
            .ok_or_else(|| "Hydra's behaviors are not ready yet".to_owned())?;
        let line = (performer.allows)(line)?;
        let started = (performer.start)(&line, Reporter::unread(&self.handle));
        Ok(Run {
            line,
            ended: started.ended,
            steer: started.steer,
        })
    }

    /// Hold each line a viewer would be shown until the runner's display
    /// hooks answer the line it came from ([`Self::shown`]), or until
    /// [`HOOK_DEADLINE`], while `on`: the runner has display hooks. Off, what
    /// is held is shown at once. Only while the runner
    /// [`listen`](Self::listen)s: the line it answers is one it heard.
    pub fn hook_lines(&self, on: bool) {
        let hooks = self.handle.hooks();
        hooks.display.store(on, Ordering::Relaxed);
        if !on {
            hooks.answered.notify_one();
        }
    }

    /// What the runner's display hooks made of the lines heard at these
    /// cursors (each [`Event::Heard`](crate::Event::Heard)'s): the text to
    /// show instead, the same text to show them as they are, or `None` to
    /// hide them. An answer for a line no longer held is ignored.
    pub fn shown(&self, answers: impl IntoIterator<Item = (u64, Option<String>)>) {
        let hooks = self.handle.hooks();
        hooks
            .answers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend(answers);
        hooks.answered.notify_one();
    }

    /// Ask `asker` about each line the player types from now on, before
    /// Hydra's command line or the game sees it; `None` once the runner has
    /// no input hooks.
    pub fn hook_typing(&self, asker: Option<Asker>) {
        *self
            .handle
            .hooks()
            .typing
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = asker;
    }

    /// The character's command symbol, which marks a line as Hydra's own:
    /// what a runner is told a `;` command starts with.
    #[must_use]
    pub fn symbol(&self) -> char {
        self.handle
            .command_symbol()
            .unwrap_or(crate::command::COMMAND_SYMBOL)
    }
}
