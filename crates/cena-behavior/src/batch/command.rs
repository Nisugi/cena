//! What a player types for a batch: `;multi ...` or `;foreach ...`.

use super::foreach::{self, Foreach};
use super::multi::{self, Multi};

/// Which batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `;multi`.
    Multi,
    /// `;foreach`.
    Foreach,
}

impl Kind {
    /// The name it is said with: `Multi`, `Foreach`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Multi => "Multi",
            Self::Foreach => "Foreach",
        }
    }

    /// How it is used.
    #[must_use]
    pub fn usage(self) -> Vec<String> {
        let lines = match self {
            Self::Multi => multi::USAGE,
            Self::Foreach => foreach::USAGE,
        };
        lines.iter().map(|line| (*line).to_owned()).collect()
    }
}

/// A batch to run.
#[derive(Clone, Debug)]
pub enum Job {
    /// `;multi`.
    Multi(Multi),
    /// `;foreach`.
    Foreach(Foreach),
}

impl Job {
    /// Which batch it is.
    #[must_use]
    pub const fn kind(&self) -> Kind {
        match self {
            Self::Multi(_) => Kind::Multi,
            Self::Foreach(_) => Kind::Foreach,
        }
    }
}

/// What was asked.
#[derive(Clone, Debug)]
pub enum Command {
    /// Run this.
    Run(Job),
    /// Stop the one of this kind that is running.
    Stop(Kind),
    /// Say how this kind is used.
    Help(Kind),
}

/// Parse a command line, without its symbol. `None` when the word is
/// neither `multi` nor `foreach`; `Some(Err)` with the reason, said as it
/// stands, when it is one and cannot be run. `symbol` is the character's
/// command symbol.
#[must_use]
pub fn parse(line: &str, symbol: char) -> Option<Result<Command, String>> {
    if let Some(parsed) = multi::parse(line, symbol) {
        return Some(parsed.map(|command| match command {
            multi::Command::Run(multi) => Command::Run(Job::Multi(multi)),
            multi::Command::Stop => Command::Stop(Kind::Multi),
            multi::Command::Help => Command::Help(Kind::Multi),
        }));
    }
    let parsed = foreach::parse(line, symbol)?;
    Some(parsed.map(|command| match command {
        foreach::Command::Run(foreach) => Command::Run(Job::Foreach(foreach)),
        foreach::Command::Stop => Command::Stop(Kind::Foreach),
        foreach::Command::Help => Command::Help(Kind::Foreach),
    }))
}
