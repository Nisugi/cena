//! What the routine has sent in this room: bigshot's `@COMMANDS_REGISTRY`,
//! which the `once`, `once_here` and `every` guards read.
//!
//! bigshot records **every** command it runs, keyed by the command as
//! written, guards and all (`once_commands_register`, `bigshot.lic:4162`,
//! `:4170-4175`), against the creature it was aimed at and with the time;
//! and it forgets them all when it moves (`bs_wander`, `:9383`). So the
//! record is the room's, not the hunt's, and `every` is a per-room throttle
//! (`repeatdelay_blocked?`, `:4186-4191`).

use std::collections::{BTreeMap, BTreeSet};

/// Each step sent in this room, by its written form.
#[derive(Clone, Debug, Default)]
pub struct Used {
    sent: BTreeMap<String, Sent>,
}

/// One step's record.
#[derive(Clone, Debug, Default)]
struct Sent {
    /// The creatures it was aimed at.
    targets: BTreeSet<i64>,
    /// The game second it was last sent, when the clock was known.
    last: Option<u32>,
}

impl Used {
    /// Nothing sent yet.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sent: BTreeMap::new(),
        }
    }

    /// `step` was sent at `target`, at game second `now`.
    pub fn record(&mut self, step: &str, target: Option<i64>, now: Option<u32>) {
        let sent = self.sent.entry(step.to_owned()).or_default();
        if let Some(target) = target {
            sent.targets.insert(target);
        }
        sent.last = now;
    }

    /// A new room: what was sent in the last one is forgotten.
    pub fn clear(&mut self) {
        self.sent.clear();
    }

    /// Whether `step` has been sent at `target` in this room.
    #[must_use]
    pub fn at(&self, step: &str, target: i64) -> bool {
        self.sent
            .get(step)
            .is_some_and(|sent| sent.targets.contains(&target))
    }

    /// Whether `step` has been sent at all in this room.
    #[must_use]
    pub fn here(&self, step: &str) -> bool {
        self.sent.contains_key(step)
    }

    /// Whether `step` may run again, `every` seconds after it was last sent
    /// here: `Some(true)` when it never was, `None` when either time is
    /// unknown.
    #[must_use]
    pub fn due(&self, step: &str, every: u32, now: Option<u32>) -> Option<bool> {
        let Some(sent) = self.sent.get(step) else {
            return Some(true);
        };
        let (last, now) = (sent.last?, now?);
        Some(now.saturating_sub(last) >= every)
    }
}
