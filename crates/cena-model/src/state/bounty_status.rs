//! The bounty a character is on, and what the guild will do next.
//!
//! [`bounty`](super::bounty) ports Lich's parser and answers *"what does this
//! sentence say"*. **Nothing called it.** MEASURED 2026-09-21: the only
//! non-test references to `bounty::` in the crate were inside `bounty.rs`
//! itself, and `GameState` had no field for one -- so a fully ported,
//! thoroughly tested classifier was dead code.
//!
//! This is the consumer above it (`plan/12` section 3a: one parser, N
//! classifiers, and anything needing memory across lines lives above the
//! model). It holds the current task and the guild's own answers.
//!
//! # What the parser does not say, and `ebounty.lic` does
//!
//! `bounty` reads the task description. A character hunting bounties needs
//! three more facts, and they arrive as ordinary guild speech rather than in
//! the task itself. Ported from `ebounty.lic`'s `@taskmaster_responses`
//! (`:950-961`) and its voucher check (`:2438`):
//!
//! | Line | What it means |
//! |---|---|
//! | `You have already been assigned a task, <name>.` | asking again is refused |
//! | `Come back in about N minutes if you want another task.` | the guild is on a timer |
//! | `I have removed you from your current assignment` | the task is gone |
//! | `You have N expedited task reassignment vouchers remaining` | how many skips are left |
//!
//! The wait is **minutes, as the guild states them**, not an absolute time.
//! The line says "about", so converting it to a deadline here would invent a
//! precision the game does not offer; a caller that wants one has the game
//! clock and can say how stale the reading is.
//!
//! # What this does NOT do
//!
//! **It sends nothing.** No `ask about bounties`, no travel, no looting --
//! that is a behavior's, at M6, and `ebounty.lic` is 3,894 lines of mostly
//! that. This is the reading half, the same split `stash.rb`, `bank.rb` and
//! `fog.rb` took (`plan/20` section 0b).

use super::bounty::{Task, TaskKind};

/// Why the guild will not give a task right now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// `You have already been assigned a task, <name>.`
    AlreadyAssigned,
    /// `Come back in about N minutes if you want another task.`
    ///
    /// Minutes as stated. The guild says "about", so this is not a deadline.
    Wait {
        /// The `N` from the message, parsed as an integer.
        minutes: u32,
    },
    /// `I don't have any tasks for you right now` -- `ebounty.lic:2596` waits
    /// on it beside the others.
    NoneAvailable,
}

/// The bounty a character is on.
///
/// Every field distinguishes "not told" from a stated absence, per
/// `plan/12` section 5.2: a character who has never run `bounty` is not a
/// character with no bounty.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BountyStatus {
    /// The current task, as the guild last described it.
    ///
    /// `Some(task)` with [`TaskKind::None`] is the game having said *"You are
    /// not currently assigned a task"*; `None` is nobody having asked.
    task: Option<Task>,
    /// The guild's last refusal, if its last word was one.
    pub refusal: Option<Refusal>,
    /// `You have N expedited task reassignment vouchers remaining`.
    pub vouchers: Option<u32>,
}

impl BountyStatus {
    /// Read one line of guild or task output.
    ///
    /// Returns whether anything changed, which is the shape
    /// `consume_standing` uses so a re-read that finds everything identical
    /// does not mark the character dirty.
    pub fn read_line(&mut self, line: &str) -> bool {
        let text = line.trim();
        if let Some(task) = super::bounty::classify(text) {
            // A task description answers the guild's state too: being told
            // what the task IS means it is no longer refusing.
            let changed = self.task.as_ref() != Some(&task) || self.refusal.is_some();
            self.task = Some(task);
            self.refusal = None;
            return changed;
        }
        if let Some(refusal) = classify_refusal(text) {
            let changed = self.refusal.as_ref() != Some(&refusal);
            self.refusal = Some(refusal);
            return changed;
        }
        if let Some(vouchers) = vouchers_remaining(text) {
            let changed = self.vouchers != Some(vouchers);
            self.vouchers = Some(vouchers);
            return changed;
        }
        false
    }

    /// The current task. `None`: nobody has asked.
    #[must_use]
    pub const fn task(&self) -> Option<&Task> {
        self.task.as_ref()
    }

    /// The kind of the current task, where one is known.
    #[must_use]
    pub fn kind(&self) -> Option<TaskKind> {
        self.task.as_ref().map(|t| t.kind)
    }

    /// Whether the character has work to do.
    ///
    /// `None` until the game has said. `Some(false)` covers both "not
    /// currently assigned" and a finished task waiting to be turned in --
    /// which is why [`Self::kind`] is what a caller acts on.
    #[must_use]
    pub fn has_work(&self) -> Option<bool> {
        self.task.as_ref().map(|t| t.kind.is_actionable())
    }

    /// Forget everything: a reconnect. The guild re-states all of it the next
    /// time it is asked, and nothing re-sends it unasked.
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// `You have N expedited task reassignment vouchers remaining` --
/// `ebounty.lic:2438`. The count carries thousands separators in the pattern
/// there, so they are stripped.
#[must_use]
pub fn vouchers_remaining(line: &str) -> Option<u32> {
    let rest = line.trim().strip_prefix("You have ")?;
    let (count, _) = rest.split_once(" expedited task reassignment vouchers remaining")?;
    super::numbers::grouped(count)
}

/// One of the guild's refusals -- `ebounty.lic:950-961`.
#[must_use]
pub fn classify_refusal(line: &str) -> Option<Refusal> {
    let text = line.trim();
    if text.starts_with("You have already been assigned a task") {
        return Some(Refusal::AlreadyAssigned);
    }
    if text.starts_with("I don't have any tasks for you right now") {
        return Some(Refusal::NoneAvailable);
    }
    // `Come back in about 27 minutes if you want another task.` The number is
    // spelled with digits in the script's own pattern (`.*` there, a count
    // here, because a caller wants the number rather than the sentence).
    let rest = text.strip_prefix("Come back in about ")?;
    let (count, tail) = rest.split_once(' ')?;
    tail.starts_with("minute")
        .then(|| count.parse().ok().map(|minutes| Refusal::Wait { minutes }))
        .flatten()
}
