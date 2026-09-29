//! The board: what the members of one group publish, and read of each
//! other (`plan/39` §5).
//!
//! One per group, found by the leader's name. Every member publishes its
//! [`Report`] each tick; the leader also publishes [`Leading`], what it is
//! doing now. Each is a [`watch`]: a writer never waits for a reader, and a
//! reader takes the latest (`plan/12` §5.5, *no shared lock on a hot
//! path*). **Nothing crosses sessions but this data**: a follower reads the
//! leader's state and sends on its own session, through its own gate
//! (`plan/39` §3).
//!
//! # Who is on it
//!
//! A member is on the board of whoever leads its game group, and the role
//! it reads off that group decides which ([`super::role`]). A member whose
//! hunt ends takes its report off ([`Board::withdraw`]), so the leader
//! stops counting it; one whose session Hydra gave up leaves it marked
//! `Closed`, for the leader's muster to read (`plan/39` §8a).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::watch;

use super::report::{Leading, Report};

/// How a hunt takes its place in a group as it starts (`plan/39` §8,
/// question 2): by what the game's group says, or as the command that
/// forms the group says, until the game's group agrees.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Place {
    /// Whatever the game's group says: `;hunt <profile>` typed on a member.
    #[default]
    Read,
    /// Leading these characters, who were asked to join: waited for until
    /// each has a report up, within `lost_wait`.
    Lead(Vec<String>),
    /// Following this leader: caught up to and joined, even from outside
    /// its group.
    Follow(String),
}

/// Every group's board in this Hydra, by the leading character's name.
#[derive(Debug, Default)]
pub struct Boards {
    boards: Mutex<BTreeMap<String, Arc<Board>>>,
    /// Leaders lost and who took over from each (`plan/39` Stage 6), for
    /// the one who comes back to follow (question 8).
    handed: Mutex<BTreeMap<String, String>>,
}

impl Boards {
    /// No groups yet.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// The board `leader` leads, made the first time it leads.
    #[must_use]
    pub fn lead(&self, leader: &str) -> Arc<Board> {
        let mut boards = self.boards.lock().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(
            boards
                .entry(leader.to_owned())
                .or_insert_with(|| Arc::new(Board::new())),
        )
    }

    /// The board `leader` leads, when its hunt has made one.
    #[must_use]
    pub fn of(&self, leader: &str) -> Option<Arc<Board>> {
        let boards = self.boards.lock().unwrap_or_else(PoisonError::into_inner);
        boards.get(leader).cloned()
    }

    /// `new` leads what `old` led: said to `old` when it comes back.
    pub fn hand_over(&self, old: &str, new: &str) {
        let mut handed = self.handed.lock().unwrap_or_else(PoisonError::into_inner);
        handed.insert(old.to_owned(), new.to_owned());
    }

    /// Who leads what `old` led, **decided once for the group**: the one
    /// already named, while it still `stands`, else the one `choose` names,
    /// kept for the members who ask after.
    ///
    /// Each follower finds the leader lost on a turn of its own and reads
    /// the reports as they stand then. With no list of successors the
    /// healthiest leads, and health moves in a fight: two followers a
    /// moment apart each found itself the healthiest and both led, or each
    /// found the other and both followed (the review of 2026-09-29). The
    /// first to ask decides, under one lock, and the rest are told.
    #[must_use]
    pub fn succeed(
        &self,
        old: &str,
        stands: impl Fn(&str) -> bool,
        choose: impl FnOnce() -> Option<String>,
    ) -> Option<String> {
        let mut handed = self.handed.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(new) = handed.get(old)
            && stands(new)
        {
            return Some(new.clone());
        }
        let new = choose();
        match &new {
            Some(new) => handed.insert(old.to_owned(), new.clone()),
            None => handed.remove(old),
        };
        new
    }

    /// Who took over from `old`, once: `old` follows them now.
    #[must_use]
    pub fn take_handed(&self, old: &str) -> Option<String> {
        let mut handed = self.handed.lock().unwrap_or_else(PoisonError::into_inner);
        handed.remove(old)
    }

    /// Take down the board `leader` leads: its hunt is over, and so are its
    /// followers' (`plan/39` §8, question 3).
    pub fn close(&self, leader: &str) {
        let mut boards = self.boards.lock().unwrap_or_else(PoisonError::into_inner);
        boards.remove(leader);
    }

    /// Take `name`'s report off every board but `keep`'s: a member is on one
    /// board, the one of whoever it follows or leads now.
    pub fn withdraw_except(&self, name: &str, keep: Option<&str>) {
        let boards = self.boards.lock().unwrap_or_else(PoisonError::into_inner);
        for (leader, board) in boards.iter() {
            if Some(leader.as_str()) != keep {
                board.withdraw(name);
            }
        }
    }
}

/// One group's board.
#[derive(Debug)]
pub struct Board {
    leading: watch::Sender<Leading>,
    reports: watch::Sender<BTreeMap<String, Report>>,
}

impl Board {
    fn new() -> Self {
        Self {
            leading: watch::Sender::new(Leading::default()),
            reports: watch::Sender::new(BTreeMap::new()),
        }
    }

    /// Put up `report` as its member's latest.
    pub fn publish(&self, report: Report) {
        self.reports.send_if_modified(|reports| {
            let same = reports.get(&report.name) == Some(&report);
            if !same {
                reports.insert(report.name.clone(), report);
            }
            !same
        });
    }

    /// Take `name`'s report off.
    pub fn withdraw(&self, name: &str) {
        self.reports
            .send_if_modified(|reports| reports.remove(name).is_some());
    }

    /// The leader's latest.
    pub fn lead(&self, leading: Leading) {
        self.leading.send_if_modified(|held| {
            let changed = *held != leading;
            if changed {
                *held = leading;
            }
            changed
        });
    }

    /// What the leader last published.
    #[must_use]
    pub fn leading(&self) -> Leading {
        self.leading.borrow().clone()
    }

    /// Every member's latest report, `name`'s own among them.
    #[must_use]
    pub fn reports(&self) -> BTreeMap<String, Report> {
        self.reports.borrow().clone()
    }
}
