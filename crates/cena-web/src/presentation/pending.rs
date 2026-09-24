//! Lines assembled from observed events between two publishes, fenced against
//! each native snapshot's cursor.

use super::hub::line_bytes;
use super::{MAX_DRAIN, MAX_HISTORY_BYTES, MAX_HISTORY_LINES};
use cena_session::{Event, Frame, Generation, ObservedEvent, Snapshot};
use cena_ui::{LineAssembler, StoryLine, StyledRun};
use std::collections::VecDeque;
use tokio::sync::broadcast;

pub(super) struct Pending {
    pub(super) assembler: LineAssembler,
    pub(super) lines: VecDeque<StoryLine>,
    pub(super) bytes: usize,
    pub(super) cursor: u64,
    pub(super) generation: Generation,
    pub(super) gap: bool,
    /// Inside a quiet command's window (`Event::Quiet`): its main-stream
    /// report stays out of the story. Other streams -- a thought, a death
    /// -- still show; they were not the command's.
    pub(super) quiet: bool,
}

impl Pending {
    pub(super) fn new(snapshot: &Snapshot) -> Self {
        Self {
            assembler: LineAssembler::default(),
            lines: VecDeque::new(),
            bytes: 0,
            cursor: snapshot.cursor,
            generation: snapshot.generation,
            gap: false,
            quiet: false,
        }
    }

    pub(super) fn observe(&mut self, event: ObservedEvent) {
        if event.cursor <= self.cursor {
            return;
        }
        self.cursor = event.cursor;
        if event.generation != self.generation {
            self.assembler.reset();
            self.generation = event.generation;
            // A window never outlives its connection.
            self.quiet = false;
        }
        if let Event::Quiet(quiet) = event.event {
            self.quiet = quiet;
            return;
        }
        if let Event::Frame(frame) = event.event {
            if self.quiet && is_report(&frame) {
                return;
            }
            let lines = frame_lines(&mut self.assembler, &frame);
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
    }

    pub(super) fn missing(&mut self) {
        self.gap = true;
        self.assembler.reset();
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
        if self.generation != snapshot.generation {
            self.assembler.reset();
        }
        self.cursor = snapshot.cursor;
        self.generation = snapshot.generation;
    }
}

/// Whether a frame inside a quiet window is part of the command's report:
/// main-stream text, and the prompt that ends it.
fn is_report(frame: &Frame) -> bool {
    match frame {
        Frame::Text(text) => text.stream.is_empty() || text.stream == "main",
        Frame::Prompt { .. } => true,
        _ => false,
    }
}

fn frame_lines(assembler: &mut LineAssembler, frame: &Frame) -> Vec<StoryLine> {
    match frame {
        Frame::Text(text) => assembler.push(
            &text.stream,
            &StyledRun {
                text: text.content.clone(),
                bold: text.style.bold_depth > 0,
                monospace: text.style.mono,
                preset: text.style.preset.clone(),
            },
            text.ends_line,
        ),
        Frame::Prompt { .. } => assembler.flush(),
        Frame::Component { id, body } => {
            let mut lines = assembler.flush();
            let count = body.runs.len();
            for (index, run) in body.runs.iter().enumerate() {
                lines.extend(assembler.push(
                    id,
                    &StyledRun {
                        text: run.text.clone(),
                        bold: run.style.bold_depth > 0,
                        monospace: run.style.mono,
                        preset: run.style.preset.clone(),
                    },
                    index + 1 == count,
                ));
            }
            lines
        }
        Frame::ClearStream { id } => {
            assembler.clear_stream(id);
            Vec::new()
        }
        _ => Vec::new(),
    }
}
