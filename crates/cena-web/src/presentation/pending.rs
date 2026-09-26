//! Lines drawn from observed events between two publishes, fenced against
//! each native snapshot's cursor.
//!
//! The session publishes each finished line ([`Event::Line`]), the model's
//! own, so this draws those rather than assembling lines from text frames
//! (`plan/45` §4a). It keeps no partial line: the model does, and clears it
//! on a reconnect. Room components are not lines in the model, so their
//! bodies are still drawn from their frames here.

use super::hub::line_bytes;
use super::{MAX_DRAIN, MAX_HISTORY_BYTES, MAX_HISTORY_LINES};
use cena_session::{Event, Frame, Generation, Line, LinkKind, ObservedEvent, Run, Snapshot};
use cena_ui::{StoryLine, StyledRun, story_lines};
use std::collections::VecDeque;
use tokio::sync::broadcast;

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
    /// `;sorter` as the player last set it (`Sessions::sort_containers`).
    pub(super) sorting: bool,
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
            sorting: false,
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
            Event::Line(line) if self.quiet && is_main(&line.stream) => return,
            Event::Line(line) => drawn(&line, self.sorting),
            Event::Frame(frame) => match *frame {
                Frame::Component { id, body } => {
                    story_lines(&id, body.runs.iter().map(piece), false)
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

    pub(super) fn missing(&mut self) {
        self.gap = true;
    }

    pub(super) fn fence(
        &mut self,
        snapshot: &Snapshot,
        old: &mut broadcast::Receiver<ObservedEvent>,
    ) {
        // Events through this fence were published before subscribe answered.
        // Never await a missing event or drain beyond this fixed budget.
        for _ in 0..MAX_DRAIN {
            if self.cursor >= snapshot.cursor {
                break;
            }
            match old.try_recv() {
                Ok(event) if event.cursor <= snapshot.cursor => self.observe(event),
                _ => {
                    self.missing();
                    break;
                }
            }
        }
        if self.cursor < snapshot.cursor {
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

/// A published line as the story lines this viewer draws.
fn drawn(line: &Line, sorting: bool) -> Vec<StoryLine> {
    story_lines(&line.stream, line.runs.runs.iter().map(piece), sorting)
}

/// One run as the story draws it, with the noun of the object it names:
/// what `;sorter` reads.
fn piece(run: &Run) -> (StyledRun, Option<String>) {
    let noun = run.object().and_then(|link| match &link.kind {
        LinkKind::Exist { noun, .. } => Some(noun.clone()),
        _ => None,
    });
    let styled = StyledRun {
        text: run.text.clone(),
        bold: run.style.bold_depth > 0,
        monospace: run.style.mono,
        preset: run.style.preset.clone(),
    };
    (styled, noun)
}
