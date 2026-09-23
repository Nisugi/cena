//! Methods on [`Frame`](super::Frame).
//!
//! Split out of `frame.rs` under Rule 4.1 (`plan/05:352-353`) -- move code
//! down, do not raise the cap -- when [`Frame::EndSetup`](super::Frame::EndSetup)
//! took that file to its 500-line cap. The parent holds the vocabulary; this
//! holds what is said ABOUT a frame.

use super::Frame;

impl Frame {
    /// True for a frame that records a tag's presence without modelling it.
    ///
    /// Provided here rather than left to each caller because the drop-nothing
    /// rule creates this filter for **every** consumer at once -- a renderer,
    /// the replay differ, and a behavior all need the same predicate on the
    /// same day, which is the rule of three (`plan/05` §-1) satisfied at
    /// introduction rather than anticipated. It is one `matches!`, not a
    /// trait and not a config option.
    #[must_use]
    pub fn is_structural(&self) -> bool {
        matches!(self, Frame::Structural { .. })
    }

    /// A [`Frame::Structural`] for `name`, carrying `raw` verbatim.
    #[must_use]
    pub(crate) fn structural(name: &str, raw: &str) -> Self {
        Frame::Structural {
            name: name.to_owned(),
            raw: raw.to_owned(),
        }
    }
}
