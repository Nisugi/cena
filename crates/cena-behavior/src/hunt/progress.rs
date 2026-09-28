//! How a hunt is getting on, and when nothing is coming of it (`plan/35` §4,
//! "No progress"; issue #19, point 6; M7 step 3c).
//!
//! The watchdog catches a hunt whose loop has stopped. It cannot catch one
//! whose loop goes round and round to no purpose: wandering empty rooms,
//! revisiting the same few, never finding a creature to fight. That is the
//! stall this reports: **in the hunting ground, not held, and nothing engaged
//! for [`STALL_AFTER`] game seconds**, with how many rooms were searched
//! since. It is said once, and said again only when it changes.
//!
//! The counts are what the machine already knows, kept as it goes: creatures
//! engaged, rests taken, rooms searched while hunting. None is a claim about
//! what the game did -- an engagement is a creature chosen and fought, not a
//! kill.

use std::collections::{BTreeMap, BTreeSet};

use cena_session::operation::Progress;

use super::engine::Hunt;
use super::said::Phase;

/// How long the hunting ground may give nothing before the hunt says it is
/// stalled, in game seconds.
pub const STALL_AFTER: u32 = 300;

/// What a hunt keeps count of as it goes.
#[derive(Clone, Debug, Default)]
pub(super) struct Tally {
    /// The game second the hunt first ticked.
    began: Option<u32>,
    /// Creatures chosen and fought, each once.
    engaged: u64,
    /// The game second the last one was.
    last_engaged: Option<u32>,
    /// Rooms entered while hunting, ever, and since the last engagement.
    rooms: BTreeSet<String>,
    rooms_since: BTreeSet<String>,
}

impl Tally {
    /// A tick at game second `now`.
    pub(super) fn ticked(&mut self, now: Option<u32>) {
        if self.began.is_none() {
            self.began = now;
        }
    }

    /// A creature was chosen to fight, at `now`.
    pub(super) fn engaged(&mut self, now: Option<u32>) {
        self.engaged += 1;
        self.last_engaged = now.or(self.last_engaged);
        self.rooms_since.clear();
    }

    /// A room was entered while hunting.
    pub(super) fn entered(&mut self, room: &str) {
        self.rooms.insert(room.to_owned());
        self.rooms_since.insert(room.to_owned());
    }
}

impl Hunt {
    /// How the hunt is getting on at game second `now`.
    #[must_use]
    pub fn progress(&self, now: Option<u32>) -> Progress {
        let doing = if self.steered.hold {
            format!("held: {}", self.phase)
        } else {
            self.phase.to_string()
        };
        let counts = BTreeMap::from([
            ("engaged".to_owned(), self.tally.engaged),
            ("rests".to_owned(), u64::from(self.rests)),
            (
                "rooms_searched".to_owned(),
                u64::try_from(self.tally.rooms.len()).unwrap_or(u64::MAX),
            ),
        ]);
        Progress {
            doing,
            counts,
            stalled: self.stalled(now),
        }
    }

    /// Tell whoever steers this hunt how it is getting on at `now`; the
    /// driver calls it after each tick. Nothing when nothing steers it.
    pub fn report_progress(&self, now: Option<u32>) {
        if let Some(steering) = &self.steering {
            steering.report(self.progress(now));
        }
    }

    /// Why the hunt is getting nowhere, when it is.
    fn stalled(&self, now: Option<u32>) -> Option<String> {
        if self.phase != Phase::Hunting || self.steered.hold {
            return None;
        }
        let since = self.tally.last_engaged.or(self.tally.began)?;
        let idle = now?.checked_sub(since)?;
        (idle >= STALL_AFTER).then(|| {
            let rooms = self.tally.rooms_since.len();
            let searched = if rooms == 1 { "room" } else { "rooms" };
            format!(
                "nothing engaged for {} minutes, across {rooms} {searched}",
                idle / 60
            )
        })
    }
}
