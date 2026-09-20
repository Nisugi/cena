//! Per-stream text buffers: **M2 step 1**, `plan/18` §2a.
//!
//! Split out of `state.rs` beside [`clock`](super::clock),
//! [`idle`](super::idle) and [`reconnect`](super::reconnect), under Rule 4.1
//! (`plan/05:352-353`) -- move code down, do not raise the cap.
//!
//! # What this is not
//!
//! **Not a window manager.** `plan/18` §2a: a map of buffers and the routing
//! rule; layout is `cena-ui`'s, at M4. `VellumFE` mixes the two -- its stream
//! handling consults widget subscribers, a route map, a discard list and TTS
//! (`core/messages/element.rs:410-470`) -- and every one of those is a frontend
//! concern that would make this crate know about windows.
//!
//! # The routing rule: read it off the frame
//!
//! [`TextFrame::stream`](cena_protocol::frame::TextFrame) is stamped by the
//! parser on every run, so there is no `current_stream` cursor here to keep in
//! sync. `VellumFE` carries one and has to fix it up on pop, on resume and on
//! prompt -- three places that can disagree, and its pop sets it to `"main"`
//! unconditionally before `StreamResume` corrects it. Reading it off the frame is
//! the same routing with the state removed.
//!
//! So [`StreamPush`](cena_protocol::Frame::StreamPush),
//! [`StreamPop`](cena_protocol::Frame::StreamPop),
//! [`StreamResume`](cena_protocol::Frame::StreamResume) and the prompt barrier
//! need **no handling here at all**: the parser has already accounted for them by
//! the time a `Frame::Text` carries its stream. They are still published, for a
//! renderer that wants to know a window opened.
//!
//! # What clears a buffer: `clearStream`, and nothing else
//!
//! MEASURED over 16 files across 4 characters: of **2,722** `pushStream` tags,
//! **2,721** are immediately preceded by a `clearStream` of the same id. The
//! single exception is a `thoughts` push -- a live feed, where clearing would
//! discard the session's chatter.
//!
//! `VellumFE` clears `inv` and `reserve` on PUSH
//! (`core/messages/element.rs:451-464`) and then needs exceptions: its
//! `perception` buffer is *"NOT cleared on pushStream"* and its `sprite`
//! component is exempt from an unchanged-check because *"the game sends it EMPTY
//! on every room change"*. Honouring the wire's own clear needs none of that: it
//! gets snapshot semantics for `room` and `inv` because the wire sends the clear,
//! and accumulation for `thoughts` because it does not.
//!
//! Evidence and the full census are in
//! `crates/cena-model/tests/stream_routing.rs`.

use super::GameState;
use cena_protocol::frame::TextFrame;
use cena_protocol::runs::Runs;

/// Buffered lines per stream id; `""` is the main window.
///
/// `BTreeMap`, not `HashMap`, for the reason
/// [`Vitals`](super::Vitals) already gives: criterion 7 replays this state, and
/// a `HashMap`'s iteration order varies run to run.
pub type StreamBuffers = std::collections::BTreeMap<String, Vec<Runs>>;

impl GameState {
    /// One stream's buffered lines, oldest first.
    ///
    /// An unseen stream is **empty rather than absent**: there is nothing worth
    /// modelling between "declared and empty" and "never mentioned", because both
    /// render as nothing. That also keeps a caller from unwrapping an `Option` at
    /// every call site to ask a question with one sensible answer.
    #[must_use]
    pub fn stream(&self, id: &str) -> &[Runs] {
        self.streams.get(id).map_or(&[], Vec::as_slice)
    }

    /// Every stream that has received text, in id order.
    ///
    /// A `streamWindow` declaration does **not** appear here. MEASURED: 16
    /// distinct `streamWindow` ids against 6 ever pushed to, so treating a
    /// declaration as a buffer would invent ten empty ones per login. A
    /// declaration is layout information, and layout is `cena-ui`'s.
    pub fn streams(&self) -> impl Iterator<Item = (&str, &[Runs])> {
        self.streams
            .iter()
            .map(|(id, lines)| (id.as_str(), lines.as_slice()))
    }

    /// Route one text run to its stream, completing a line when it ends one.
    ///
    /// **A frame boundary is not a line boundary.** The parser emits one run per
    /// markup boundary, so a single displayed line arrives as several frames;
    /// `ends_line` is what says which frame finishes it. Appending each run as
    /// its own line is the bug that printed the author's worn inventory as
    /// `  a` / `pebbled grey leather doublet` down the screen.
    ///
    /// The pending line is keyed by stream because two can be mid-line at once: a
    /// `pushStream` may interrupt an unterminated run, and the enclosing stream
    /// resumes after the pop.
    pub(super) fn route_text(&mut self, text: &TextFrame) {
        let pending = self.pending.entry(text.stream.clone()).or_default();
        pending.runs.push(text.as_run());
        if text.ends_line {
            let line = std::mem::take(pending);
            // **The chunk sees the line too, and only the main stream's.**
            // A report's output is prose in the main window; a `thoughts` or
            // `bounty` stream carries someone else's words and must not become
            // part of the command's answer. Lich gates the same way, by
            // refusing lines while `XMLData.in_stream` is true
            // (`combat/tracker.rb:481`).
            if text.stream.is_empty() {
                self.chunk.push_line(super::chunks::ChunkLine {
                    text: line.plain(),
                    bold: line.bold_fragments(),
                });
            }
            self.streams
                .entry(text.stream.clone())
                .or_default()
                .push(line);
        }
    }

    /// `<clearStream id=>`: drop one stream's buffer.
    ///
    /// The **only** thing that empties a buffer. Named streams only: a clear
    /// says nothing about any stream but its own, which is the same per-category
    /// rule `ClearDialogData` follows for effects.
    ///
    /// Removes the entry rather than emptying it, so [`Self::streams`] lists what
    /// has content instead of every id ever cleared.
    pub(super) fn clear_stream(&mut self, id: &str) {
        self.streams.remove(id);
    }
}
