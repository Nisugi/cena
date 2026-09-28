//! Lines held for a script runner's display hooks (`plan/46` §6.1,
//! `crate::script`).
//!
//! While a runner has display hooks, what a viewer is shown of each line --
//! after `;sorter` and the triggers, as it would have been published -- waits
//! here for the runner to answer the line as the game sent it (the
//! [`Event::Heard`](super::Event::Heard) it read), and is shown in order:
//! as it is, hidden, or as the hooks changed it. A line the runner has not
//! answered by [`HOOK_DEADLINE`] goes as it came, and everything goes when
//! the hooks go or the connection ends. Nothing is lost for a hook: a line
//! shown a little late on a hooked character, never one dropped.
//!
//! **What waits is the showing alone.** The model has the line, the log
//! wrote it, the triggers' flags, attention and sends went when it arrived,
//! and every script read it: Lich's rule, that hooks run after the scripts
//! and the state have the line (`inventory/13` §1.6), so nothing a behavior
//! sees depends on a script.
//!
//! **A line the hooks change is answered by the player's triggers again**, as
//! Wrayth's highlights answer what Lich's hooks let through: its looks, a
//! substitution, a squelch. Only the showing: the triggers already fired on
//! the line the game sent. A line the triggers squelched stays squelched.

use std::collections::VecDeque;
use std::sync::Arc;

use cena_model::line::Line;
use cena_platform::ByteSource;
use cena_protocol::frame::Style;
use cena_protocol::runs::{Run, Runs};
use tokio::time::Instant;

use super::{Event, SessionActor};
use crate::script::{HOOK_DEADLINE, Hooks};

/// The lines not yet shown, oldest first.
#[derive(Debug, Default)]
pub(super) struct Held {
    waiting: VecDeque<Waiting>,
}

/// One line the game sent, waiting for the hooks' answer.
#[derive(Debug)]
struct Waiting {
    /// Where its [`Event::Heard`] was published: what the runner answers by.
    cursor: u64,
    /// The line as the game sent it.
    heard: Arc<Line>,
    /// What a viewer is shown of it unless the hooks change it.
    shown: Vec<Arc<Line>>,
    /// When it goes as it came.
    due: Instant,
    /// What the hooks said of it.
    answer: Answer,
}

/// What the hooks said of a line.
#[derive(Debug)]
enum Answer {
    /// Nothing yet: at the deadline it goes as it came.
    Awaited,
    /// Hide it.
    Hidden,
    /// Show this: the line's own text shows it as it came.
    Text(String),
}

impl Held {
    /// Whether any line waits.
    pub(super) fn is_waiting(&self) -> bool {
        !self.waiting.is_empty()
    }

    /// When the oldest line goes unanswered; an hour off when none waits,
    /// so the loop's arm always has an instant to name.
    pub(super) fn due(&self) -> Instant {
        self.waiting.front().map_or_else(
            || Instant::now() + std::time::Duration::from_hours(1),
            |oldest| oldest.due,
        )
    }
}

/// Wait until the hooks answered something, or the oldest line is `due`.
pub(super) async fn wake(hooks: &Hooks, due: Instant) {
    tokio::select! {
        () = hooks.answered() => {}
        () = tokio::time::sleep_until(due) => {}
    }
}

impl<S: ByteSource> SessionActor<S> {
    /// Hold `shown`, what a viewer is shown of `heard` (published as heard at
    /// `cursor`), for the hooks.
    pub(super) fn hold(&mut self, cursor: u64, heard: Arc<Line>, shown: Vec<Arc<Line>>) {
        self.held.waiting.push_back(Waiting {
            cursor,
            heard,
            shown,
            due: Instant::now() + HOOK_DEADLINE,
            answer: Answer::Awaited,
        });
    }

    /// Show what may be shown now, in order: each line the hooks answered,
    /// each past its deadline as it came, and with `all`, or once the hooks
    /// are gone, everything.
    pub(super) fn show_held(&mut self, all: bool) {
        let hooks = self.events.hooks();
        for (cursor, answer) in hooks.take_answers() {
            if let Some(waiting) = self.held.waiting.iter_mut().find(|w| w.cursor == cursor) {
                waiting.answer = answer.map_or(Answer::Hidden, Answer::Text);
            }
        }
        let (all, now) = (all || !hooks.display(), Instant::now());
        while let Some(oldest) = self.held.waiting.front() {
            if !all && matches!(oldest.answer, Answer::Awaited) && oldest.due > now {
                break;
            }
            let Some(waiting) = self.held.waiting.pop_front() else {
                break;
            };
            for line in self.answered(waiting) {
                let _ = self.events.send(Event::Line(line));
            }
        }
    }

    /// What a viewer is shown of a line, the hooks having answered it or
    /// not.
    fn answered(&self, waiting: Waiting) -> Vec<Arc<Line>> {
        let text = match waiting.answer {
            Answer::Awaited => return waiting.shown,
            Answer::Hidden => return Vec::new(),
            Answer::Text(text) => text,
        };
        if waiting.shown.is_empty() || text.is_empty() {
            return Vec::new();
        }
        if text == waiting.heard.text() {
            return waiting.shown;
        }
        let triggers = self.events.triggers();
        text.split('\n')
            .map(|piece| piece.strip_suffix('\r').unwrap_or(piece))
            .flat_map(|piece| {
                let line = Line::new(
                    waiting.heard.stream.clone(),
                    Runs {
                        runs: vec![Run {
                            text: piece.to_owned(),
                            style: Style::default(),
                            link: None,
                            inner_link: None,
                        }],
                    },
                );
                triggers.answer(&line, &self.state).lines
            })
            .map(Arc::new)
            .collect()
    }
}
