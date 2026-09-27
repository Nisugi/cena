//! `wait`: what happened to a character after a cursor (`plan/35` §5, §6).
//!
//! **Few and meaningful** (`plan/35` §5): a status changed, the character
//! moved, a creature arrived, left or died, a command went out, Hydra said
//! something, the session's lifecycle moved. Not every frame.
//!
//! # Where they come from, and why no second model
//!
//! Most are the difference between two projections of the session's own
//! snapshots, taken when a prompt closes a chunk of game text. That is how
//! Despana stays current (`crates/cena-web/src/presentation.rs`: *"Native
//! snapshots are coalesced, never rebuilt by applying frames to a second
//! `GameState`"*), and it is why a status learned from text reaches `wait` as
//! surely as one from an indicator: both are in the snapshot's store. The rest
//! (`sent`, `notice`, `lifecycle`) are the session's own events, carried as
//! they are.
//!
//! **`changed` makes state reconstructable between reads** (issue #19, point
//! 1): each snapshot step that altered the projection carries every top-level
//! field that changed, with its new value. A reader holding `state` at one
//! cursor and every `changed` after it holds `state` at each later snapshot.
//! The fields that only count the clock (`roundtime`, `cast_roundtime`, an
//! effect's `seconds_left`, `game_time`, `captured_unix_ms`) are left out:
//! their absolute forms are in, and `game_time` rebuilds them.
//!
//! Every happening carries the session's cursor, which only increases, across
//! reconnects too (`cena_session::ObservedEvent::cursor`). A caller waits from
//! the cursor `state` gave it, then from the one `wait` returned.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde::Serialize;
use tokio::sync::Notify;

use crate::projection::CharacterState;

/// One thing that happened.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Happening {
    /// A status changed. `now` is `null` when it became unknown, as after a
    /// reconnect.
    Status {
        /// The status: `stunned`, `poisoned`, `IconDEAD`'s `dead`...
        id: String,
        /// Its value now.
        now: Option<bool>,
    },
    /// The character moved.
    Moved {
        /// The game's room number before, when known.
        from: Option<String>,
        /// The game's room number now, when known.
        to: Option<String>,
        /// The room's title now.
        title: Option<String>,
    },
    /// A creature is in the room that was not.
    Arrived {
        /// Its id.
        id: String,
        /// What the game calls it.
        name: String,
    },
    /// A creature that was in the room is not.
    Left {
        /// Its id.
        id: String,
        /// What the game called it.
        name: String,
    },
    /// A creature in the room died.
    CreatureDied {
        /// Its id.
        id: String,
        /// What the game calls it.
        name: String,
    },
    /// A command went out to the game.
    Sent {
        /// The command.
        line: String,
        /// Who sent it: `manual`, `behavior`, `script`.
        origin: String,
    },
    /// Hydra said something to the player.
    Notice {
        /// What it said.
        text: String,
    },
    /// The session's lifecycle changed: `ready`, `reconnecting`...
    Lifecycle {
        /// The state now.
        state: String,
    },
    /// Some `sent` and `notice` events were missed: the session published
    /// faster than this was read. The statuses and the room are not affected;
    /// they come from the next snapshot.
    Gap,
    /// The projection's fields that changed at this snapshot, with their new
    /// values: `state` rebuilt from the last one.
    Changed {
        /// Field name to its new value, as `state` spells both.
        fields: serde_json::Map<String, serde_json::Value>,
    },
}

/// Every kind, for `wait`'s filter and for `capabilities`.
pub const KINDS: &[&str] = &[
    "status",
    "moved",
    "arrived",
    "left",
    "creature_died",
    "sent",
    "notice",
    "lifecycle",
    "gap",
    "changed",
];

impl Happening {
    /// Its kind, as `wait` filters on it.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Status { .. } => "status",
            Self::Moved { .. } => "moved",
            Self::Arrived { .. } => "arrived",
            Self::Left { .. } => "left",
            Self::CreatureDied { .. } => "creature_died",
            Self::Sent { .. } => "sent",
            Self::Notice { .. } => "notice",
            Self::Lifecycle { .. } => "lifecycle",
            Self::Gap => "gap",
            Self::Changed { .. } => "changed",
        }
    }
}

/// What changed between two projections of one character.
#[must_use]
pub fn diff(before: &CharacterState, after: &CharacterState) -> Vec<Happening> {
    let mut out = Vec::new();
    let ids: BTreeSet<&String> = before
        .statuses
        .keys()
        .chain(after.statuses.keys())
        .collect();
    for id in ids {
        let (was, now) = (before.statuses.get(id), after.statuses.get(id));
        if was != now {
            out.push(Happening::Status {
                id: id.clone(),
                now: now.copied(),
            });
        }
    }
    let moved = before.room.id != after.room.id
        || (after.room.id.is_none() && before.room.title != after.room.title);
    if moved {
        out.push(Happening::Moved {
            from: before.room.id.clone(),
            to: after.room.id.clone(),
            title: after.room.title.clone(),
        });
        // A new room's creatures are the room, not arrivals.
        return out;
    }
    // Arrivals and departures only between two stated lists: a list that
    // was not stated says nothing about who came or went.
    let (Some(was), Some(now)) = (&before.room.creatures, &after.room.creatures) else {
        return out;
    };
    let was: BTreeMap<&str, &crate::projection::Creature> =
        was.iter().map(|c| (c.id.as_str(), c)).collect();
    let now: BTreeMap<&str, &crate::projection::Creature> =
        now.iter().map(|c| (c.id.as_str(), c)).collect();
    for (id, creature) in &now {
        match was.get(id) {
            None => out.push(Happening::Arrived {
                id: (*id).to_owned(),
                name: creature.name.clone(),
            }),
            Some(earlier) if !earlier.dead && creature.dead => out.push(Happening::CreatureDied {
                id: (*id).to_owned(),
                name: creature.name.clone(),
            }),
            Some(_) => {}
        }
    }
    for (id, creature) in &was {
        if !now.contains_key(id) {
            out.push(Happening::Left {
                id: (*id).to_owned(),
                name: creature.name.clone(),
            });
        }
    }
    out
}

/// The fields that only count the clock, left out of `changed`.
const CLOCK_ONLY: &[&str] = &[
    "cursor",
    "captured_unix_ms",
    "game_time",
    "roundtime",
    "cast_roundtime",
];

/// Every top-level field of `after` that differs from `before`, clock-only
/// fields aside; `None` when nothing did.
#[must_use]
pub fn changed(before: &CharacterState, after: &CharacterState) -> Option<Happening> {
    let (serde_json::Value::Object(was), serde_json::Value::Object(now)) = (
        steady(serde_json::to_value(before).ok()?),
        steady(serde_json::to_value(after).ok()?),
    ) else {
        return None;
    };
    let mut fields = serde_json::Map::new();
    for (key, value) in now {
        if !CLOCK_ONLY.contains(&key.as_str()) && was.get(&key) != Some(&value) {
            fields.insert(key, value);
        }
    }
    (!fields.is_empty()).then_some(Happening::Changed { fields })
}

/// A projection with each effect's `seconds_left` taken out: it counts the
/// clock, and `ends_at` says the same thing without moving.
fn steady(mut value: serde_json::Value) -> serde_json::Value {
    if let Some(effects) = value.get_mut("effects").and_then(|e| e.as_array_mut()) {
        for effect in effects {
            if let Some(effect) = effect.as_object_mut() {
                effect.remove("seconds_left");
            }
        }
    }
    value
}

/// A happening at its cursor.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Entry {
    /// The session's cursor it happened at.
    pub cursor: u64,
    /// What happened.
    #[serde(flatten)]
    pub happening: Happening,
}

/// What `wait` answers.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Waited {
    /// What happened after `since`, of the kinds asked for.
    pub happenings: Vec<Entry>,
    /// Wait from here next.
    pub cursor: u64,
    /// `since` fell out of the log: happenings between it and the oldest
    /// kept were lost. Read `state` again.
    pub lagged: bool,
    /// The character's session has ended; nothing more will come.
    pub closed: bool,
}

/// How many happenings a character's log keeps.
pub const KEPT: usize = 1_000;
/// How many one `wait` returns at most.
pub const RETURNED: usize = 200;
/// The longest one `wait` blocks.
pub const LONGEST_WAIT: Duration = Duration::from_secs(30);

/// One character's happenings, oldest first, bounded.
#[derive(Debug, Default)]
pub struct Log {
    inner: Mutex<Inner>,
    changed: Notify,
}

#[derive(Debug, Default)]
struct Inner {
    entries: VecDeque<Entry>,
    /// The highest cursor dropped to the bound; 0 when none was.
    dropped_through: u64,
    /// The latest cursor seen, happenings or not.
    latest: u64,
    closed: bool,
}

impl Log {
    /// A happening at `cursor`.
    pub fn push(&self, cursor: u64, happening: Happening) {
        let mut inner = self.lock();
        inner.entries.push_back(Entry { cursor, happening });
        while inner.entries.len() > KEPT {
            if let Some(dropped) = inner.entries.pop_front() {
                inner.dropped_through = dropped.cursor;
            }
        }
        inner.latest = inner.latest.max(cursor);
        drop(inner);
        self.changed.notify_waiters();
    }

    /// The session has reached `cursor`, whether or not anything happened.
    pub fn reached(&self, cursor: u64) {
        let mut inner = self.lock();
        inner.latest = inner.latest.max(cursor);
    }

    /// The session has ended.
    pub fn close(&self) {
        self.lock().closed = true;
        self.changed.notify_waiters();
    }

    /// Happenings after `since` of these `kinds` (every kind when `None`),
    /// waiting up to `timeout` (at most [`LONGEST_WAIT`]) for the first.
    pub async fn wait(&self, since: u64, kinds: Option<&[String]>, timeout: Duration) -> Waited {
        let deadline = tokio::time::Instant::now() + timeout.min(LONGEST_WAIT);
        loop {
            let notified = self.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let waited = self.read(since, kinds);
            if !waited.happenings.is_empty() || waited.closed || waited.lagged {
                return waited;
            }
            if tokio::time::timeout_at(deadline, notified).await.is_err() {
                return self.read(since, kinds);
            }
        }
    }

    fn read(&self, since: u64, kinds: Option<&[String]>) -> Waited {
        let inner = self.lock();
        let wanted = |entry: &&Entry| {
            entry.cursor > since
                && kinds.is_none_or(|kinds| kinds.iter().any(|k| k == entry.happening.kind()))
        };
        let mut happenings: Vec<Entry> = Vec::new();
        for entry in inner.entries.iter().filter(wanted) {
            // Never split one cursor's happenings between two answers: the
            // next `wait` starts after the cursor this one returns.
            let full = happenings.len() >= RETURNED
                && happenings
                    .last()
                    .is_some_and(|last| last.cursor != entry.cursor);
            if full {
                break;
            }
            happenings.push(entry.clone());
        }
        let truncated = happenings.len() >= RETURNED;
        let cursor = if truncated {
            happenings.last().map_or(since, |last| last.cursor)
        } else {
            inner.latest.max(since)
        };
        Waited {
            happenings,
            cursor,
            lagged: since < inner.dropped_through,
            closed: inner.closed,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A shared log.
pub type Shared = Arc<Log>;
