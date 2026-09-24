//! Whether a **person** is using a session: the supervisor's question when a
//! connection is lost, "is anyone here?".
//!
//! It used to be answered by counting every byte written, and a behavior
//! writes all the time -- so with a hunt running, a session always looked
//! attended, and two clients fighting over one character (Hydra and the
//! player's phone) would re-login over each other forever. The author,
//! 2026-09-24, of logging the character in elsewhere mid-hunt: *"yes it
//! should give him up"* (`plan/30` §6 Q5).
//!
//! So only what a person does is counted: a command typed at a page or the
//! terminal, claimed by Hydra's command line or sent to the game, whether or
//! not a connection is up to take it. It is counted **at the handle**,
//! because a typed `;go2` never reaches the actor and a command typed during
//! a reconnect never reaches the recorder, and both are a person present.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// A count of things a person did, shared by every clone of one session's
/// handle and read by its supervisor.
#[derive(Clone, Debug, Default)]
pub(crate) struct Attendance(Arc<AtomicU64>);

impl Attendance {
    /// A person did something.
    pub(crate) fn mark(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }

    /// How many things a person has done, ever. Compared, never read alone.
    pub(crate) fn count(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}
