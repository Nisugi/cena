//! Per-stream text buffers: **M2 step 1**, `plan/18` §2a.
//!
//! Split out of `state.rs` beside `clock`, `idle` and `reconnect` (private
//! siblings, so named rather than linked), under Rule 4.1
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

/// How many completed lines one stream retains before the oldest are dropped.
///
/// **A scrollback depth, not a protocol fact.** Lich's combat tracker caps its
/// analysis buffer at 200 (`chunks.rs` ports that number for the same reason),
/// but this is display history rather than one command's output, so it is
/// larger: 2,000 lines is roughly what a terminal scrollback holds and well
/// past what any classifier looks back over.
///
/// Fixed rather than configurable, per Rule -1: no config option with one
/// value. When a frontend needs a different depth it will say so, and that is
/// the moment to make it a parameter.
pub const MAX_STREAM_LINES: usize = 2_000;

/// How many completed lines were routed, and how many the cap discarded.
///
/// **Seen is not retained**, and the difference is why both are counted: *"the
/// model never lags however far behind a subscriber falls"* is a claim about
/// being FED, which a retained-line count cannot express once a cap starts
/// dropping. `event_ring.rs` asserts the first and used to measure the second.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LineTally {
    /// Completed lines ever routed.
    pub seen: u64,
    /// Lines discarded to [`MAX_STREAM_LINES`].
    pub dropped: u64,
}

use super::GameState;
use cena_protocol::frame::TextFrame;
use cena_protocol::runs::Runs;

/// Buffered lines per stream id; `""` is the main window.
///
/// `BTreeMap`, not `HashMap`, for the reason
/// [`Vitals`](super::Vitals) already gives: criterion 7 replays this state, and
/// a `HashMap`'s iteration order varies run to run.
///
/// Each buffer is a `Ring` (`state/ring.rs`), not a `Vec`: at
/// [`MAX_STREAM_LINES`] the old `Vec::remove(0)` shifted 1,999 lines to drop
/// one, on every line the main window received (review). `pub(crate)` since
/// then, because the ring is; nothing outside the crate named this alias.
pub(crate) type StreamBuffers = std::collections::BTreeMap<String, super::ring::Ring<Runs>>;

impl GameState {
    /// How many completed lines this session has ever routed.
    ///
    /// **Counts what the model was FED, not what it kept.** The two diverge
    /// once [`MAX_STREAM_LINES`] starts dropping.
    #[must_use]
    pub const fn lines_seen(&self) -> u64 {
        self.tally.seen
    }

    /// How many lines the scrollback cap has discarded.
    ///
    /// Reported rather than silent, the rule `Chunk::dropped` follows: a
    /// consumer may want to know its history is incomplete, and Rule 2.2's
    /// floor is that nothing is dropped without saying so.
    #[must_use]
    pub const fn lines_dropped(&self) -> u64 {
        self.tally.dropped
    }

    /// One stream's buffered lines, oldest first.
    ///
    /// An unseen stream is **empty rather than absent**: there is nothing worth
    /// modelling between "declared and empty" and "never mentioned", because both
    /// render as nothing. That also keeps a caller from unwrapping an `Option` at
    /// every call site to ask a question with one sensible answer.
    #[must_use]
    pub fn stream(&self, id: &str) -> &[Runs] {
        self.streams
            .get(id)
            .map_or(&[], super::ring::Ring::as_slice)
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

    /// What each stream declared it does when its window is **closed**.
    ///
    /// The other half of the note above: a `streamWindow` declaration is not a
    /// buffer, but it is not nothing either. It says whether a closed window's
    /// text falls through to main, falls through styled, is re-routed, or is a
    /// duplicate to be dropped -- which is what stops `speech` rendering twice.
    ///
    /// See [`stream_windows`](super::stream_windows) for the rule and the
    /// census behind it.
    #[must_use]
    pub const fn stream_windows(&self) -> &super::stream_windows::Windows {
        &self.stream_windows
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
                // The runs themselves, not a rendering of them: the links are
                // what a combat consumer reads, and this used to drop them
                // (`chunks.rs`, CORRECTED 2026-09-20).
                //
                // **The one deep copy on this path, and it is not waste.** The
                // chunk and the scrollback each OWN the line -- one is drained
                // at the prompt, the other outlives it -- and `stream()` hands
                // out `&[Runs]`, so sharing would change a type other crates
                // read. The review's other costs on this path (the line text
                // rebuilt per classifier, the `remove(0)` shift) were not
                // inherent and are gone.
                let chunk_line = super::chunks::ChunkLine { runs: line.clone() };
                // **Hiding is read HERE, not when the chunk closes**, because
                // it records the room and the room can change first. A `<nav>`
                // arrives on its own frame while the chunk stays open until
                // the prompt, so a creature that hides and is then walked away
                // from would be recorded in the room we walked TO -- found by
                // a test that expected the departure room and got the arrival
                // one.
                //
                // Every other classifier is happy at close_chunk, because none
                // of them reads state that a later frame in the same chunk can
                // move.
                //
                // **And a reveal clears it, here too.** Lich's
                // `push_revealed_targets` resets `@@hidden_targets = nil` first
                // (`overwatch.rb:84`), whatever else it does. The reveal was
                // handled only at `close_chunk`, and only to register the
                // creature, so a room stayed "has hiders" after the thing
                // hiding in it had come out (review). It is cleared at
                // ARRIVAL, not at the prompt, for the reason hiding is: in wire
                // order, a reveal then a fresh hide must leave a hider, and
                // the prompt would see both at once and could not tell.
                let rendered = chunk_line.text();
                match super::overwatch::classify_text(&chunk_line, &rendered) {
                    Some(super::overwatch::Sighting::Hid) => {
                        let room = self.room.id.clone();
                        self.overwatch.hid_in(room);
                    }
                    Some(super::overwatch::Sighting::Revealed { .. }) => self.overwatch.clear(),
                    None => {}
                }
                // **Group lines are read here too, for the same reason:
                // order.** `<indicator id='IconJOINED' visible='n'/>` empties
                // the group the moment its frame arrives (`group.rb:603-605`),
                // and a join line read later, at the prompt, would undo that
                // though the wire sent it FIRST. Found by the test for the
                // indicator, which failed exactly so.
                // With the character's own id, so a link that is you reads as
                // you (`Group::apply`).
                if let Some(event) = super::group::classify_text(&chunk_line, &rendered) {
                    let me = self.character.exist_id();
                    self.group.apply(&event, me.as_deref());
                }
                // A kill with no corpse, a portal, a boss's phase: here for
                // order, so a later `room objs` still outranks it (`prose.rs`).
                self.creatures.read_prose(&chunk_line, &rendered);
                self.chunk.push_line(chunk_line);
            }
            if text.stream == super::known_spells::STREAM {
                self.known_spells.read_line(&line);
            }
            self.list_line(&text.stream, &line);
            let buffer = self.streams.entry(text.stream.clone()).or_default();
            // **Bounded.** Found by review: every completed line was retained
            // forever, including ordinary main-window output, and nothing ever
            // dropped one -- `clearStream` empties a NAMED stream on the
            // game's say-so, and the prompt closes the analysis chunk without
            // touching this. A probe measured 2,000 lines retained from 2,000,
            // so a session left running accumulates text, styles and links
            // without limit.
            //
            // Oldest dropped first, which is the same rule and the same
            // reasoning as `chunks.rs`'s cap: the recent lines are the ones a
            // reader or a renderer wants, and a scrollback that forgets its
            // beginning is a scrollback rather than a leak.
            if buffer.push(line, MAX_STREAM_LINES) {
                self.tally.dropped = self.tally.dropped.saturating_add(1);
            }
            self.tally.seen = self.tally.seen.saturating_add(1);
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
