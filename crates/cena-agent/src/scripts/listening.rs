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
//!
//! **A read that lagged begins with the whole copy.** A `state` carries only
//! the fields that changed, and the watcher does not send a field again
//! until it changes again; so a `state` let go for room left the runner's
//! copy wrong for good, a warning its only sign (the integrated crate review
//! of 2026-09-28, I5). The log keeps every field of every `state` that has
//! left it, read or let go, the last word winning: the copy as of the last
//! one gone. A read that lagged is answered with that first, as one `state`,
//! and the `state`s still kept after it apply over it in order.

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
    /// Every field of every `state` that has left the log, the last word
    /// winning, and the cursor of the last: the copy as of then.
    gone: serde_json::Map<String, serde_json::Value>,
    gone_cursor: u64,
}

impl Inner {
    /// `entry` leaves the log: a `state`'s fields are kept in [`Self::gone`].
    fn leaves(&mut self, entry: Entry) {
        if let Event::State { cursor, fields } = entry.event {
            self.gone.extend(fields);
            self.gone_cursor = cursor;
        }
    }
}

impl Listening {
    /// Add `event` at the next position.
    pub fn push(&self, event: Event) {
        let mut inner = self.lock();
        inner.last += 1;
        let at = inner.last;
        inner.entries.push_back(Entry { at, event });
        while inner.entries.len() > KEPT {
            // A built-in's ending stays with them: a script waits on it
            // with no limit (`Runs.wait` in the runner), a read that lagged
            // is given the copy again and never an ending let go, so one
            // dropped in a hunt's flood left `Script.run('go2', ...)` waiting
            // for the life of the runner (the review of 2026-09-29). There
            // is one for each built-in a script started.
            let Some(oldest) = inner.entries.iter().position(|entry| {
                !matches!(
                    entry.event,
                    Event::Typed { .. } | Event::Input { .. } | Event::Ended { .. }
                )
            }) else {
                break;
            };
            if let Some(dropped) = inner.entries.remove(oldest) {
                inner.dropped_through = inner.dropped_through.max(dropped.at);
                inner.leaves(dropped);
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
        // Read, or a typed line kept past the bound ahead of lines already
        // read: let go of each.
        let (read, kept): (VecDeque<Entry>, VecDeque<Entry>) = std::mem::take(&mut inner.entries)
            .into_iter()
            .partition(|entry| entry.at <= since);
        inner.entries = kept;
        for entry in read {
            inner.leaves(entry);
        }
        let lagged = since < inner.dropped_through;
        let mut events: Vec<Entry> = Vec::new();
        if lagged && !inner.gone.is_empty() {
            // The whole copy as of the last `state` gone, at the last
            // position let go: after `since`, before everything kept.
            events.push(Entry {
                at: inner.dropped_through,
                event: Event::State {
                    cursor: inner.gone_cursor,
                    fields: inner.gone.clone(),
                },
            });
        }
        let returned = RETURNED - events.len();
        events.extend(inner.entries.iter().take(returned).cloned());
        Heard {
            next: events.last().map_or(since, |last| last.at),
            lagged,
            closed: inner.closed && inner.entries.len() <= returned,
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

    /// A built-in's ending is never let go for room: the script that
    /// started it is waiting on it.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn a_flood_never_drops_a_built_ins_ending() {
        let ended = Event::Ended {
            run: 7,
            work: "completed".into(),
            reason: "arrived".into(),
            left: Vec::new(),
        };
        let log = Listening::default();
        log.push(ended.clone());
        for cursor in 0..KEPT as u64 + 10 {
            log.push(line(cursor));
        }
        let heard = log.listen(0, Duration::ZERO).await;
        assert!(heard.lagged, "lines went for room");
        assert!(
            heard.events.iter().any(|entry| entry.event == ended),
            "and the ending did not"
        );
    }

    fn state(cursor: u64, fields: &[(&str, serde_json::Value)]) -> Event {
        Event::State {
            cursor,
            fields: fields
                .iter()
                .map(|(name, value)| ((*name).to_owned(), value.clone()))
                .collect(),
        }
    }

    /// The review's case: a `state` let go for room, and the field it
    /// changed never changing again. The read that lagged begins with the
    /// whole copy -- that field among the ones read before -- and a `state`
    /// still kept applies over it.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn a_read_that_lagged_begins_with_the_whole_copy() {
        use serde_json::json;
        let log = Listening::default();
        log.push(state(
            1,
            &[("room", json!("Town Square")), ("hands", json!("empty"))],
        ));
        let first = log.listen(0, Duration::ZERO).await;
        assert_eq!(first.events.len(), 1);
        log.push(state(2, &[("room", json!("North Gate"))]));
        for cursor in 3..KEPT as u64 + 3 {
            log.push(line(cursor));
        }
        log.push(state(9_000, &[("hands", json!("a sword"))]));

        let heard = log.listen(first.next, Duration::ZERO).await;
        assert!(heard.lagged);
        let Some(Event::State { cursor, fields }) = heard.events.first().map(|e| &e.event) else {
            panic!(
                "a lagged read begins with the copy: {:?}",
                heard.events.first()
            );
        };
        assert_eq!(*cursor, 2);
        assert_eq!(fields["room"], json!("North Gate"), "the lost change");
        assert_eq!(fields["hands"], json!("empty"), "a field read before");
        assert!(
            heard.events.windows(2).all(|pair| pair[0].at < pair[1].at),
            "in order"
        );

        // Read to the end: the later `state` still comes, and after it the
        // log is caught up.
        let mut since = heard.next;
        let mut last_state = None;
        loop {
            let more = log.listen(since, Duration::ZERO).await;
            if more.events.is_empty() {
                assert!(!more.lagged, "once, not again");
                break;
            }
            for entry in &more.events {
                if let Event::State { fields, .. } = &entry.event {
                    last_state = Some(fields.clone());
                }
            }
            since = more.next;
        }
        assert_eq!(last_state.unwrap()["hands"], json!("a sword"));
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
