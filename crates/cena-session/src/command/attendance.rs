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
//!
//! # An open page, when the player allows it
//!
//! The author, 2026-09-24, on whether a page left open should count: *"I
//! don't think open page is enough, maybe it could be an advanced option."*
//! (`plan/29` §5b). So a page showing the session is counted
//! ([`Watching`]), and it counts as a person only once the player has
//! allowed it ([`Attendance::let_pages_attend`]; the binary's
//! `--pages-attend`). Off, two clients fighting over one character still
//! end with Hydra giving it up, open page or not.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

/// A count of things a person did, and of pages open on the session, shared
/// by every clone of one session's handle and read by its supervisor.
#[derive(Clone, Debug, Default)]
pub(crate) struct Attendance {
    marks: Arc<AtomicU64>,
    pages: Arc<Pages>,
}

/// The pages open on a session, and whether they count.
#[derive(Debug, Default)]
struct Pages {
    open: AtomicUsize,
    attend: AtomicBool,
}

/// A page showing a session: counted while held, and no longer once
/// dropped ([`crate::SessionHandle::watching`]).
#[derive(Debug)]
pub struct Watching(Arc<Pages>);

impl Drop for Watching {
    fn drop(&mut self) {
        self.0.open.fetch_sub(1, Ordering::Relaxed);
    }
}

impl Attendance {
    /// A person did something.
    pub(crate) fn mark(&self) {
        self.marks.fetch_add(1, Ordering::Relaxed);
    }

    /// How many things a person has done, ever. Compared, never read alone.
    pub(crate) fn count(&self) -> u64 {
        self.marks.load(Ordering::Relaxed)
    }

    /// A page opened on the session.
    pub(crate) fn open_page(&self) -> Watching {
        self.pages.open.fetch_add(1, Ordering::Relaxed);
        Watching(Arc::clone(&self.pages))
    }

    /// Whether an open page counts as a person present.
    pub(crate) fn let_pages_attend(&self, on: bool) {
        self.pages.attend.store(on, Ordering::Relaxed);
    }

    /// A page is open and allowed to count: someone is here.
    pub(crate) fn watched(&self) -> bool {
        self.pages.attend.load(Ordering::Relaxed) && self.pages.open.load(Ordering::Relaxed) > 0
    }
}
