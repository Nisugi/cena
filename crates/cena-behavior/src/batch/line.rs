//! What a batch does, one line at a time, and how it runs a Hydra command.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use cena_session::GameState;

/// One thing a batch does.
///
/// A game command is one variant; the rest are foreach.lic's conveniences
/// (`reference/scripts/scripts/foreach.lic:1963-2126`), each of which
/// `;multi` would send to the game as it stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    /// A command for the game, sent as the hunt sends one (`plan/30` §3):
    /// roundtime, cast roundtime, a stun and a web waited out first, then
    /// through the gate, taking the prompt. A `...wait N` in the answer is
    /// waited out and the line sent again, as Lich's `fput` does
    /// (`reference/lich-5/lib/global_defs.rb:1556-1565`).
    Send(String),
    /// A Hydra command, without its symbol: run, and waited for until what
    /// it started is over (Lich's `Script.run`, `multi.lic:61`,
    /// `foreach.lic:1963-1969`).
    Hydra(String),
    /// Wait out roundtime, if there is any (`waitrt`, `waitrt?`).
    WaitRt,
    /// Wait out cast roundtime, if there is any (`waitcastrt`,
    /// `waitcastrt?`).
    WaitCastRt,
    /// Wait this long (`sleep`).
    Sleep(Duration),
    /// Say this to the player (`echo`).
    Echo(String),
    /// Wait for a main-window line holding this phrase, in any case
    /// (`waitfor`).
    WaitFor(String),
    /// Wait for a main-window line this pattern matches (`waitre`). The
    /// pattern's source, already known to compile.
    WaitRe(String),
    /// Wait until a pool holds at least this much (`waitmana` and the rest).
    WaitVital(Pool, i32),
}

/// A pool a line may wait on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pool {
    /// `waitmana`, `waitmp`.
    Mana,
    /// `waithealth`, `waithp`.
    Health,
    /// `waitspirit`, `waitsp`.
    Spirit,
    /// `waitstamina`, `waitst`.
    Stamina,
}

impl Pool {
    /// How much the character has now, when the game has said.
    #[must_use]
    pub fn current(self, state: &GameState) -> Option<i32> {
        let vital = match self {
            Self::Mana => state.mana(),
            Self::Health => state.health(),
            Self::Spirit => state.spirit(),
            Self::Stamina => state.stamina(),
        };
        vital.and_then(|vital| vital.current)
    }
}

/// What running a Hydra command came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ran {
    /// It ran, and what it started, if anything, is over.
    Done,
    /// Nothing knows its word.
    Unknown,
}

/// How a batch runs a Hydra command: given the line without its symbol, a
/// future that resolves once what the command started is over.
///
/// The binary's to give, because only the binary holds every command's
/// family (`crates/cena/src/commands.rs`). A batch gives the authority back
/// before it calls this and takes it again after, so what the command
/// starts can claim it.
pub type Hydra = Arc<dyn Fn(&str) -> Pin<Box<dyn Future<Output = Ran> + Send>> + Send + Sync>;
