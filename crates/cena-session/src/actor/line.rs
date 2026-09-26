//! A finished line leaves the model here: once, to every viewer.
//!
//! `route_text` is the one place a frame boundary becomes a line boundary
//! (`cena-model`'s `state/streams.rs`), and `lines_seen` moving is how it says
//! it just made one. The player log already took its line from that point;
//! this publishes the same line as [`Event::Line`](super::Event::Line), so a
//! viewer draws the line the classifiers and the log read, rather than
//! assembling a second one from frames (`plan/45` §4a).
//!
//! It is also the point M8's triggers run at (`plan/45` §4): once per line, in
//! the session, before any viewer, so every viewer and a session nobody
//! watches agree on what a line said and what it set off.
//!
//! The line itself is `cena-model`'s ([`Line`]), since the model is what
//! makes it and what the triggers answer.

use std::sync::Arc;

use cena_model::line::Line;
use cena_platform::ByteSource;
use cena_protocol::Frame;

use super::{Event, SessionActor};

impl<S: ByteSource> SessionActor<S> {
    /// The line `frame` finished, if it finished one.
    ///
    /// `before` is [`lines_seen`](cena_model::GameState::lines_seen) read
    /// before the frame was applied. Only a text frame with `ends_line` moves
    /// it, and it moves by one, so the line is the last in its stream.
    pub(super) fn finished_line(&self, frame: &Frame, before: u64) -> Option<Arc<Line>> {
        let Frame::Text(text) = frame else {
            return None;
        };
        if self.state.lines_seen() == before {
            return None;
        }
        let runs = self.state.stream(&text.stream).last()?;
        Some(Arc::new(Line::new(text.stream.clone(), runs.clone())))
    }

    /// Publish a finished line to every viewer: as it came, or, with
    /// `;sorter` on and a main-stream container look, as the lines it sorts
    /// into (`cena_model::sorter`), one [`Event::Line`] each.
    ///
    /// Sorting here and not in a viewer is what lets M8's triggers match each
    /// sorted line, as `VellumFE` sorts before it highlights (`plan/45` §4a).
    /// The model's scrollback and the player log keep the look whole.
    pub(super) fn publish_line(&self, line: Arc<Line>) {
        let main = line.stream.is_empty() || line.stream == "main";
        if main
            && self.events.sorts_containers()
            && let Some(sorted) = cena_model::sorter::sort(&line.runs)
        {
            for runs in sorted {
                let sorted = Line::new(line.stream.clone(), runs);
                let _ = self.events.send(Event::Line(Arc::new(sorted)));
            }
            return;
        }
        let _ = self.events.send(Event::Line(line));
    }
}
