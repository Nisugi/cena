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
//!   (`plan/45` Stage 5), so nothing marked as Hydra's reaches the game;
//!   otherwise to the game at once as [`Origin::Script`], with no roundtime
//!   gate and no queue, as `Game.puts` writes straight to the socket
//!   (`inventory/13` §1.2), and never counted as the player being there.
//! - [`Door::say`]: tell the player something, Lich's `respond`.

use crate::command::Claimed;
use crate::{Gate, Notice, Origin, Refusal, Sent, SessionHandle};

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
    pub async fn send(&self, line: &str) -> Sending {
        match self.handle.typed(line) {
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
}
