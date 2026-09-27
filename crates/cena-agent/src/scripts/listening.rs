//! What a script runner listens to (`plan/46` §4.1): one character's lines
//! as the game sent them, the lines that went out, prompts, the lines the
//! player typed for the runner, and the connection's state, in order.
//!
//! **Its own positions, beside the session's cursor.** A line the player
//! types for the runner is no event of the session's and has no cursor, yet
//! must arrive in order with the rest; so each entry takes the next
//! position here, and what the session published carries its cursor too,
//! which is what `send` answers with (`plan/46` §3).
//!
//! **Read, then let go.** A runner reads from the position after the last it
//! handled, and asking from a position lets go of everything at or before
//! it: a reply that was lost on the way is answered again when the runner
//! asks again from where it was. The log is bounded; past the bound the
//! oldest entry goes, and the next read says `lagged`. **A typed line is
//! never let go for room**: a `;kill` lost to a flood of combat lines would
//! be lost exactly when it mattered; nor is a line asked of the input hooks,
//! which the player is waiting on.

use std::collections::VecDeque;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use serde::Serialize;
use tokio::sync::Notify;

/// One thing a runner is told.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    /// The local copy changed (`super::local`): every field that did, with
    /// its new value; the first one a runner is told carries every field.
    /// It comes **before** the lines of the chunk that changed it.
    State {
        /// The session's cursor of the snapshot it was taken from: every
        /// event at or before it is in it.
        cursor: u64,
        /// Field name to its new value.
        fields: serde_json::Map<String, serde_json::Value>,
    },
    /// A line of game text as the game sent it: before the player's
    /// triggers and `;sorter` (`cena_session::Event::Heard`).
    Line {
        /// The session's cursor it was published at.
        cursor: u64,
        /// Its stream: `""` is the main window; `thoughts`, `speech`...
        stream: String,
        /// The text, markup removed.
        text: String,
    },
    /// A line went out to the game.
    Sent {
        /// The session's cursor it was published at: `send` answers the
        /// same for a runner's own.
        cursor: u64,
        /// The line.
        line: String,
        /// Who sent it: `manual`, `behavior`, `script`, `trigger`, `agent`.
        origin: String,
    },
    /// The game's prompt: the end of a chunk of text.
    Prompt {
        /// The session's cursor it was published at.
        cursor: u64,
        /// The game's clock, epoch seconds, when the prompt carried it.
        time: Option<u64>,
        /// The prompt itself: `>`, `R>`...
        text: String,
    },
    /// The player typed a command for the runner: the line after the
    /// command symbol, `trollspeak say hello` or `k trollspeak`.
    Typed {
        /// What was typed, without the symbol.
        line: String,
    },
    /// The player typed a line, and the runner's input hooks say what
    /// becomes of it before anything else sees it: answered with `input`.
    Input {
        /// The question's number, for the answer.
        asked: u64,
        /// The line as typed, the command symbol and all.
        line: String,
    },
    /// The connection's state changed.
    Lifecycle {
        /// The session's cursor it was published at.
        cursor: u64,
        /// `ready`, `reconnecting`...
        state: String,
        /// Which connection; it advances on every reconnect.
        generation: u64,
    },
    /// A built-in a runner started (`perform`) ended.
    Ended {
        /// The run's number, as `perform` gave it.
        run: u64,
        /// What the work came to: `completed`, `failed`, `interrupted`,
        /// `no_opportunity`, `unknown`.
        work: String,
        /// The behavior's own word for why: `arrived`, `stopped`...
        reason: String,
        /// What it left undone that someone may need to put right.
        left: Vec<String>,
    },
    /// Events were missed between the session and this log: lines a script
    /// was waiting for may be among them.
    Lagged {
        /// How many of the session's events were missed.
        missed: u64,
    },
}

/// An event at its position.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Entry {
    /// Its position in this log.
    pub at: u64,
    /// What it was.
    #[serde(flatten)]
    pub event: Event,
}

/// What `listen` answers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Heard {
    /// What came after the position asked from, oldest first.
    pub events: Vec<Entry>,
    /// Ask from here next.
    pub next: u64,
    /// Events after the position asked from were let go for room.
    pub lagged: bool,
    /// The character's session has ended, or the runner was dismissed:
    /// nothing more will come.
    pub closed: bool,
}

/// How many entries the log keeps. The size of the session's own event ring
/// (`cena-session`'s `actor/bounds.rs`), which a login burst fits.
pub const KEPT: usize = 4_096;
/// How many one read returns at most.
pub const RETURNED: usize = 500;
/// The longest one read blocks.
pub const LONGEST_WAIT: Duration = Duration::from_secs(30);

/// One runner's log.
#[derive(Debug, Default)]
pub struct Listening {
    inner: Mutex<Inner>,
    changed: Notify,
}

#[derive(Debug, Default)]
struct Inner {
    entries: VecDeque<Entry>,
    /// The last position given.
    last: u64,
    /// The highest position let go for room; 0 when none was.
    dropped_through: u64,
    closed: bool,
}

impl Listening {
    /// Add `event` at the next position.
    pub fn push(&self, event: Event) {
        let mut inner = self.lock();
        inner.last += 1;
        let at = inner.last;
        inner.entries.push_back(Entry { at, event });
        while inner.entries.len() > KEPT {
            let Some(oldest) = inner.entries.iter().position(|entry| {
                !matches!(entry.event, Event::Typed { .. } | Event::Input { .. })
            }) else {
                break;
            };
            if let Some(dropped) = inner.entries.remove(oldest) {
                inner.dropped_through = inner.dropped_through.max(dropped.at);
            }
        }
        drop(inner);
        self.changed.notify_waiters();
    }

    /// Nothing more will come.
    pub fn close(&self) {
        self.lock().closed = true;
        self.changed.notify_waiters();
    }

    /// What came after `since`, waiting up to `timeout` (at most
    /// [`LONGEST_WAIT`]) for the first. Lets go of everything at or before
    /// `since`: the runner has it.
    pub async fn listen(&self, since: u64, timeout: Duration) -> Heard {
        let deadline = tokio::time::Instant::now() + timeout.min(LONGEST_WAIT);
        loop {
            let notified = self.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let heard = self.read(since);
            if !heard.events.is_empty() || heard.closed || heard.lagged {
                return heard;
            }
            if tokio::time::timeout_at(deadline, notified).await.is_err() {
                return self.read(since);
            }
        }
    }

    fn read(&self, since: u64) -> Heard {
        let mut inner = self.lock();
        while inner.entries.front().is_some_and(|entry| entry.at <= since) {
            inner.entries.pop_front();
        }
        // A typed line kept past the bound may sit ahead of lines already
        // read: let go of those too.
        inner.entries.retain(|entry| entry.at > since);
        let events: Vec<Entry> = inner.entries.iter().take(RETURNED).cloned().collect();
        Heard {
            next: events.last().map_or(since, |last| last.at),
            lagged: since < inner.dropped_through,
            closed: inner.closed && inner.entries.len() == events.len(),
            events,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(cursor: u64) -> Event {
        Event::Line {
            cursor,
            stream: String::new(),
            text: format!("line {cursor}"),
        }
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn a_read_is_answered_again_until_the_runner_moves_on() {
        let log = Listening::default();
        log.push(line(10));
        log.push(line(11));
        let first = log.listen(0, Duration::ZERO).await;
        assert_eq!(first.events.len(), 2);
        assert_eq!(first.next, 2);
        // The reply was lost: asked again from the same place.
        assert_eq!(log.listen(0, Duration::ZERO).await, first);
        let after = log.listen(first.next, Duration::ZERO).await;
        assert!(after.events.is_empty() && !after.lagged);
        assert_eq!(after.next, 2);
    }

    /// Past the bound the oldest line goes and the read says so; a typed
    /// line never goes for room.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn a_flood_drops_lines_and_never_a_typed_command() {
        let log = Listening::default();
        log.push(Event::Typed {
            line: "k trollspeak".into(),
        });
        for cursor in 0..KEPT as u64 + 10 {
            log.push(line(cursor));
        }
        let heard = log.listen(0, Duration::ZERO).await;
        assert!(heard.lagged);
        assert_eq!(
            heard.events.first().map(|entry| &entry.event),
            Some(&Event::Typed {
                line: "k trollspeak".into()
            })
        );
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn a_waiting_read_wakes_for_the_next_event_and_for_the_end() {
        let log = std::sync::Arc::new(Listening::default());
        let pushed = std::sync::Arc::clone(&log);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            pushed.push(line(5));
        });
        let heard = log.listen(0, Duration::from_secs(10)).await;
        assert_eq!(heard.events.len(), 1);
        let ended = std::sync::Arc::clone(&log);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            ended.close();
        });
        assert!(log.listen(heard.next, Duration::from_secs(10)).await.closed);
    }
}
