//! Lines drawn from observed events between two publishes, fenced against
//! each native snapshot's cursor.
//!
//! The session publishes each finished line ([`Event::Line`]), the model's
//! own, so this draws those rather than assembling lines from text frames
//! (`plan/45` §4a). It keeps no partial line: the model does, and clears it
//! on a reconnect. It sorts nothing and matches nothing: with `;sorter` on,
//! the session publishes a container look already sorted, and a line comes
//! already answered by the character's triggers, its paint resolved
//! ([`painted`]). Room components are not lines in the model, so their bodies
//! are still drawn from their frames here, unpainted. A trigger's banner
//! ([`Event::Attention`]'s `alert`) is kept here too, for the next publish to
//! send after its lines, the last [`MAX_ALERTS`] of them.

use super::hub::line_bytes;
use super::{MAX_HISTORY_BYTES, MAX_HISTORY_LINES};
use cena_session::observation::catch_up;
use cena_session::{Event, Frame, Generation, Line, ObservedEvent, Snapshot};
use cena_ui::{StoryLine, painted, story_lines};
use std::collections::VecDeque;
use tokio::sync::broadcast;

/// The banners kept between two publishes, newest kept: a viewer shows no
/// more at once (`VellumFE`'s `MAX_CONCURRENT`).
pub(super) const MAX_ALERTS: usize = 5;

pub(super) struct Pending {
    pub(super) lines: VecDeque<StoryLine>,
    pub(super) bytes: usize,
    pub(super) cursor: u64,
    pub(super) generation: Generation,
    pub(super) gap: bool,
    /// Inside a quiet command's window (`Event::Quiet`): its main-stream
    /// report stays out of the story. Other streams -- a thought, a death
    /// -- still show; they were not the command's.
    pub(super) quiet: bool,
    /// Banners since the last publish, oldest first.
    pub(super) alerts: Vec<String>,
}

impl Pending {
    pub(super) fn new(snapshot: &Snapshot) -> Self {
        Self {
            lines: VecDeque::new(),
            bytes: 0,
            cursor: snapshot.cursor,
            generation: snapshot.generation,
            gap: false,
            quiet: false,
            alerts: Vec::new(),
        }
    }

    pub(super) fn observe(&mut self, event: ObservedEvent) {
        if event.cursor <= self.cursor {
            return;
        }
        self.cursor = event.cursor;
        if event.generation != self.generation {
            self.generation = event.generation;
            // A window never outlives its connection.
            self.quiet = false;
        }
        let lines = match event.event {
            Event::Quiet(quiet) => {
                self.quiet = quiet;
                return;
            }
            Event::Attention(call) => {
                if let Some(text) = &call.alert {
                    self.alerts.push(text.clone());
                    let over = self.alerts.len().saturating_sub(MAX_ALERTS);
                    self.alerts.drain(..over);
                }
                return;
            }
            Event::Line(line) if self.quiet && is_main(&line.stream) => return,
            Event::Line(line) => story_lines(&line.stream, painted(&line)),
            Event::Frame(frame) => match *frame {
                Frame::Component { id, body } => {
                    let body = Line::new(id, body);
                    story_lines(&body.stream, painted(&body))
                }
                _ => return,
            },
            _ => return,
        };
        for line in lines {
            self.bytes += line_bytes(&line);
            self.lines.push_back(line);
            while self.lines.len() > MAX_HISTORY_LINES || self.bytes > MAX_HISTORY_BYTES {
                if let Some(old) = self.lines.pop_front() {
                    self.bytes -= line_bytes(&old);
                    self.gap = true;
                }
            }
        }
    }

    /// Events were lost: mark the place, and end a quiet window, whose end
    /// may be among them. The snapshot does not carry the quiet state, and a
    /// story shown too much beats one silent for good (the integrated crate
    /// review of 2026-09-28, I6), as the GUI's story does (`Story::missed`).
    pub(super) fn missing(&mut self) {
        self.gap = true;
        self.quiet = false;
    }

    pub(super) fn fence(
        &mut self,
        snapshot: &Snapshot,
        old: &mut broadcast::Receiver<ObservedEvent>,
    ) {
        // Events through this fence were published before subscribe answered.
        let (events, whole) = catch_up(old, self.cursor, snapshot.cursor);
        for event in events {
            self.observe(event);
        }
        if !whole {
            self.missing();
        }
        self.cursor = snapshot.cursor;
        self.generation = snapshot.generation;
    }
}

/// Whether a line inside a quiet window is part of the command's report:
/// main-stream text.
fn is_main(stream: &str) -> bool {
    stream.is_empty() || stream == "main"
}
