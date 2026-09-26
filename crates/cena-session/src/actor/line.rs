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

use std::sync::Arc;

use cena_platform::ByteSource;
use cena_protocol::Frame;
use cena_protocol::runs::Runs;

use super::SessionActor;

/// A finished line of game text, as the model completed it.
///
/// The runs, not a rendering of them: their styles, and the links a click
/// acts on and a trigger may name. The model keeps its own copy in the
/// stream's scrollback; this one is the viewers'.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// The stream the wire named; `""` is the main window.
    pub stream: String,
    /// The line, split where its style or link changes.
    pub runs: Runs,
}

impl Line {
    /// The text, markup removed.
    #[must_use]
    pub fn text(&self) -> String {
        self.runs.plain()
    }
}

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
        Some(Arc::new(Line {
            stream: text.stream.clone(),
            runs: runs.clone(),
        }))
    }
}
