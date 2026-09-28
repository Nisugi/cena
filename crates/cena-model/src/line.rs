//! A finished line of game text, as a viewer is given it (`plan/45` §4a).
//!
//! The session publishes one for each line the model finishes, once, to
//! every viewer. It is the model's own line -- [`Unfinished`] is the one place
//! a frame boundary becomes a line boundary -- so a viewer draws what the
//! classifiers and the player log read. What a character's triggers do to it is applied before it is
//! published ([`Matcher::respond`](crate::trigger::Matcher::respond)): the
//! model's scrollback and the log keep the game's text, and the line a viewer
//! gets may be substituted, moved to another stream, or painted.

use std::collections::BTreeMap;

use cena_protocol::frame::TextFrame;
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

/// The lines still being put together, one per stream: **the one place a
/// frame boundary becomes a line boundary.**
///
/// The parser emits one run per markup boundary, so `  a` +
/// `<a>pebbled grey leather doublet</a>` is two frames of one line -- the
/// split that printed the author's worn inventory down the screen.
/// [`TextFrame::ends_line`] is what says where a line really ends, and this
/// holds the runs until it does. Keyed by stream because two streams can be
/// mid-line at once: a `pushStream` can interrupt an unterminated run and the
/// enclosing stream resumes afterwards.
///
/// The model's lines are put together here (`state/streams.rs`), and so is
/// the text of a player's Lich, which the session shows in their place
/// (`plan/51` step 3): both end a line where the other would.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Unfinished(BTreeMap<String, Runs>);

impl Unfinished {
    /// Add `text` to its stream's line: that line's runs, once `text` ends
    /// it.
    pub fn push(&mut self, text: &TextFrame) -> Option<Runs> {
        let line = self.0.entry(text.stream.clone()).or_default();
        line.runs.push(text.as_run());
        text.ends_line.then(|| std::mem::take(line))
    }

    /// Forget one stream's unfinished line: the game cleared the stream.
    pub fn clear_stream(&mut self, stream: &str) {
        self.0.remove(stream);
    }

    /// Forget every unfinished line: half a sentence nobody will finish.
    pub fn clear(&mut self) {
        self.0.clear();
    }
}
