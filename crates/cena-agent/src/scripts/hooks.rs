//! A runner's hooks (`plan/46` §6.1; `SCRIPTS.md`, `hooks`): which kinds it
//! has, told to the session through the script door, and the lines the
//! player typed that its input hooks were asked about and have not
//! answered.
//!
//! The display hooks answer the lines a runner already hears, by their
//! cursors, so they need nothing here beyond the switch. The input hooks are
//! asked about a line the session has not sent yet: it goes to the runner as
//! an `input` event, pushed straight onto its log as a `typed` line is, and
//! the session waits on the answer (`cena_session::script::Asker`).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use cena_session::script::Asker;
use tokio::sync::oneshot;

use super::Seat;
use super::listening::{Event, Listening};

/// A runner's hooks.
#[derive(Debug, Default)]
pub(super) struct Hooking {
    /// It has display hooks.
    display: bool,
    /// It has input hooks.
    input: bool,
    /// The last typed line's number.
    asked: u64,
    /// Typed lines asked about, by number, and where the answer goes.
    waiting: BTreeMap<u64, oneshot::Sender<Option<String>>>,
}

impl Seat {
    /// The runner has display hooks, input hooks, both or neither now.
    pub(super) fn hook(&self, display: bool, input: bool) {
        let mut hooking = lock(&self.hooking);
        if hooking.display != display {
            hooking.display = display;
            self.door.hook_lines(display);
        }
        if hooking.input != input {
            hooking.input = input;
            let asker = input.then(|| asker(&self.listening, &self.hooking));
            self.door.hook_typing(asker);
            if !input {
                // Dropped: each line still waiting goes as typed.
                hooking.waiting.clear();
            }
        }
    }

    /// The input hooks' answer about typed line `asked`: `false` when it
    /// was not waiting, answered already or given up on.
    pub(super) fn typed_answer(&self, asked: u64, line: Option<String>) -> bool {
        let waiting = lock(&self.hooking).waiting.remove(&asked);
        waiting.is_some_and(|answer| answer.send(line).is_ok())
    }
}

/// Ask the runner whose log is `listening` about each line the player types.
fn asker(listening: &Arc<Listening>, hooking: &Arc<Mutex<Hooking>>) -> Asker {
    let (listening, hooking) = (Arc::clone(listening), Arc::clone(hooking));
    Arc::new(move |line: &str| {
        let (answer, answered) = oneshot::channel();
        let asked = {
            let mut hooking = lock(&hooking);
            // A question given up on is no longer waited for.
            hooking.waiting.retain(|_, answer| !answer.is_closed());
            hooking.asked += 1;
            let asked = hooking.asked;
            hooking.waiting.insert(asked, answer);
            asked
        };
        listening.push(Event::Input {
            asked,
            line: line.to_owned(),
        });
        answered
    })
}

fn lock(hooking: &Mutex<Hooking>) -> MutexGuard<'_, Hooking> {
    hooking.lock().unwrap_or_else(PoisonError::into_inner)
}
