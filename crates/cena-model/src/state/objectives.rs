//! The quest and bounty list: `<objectives>` and its `<objective>` rows.
//!
//! # Why a list and not a log
//!
//! The wire sends three operations (`payload::ObjectivesAction`), and only
//! one of them is a snapshot. `full-refresh` IS the list; `patch-objective`
//! adds or updates rows; `delete-objective` removes them. A consumer that
//! treated every update as a snapshot would show one quest after a patch,
//! and one that appended everything would never lose a completed one.
//!
//! MEASURED over the author's September logs: 1,067 rows, of which 945 are
//! full quest entries and the rest bare `BOUNTY` ids.

use std::collections::BTreeMap;

use cena_protocol::{Objective, ObjectivesAction};

/// Every objective the game has stated, by id.
///
/// `BTreeMap` for the reason `Vitals` gives: iteration order must not vary
/// run to run, or a replay assertion over it is non-deterministic.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Objectives {
    entries: BTreeMap<String, Objective>,
    /// Whether the game has ever stated the list.
    ///
    /// Empty-and-never-said is not empty-and-said-so: §5.2's `Unknown`.
    known: bool,
}

impl Objectives {
    /// Apply one update.
    pub fn apply(&mut self, action: ObjectivesAction, entries: &[Objective]) {
        match action {
            // The list, entire. Anything absent is finished or abandoned.
            ObjectivesAction::FullRefresh => {
                self.entries.clear();
                self.known = true;
            }
            ObjectivesAction::Patch | ObjectivesAction::Delete => {}
            // An action this port does not know. Applying a guess could
            // delete a list the server meant to extend, so it changes
            // nothing -- and `Frame::ObjectivesUpdate` still reached every
            // observer, which is how it stays visible (Rule 2.2).
            ObjectivesAction::Other(_) => return,
        }
        for entry in entries {
            if action == ObjectivesAction::Delete {
                self.entries.remove(&entry.id);
            } else {
                self.entries.insert(entry.id.clone(), entry.clone());
            }
        }
    }

    /// One objective by id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Objective> {
        self.entries.get(id)
    }

    /// Every objective, by id.
    pub fn all(&self) -> impl Iterator<Item = &Objective> {
        self.entries.values()
    }

    /// Objectives of one `type=`, e.g. `QUEST` or `BOUNTY`.
    pub fn of_kind<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a Objective> + 'a {
        self.entries.values().filter(move |o| o.kind == kind)
    }

    /// How many are known.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether none are known.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Has the game ever stated the list?
    ///
    /// `false` with none held means nobody has asked; `true` with none held
    /// means the game said there are none.
    #[must_use]
    pub const fn is_known(&self) -> bool {
        self.known
    }
}
