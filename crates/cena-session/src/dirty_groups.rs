//! When a character's facts get written to disk.
//!
//! # The rule
//!
//! > **AUTHOR, 2026-09-20:** *"I say they are written to a queue that gets
//! > drained 5 minutes after it stops changing?"*
//!
//! A group goes dirty when something teaches it, and the deadline moves to
//! five minutes from now. When it fires, **every** dirty group is written in
//! one save -- the file is written whole regardless, so coalescing is free:
//!
//! > **AUTHOR:** *"We do want "bulk writes" if possible, We're just talking
//! > "infomon" data here right, things that don't change quickly."*
//!
//! Which is the reason this is safe. The five commands' worth of facts here
//! change when you train, join a society or swap an enhancive item -- not
//! continuously. A sync marks every group dirty within the same second and
//! they collapse into one write.
//!
//! # Why the queue holds GROUPS and not values
//!
//! A `BTreeSet<Group>` is at most six entries no matter how many facts arrive,
//! so the queue cannot grow without bound. Marking a group dirty twice is one
//! entry, and a group is the natural unit anyway: train five PSM ranks in a
//! row and that is five marks on `Psms` and **one** write.
//!
//! # The starvation question, asked and answered
//!
//! > **AUTHOR:** *"So then what goes in there so it doesn't get in a never
//! > drain loop?"*
//!
//! The timer resets on every change, so a fact arriving faster than the window
//! would hold the write off forever. Two things make that a non-issue rather
//! than a bug to engineer around:
//!
//! 1. **The data does not behave that way.** These are facts taught by
//!    commands and by rare events, not by continuous traffic.
//! 2. **A clean shutdown flushes unconditionally**, so an ordinary logout
//!    never waits on the timer.
//!
//! A maximum-age cap was considered and **left out** under `plan/05` §-1: it
//! guards against a fact changing every four minutes for an hour, which this
//! data does not do. If measurement ever shows otherwise, the cap is the known
//! fix and this paragraph is where to find it.

use std::collections::BTreeSet;
use std::time::Duration;

use cena_model::state::character::snapshot::Group;

/// How long a group stays dirty before it is written.
pub const IDLE_WINDOW: Duration = Duration::from_mins(5);

/// Groups taught since the last save, and when they should be written.
///
/// Holds no clock of its own: the deadline is an instant the caller supplies
/// and compares. That keeps this testable without sleeping and keeps the
/// tokio dependency in the actor where it belongs.
#[derive(Debug, Default)]
pub struct DirtyGroups {
    groups: BTreeSet<Group>,
}

impl DirtyGroups {
    /// Note that a group was just taught.
    ///
    /// **Always pushes the deadline out**, even for a group already dirty:
    /// the window is "five minutes after it stops CHANGING", and a group being
    /// taught again is a change. Returning `insert`'s answer instead would
    /// mean a fact arriving every minute on an already-dirty group let the
    /// write fire while it was still moving.
    ///
    /// The caller decides whether to mark at all -- `Standing::apply_society`
    /// and its siblings return whether anything actually changed, so a re-sync
    /// that finds everything identical marks nothing and does not delay a
    /// pending write.
    pub fn mark(&mut self, group: Group) {
        self.groups.insert(group);
    }

    /// Note several groups at once, as a sync does.
    pub fn mark_all(&mut self, groups: impl IntoIterator<Item = Group>) {
        for group in groups {
            self.mark(group);
        }
    }

    /// Whether anything is waiting to be written.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    /// How many groups are waiting.
    #[must_use]
    pub fn len(&self) -> usize {
        self.groups.len()
    }

    /// Whether this group is waiting to be written.
    #[must_use]
    pub fn contains(&self, group: Group) -> bool {
        self.groups.contains(&group)
    }

    /// Take everything waiting, leaving the queue empty.
    ///
    /// Returns the groups in [`Group::ALL`] order, which is what makes a
    /// replay deterministic.
    pub fn drain(&mut self) -> Vec<Group> {
        std::mem::take(&mut self.groups).into_iter().collect()
    }
}

/// Everything the five-minute rule needs: what is dirty, where it goes, when.
///
/// One struct so the actor carries one field, and **boxed there** because
/// three inline fields pushed its future past clippy's `large_futures`
/// threshold. See `SessionActor::persistence`.
#[derive(Debug, Default)]
pub struct Persistence {
    /// Groups taught since the last save.
    pub groups: DirtyGroups,
    /// When they are due. `None` disarms the actor's timer arm.
    pub deadline: Option<tokio::time::Instant>,
    /// Where this character's facts are written. `None` writes nothing, which
    /// is every test and any caller that has not opted in.
    pub dir: Option<std::path::PathBuf>,
    /// Whether the stored snapshot has been read back yet.
    ///
    /// The load needs a name, which arrives with `<app>` partway into the
    /// login burst, so it cannot happen at construction. This makes it happen
    /// **once**: a second load would overwrite facts the session has learned
    /// since with the older ones on disk.
    pub loaded: bool,
}
