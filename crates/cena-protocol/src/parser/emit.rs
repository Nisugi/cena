//! Emitting text: flushing the buffer into frames, and parsing a component
//! body into runs.
//!
//! Split out of `parser.rs` under Rule 4.1 (`plan/05:352-353`) -- move code
//! down, do not raise the cap. It holds [`Parser::parse_runs`], which is the
//! Rule 2.1 fix: a component body reaches the layer above as structured text
//! and links, never as a markup string.
//!
//! This file was `capture.rs` and also held the multi-line capture path. That
//! path is gone -- see MULTI-LINE CAPTURES in `parser.rs` for the measurement
//! (0 occurrences in 1,230,355 wire lines, against a 4,297-line silent-loss
//! window) -- so the file is named for what remains.

use super::Parser;
use crate::frame::{Frame, Style, TextFrame};
use crate::runs::{Run, Runs};
use crate::text;

impl Parser {
    /// Mark the **last** text run of a finished line as ending it.
    ///
    /// Called once per `parse_line`, which is what makes the invariant a single
    /// line of code: a line's runs carry `false` until exactly one is promoted.
    /// A line whose frames are all non-text -- a bare `<prompt>`, an indicator --
    /// promotes nothing, which is correct: it contributed no display text, so
    /// there is no run for a printer to terminate.
    pub(super) fn mark_line_end(frames: &mut [Frame]) {
        if let Some(Frame::Text(text)) = frames
            .iter_mut()
            .rev()
            .find(|frame| matches!(frame, Frame::Text(_)))
        {
            text.ends_line = true;
        }
    }

    /// Emit the buffered text, if any, as a [`Frame::Text`].
    pub(super) fn flush(&mut self, buffer: &mut String, frames: &mut Vec<Frame>) {
        if buffer.is_empty() {
            return;
        }
        let content = std::mem::take(buffer);
        frames.push(self.text_frame(&content));
    }

    /// Wrap display text in the markup state currently open.
    pub(super) fn text_frame(&mut self, content: &str) -> Frame {
        let content = text::strip_control_chars(&text::decode_entities(content));
        // Link text accumulates on the outermost open link, which is the one
        // that surfaces -- Vellum's rule at src/parser/text.rs:97-106.
        if let Some(link) = self.links.first_mut() {
            link.text.push_str(&content);
        }
        Frame::Text(TextFrame {
            content,
            stream: self.current_stream(),
            style: self.style(),
            link: self.links.first().cloned(),
            // Set by `mark_line_end` once the line is fully parsed: a run
            // cannot know here whether another follows it.
            ends_line: false,
        })
    }

    /// The markup open right now.
    fn style(&self) -> Style {
        Style {
            bold_depth: self.bold_depth,
            preset: self.presets.last().cloned(),
            mono: self.mono,
        }
    }

    /// The stream text currently belongs to; `""` is the main window.
    fn current_stream(&self) -> String {
        self.streams.last().cloned().unwrap_or_default()
    }

    /// Parse a component body into structured runs.
    ///
    /// This is the Rule 2.1 fix: the layer above receives text and links, not
    /// the markup string Vellum hands it (`src/parser.rs:803-832`).
    ///
    /// Markup state is saved and restored around the body, so an unbalanced
    /// `<pushBold>` inside a component -- which the corpus does contain -- does
    /// not leak bold onto the lines that follow it. Vellum earned roughly ten
    /// regression tests for exactly this class of leak
    /// (`src/parser/tests.rs:654,755,801,897`); recomputing from a saved
    /// snapshot removes the class rather than re-earning the tests.
    pub(super) fn parse_runs(&mut self, body: &str) -> Runs {
        let saved_bold = self.bold_depth;
        let saved_presets = std::mem::take(&mut self.presets);
        let saved_links = std::mem::take(&mut self.links);

        let mut runs = Vec::new();
        let mut buffer = String::new();
        let mut rest = body;
        while !rest.is_empty() {
            let Some(start) = text::find_tag_start(rest) else {
                buffer.push_str(rest);
                break;
            };
            buffer.push_str(&rest[..start]);
            let tail = &rest[start..];
            let Some(close) = tail.find('>') else {
                buffer.push_str(tail);
                break;
            };
            let tag = &tail[..=close];
            rest = &tail[close + 1..];
            self.push_run(&mut buffer, &mut runs);
            // `markup_state`, not `markup_tag`: a component body becomes
            // `Runs`, where the markup's effect is already carried by each
            // run's own `style` and `link`. There is no frame vector here for
            // a `Structural` to go into, and the body's raw bytes are already
            // recoverable from the `Component` frame that encloses it.
            self.markup_state(tag);
        }
        self.push_run(&mut buffer, &mut runs);

        self.bold_depth = saved_bold;
        self.presets = saved_presets;
        self.links = saved_links;
        Runs { runs }
    }

    /// Emit one run of a component body, if there is text pending.
    pub(super) fn push_run(&mut self, buffer: &mut String, runs: &mut Vec<Run>) {
        if buffer.is_empty() {
            return;
        }
        let content = text::strip_control_chars(&text::decode_entities(buffer));
        buffer.clear();
        if let Some(link) = self.links.first_mut() {
            link.text.push_str(&content);
        }
        runs.push(Run {
            text: content,
            style: self.style(),
            link: self.links.first().cloned(),
        });
    }
}
