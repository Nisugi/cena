//! A finished line of game text, as a viewer is given it (`plan/45` §4a).
//!
//! The session publishes one for each line the model finishes, once, to
//! every viewer. It is the model's own line -- `route_text` in
//! `state/streams.rs` is the one place a frame boundary becomes a line
//! boundary -- so a viewer draws what the classifiers and the player log
//! read. What a character's triggers do to it is applied before it is
//! published ([`Matcher::respond`](crate::trigger::Matcher::respond)): the
//! model's scrollback and the log keep the game's text, and the line a viewer
//! gets may be substituted, moved to another stream, or painted.

use cena_protocol::runs::Runs;

use crate::trigger::Paint;

/// A finished line of game text, as a viewer is given it.
///
/// The runs, not a rendering of them: their styles, and the links a click
/// acts on and a trigger may name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// The stream it is shown on; `""` is the main window. The wire's, unless
    /// a trigger redirected it.
    pub stream: String,
    /// The line, split where its style or link changes.
    pub runs: Runs,
    /// The looks its triggers gave it, over bytes of [`Line::text`]: in order,
    /// never overlapping, each setting something.
    pub paint: Vec<Paint>,
}

impl Line {
    /// A line as the game sent it, unpainted.
    #[must_use]
    pub fn new(stream: impl Into<String>, runs: Runs) -> Self {
        Self {
            stream: stream.into(),
            runs,
            paint: Vec::new(),
        }
    }

    /// The text, markup removed.
    #[must_use]
    pub fn text(&self) -> String {
        self.runs.plain()
    }
}
