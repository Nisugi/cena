//! The hub every viewer attaches to: retained Story, the cached snapshot, and
//! the one place a native snapshot becomes a message on the wire.

use super::{MAX_HISTORY_BYTES, MAX_HISTORY_LINES, MAX_WIRE_BYTES};
use cena_session::{Snapshot, State};
use cena_ui::{Closed, LifecycleView, ServerMessage, SessionView, StoryLine, WIRE_VERSION};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::broadcast;

pub(crate) struct Hub {
    pub(crate) map_projection: Option<crate::MapProjection>,
    pub(crate) snapshot: Option<Arc<str>>,
    pub(crate) session: String,
    pub(crate) generation: String,
    pub(crate) updates: broadcast::Sender<Arc<str>>,
    pub(super) sequence: u64,
    pub(super) history: VecDeque<StoryLine>,
    pub(super) history_bytes: usize,
    /// How many retained lines come **before** the newest hole in the Story,
    /// while any do.
    ///
    /// # Why a position and not a flag
    ///
    /// This was a flag, OR-ed on every lag and also set by ordinary eviction,
    /// and never cleared -- so after the first 256 lines every viewer that
    /// attached was told "Some Story history is unavailable", announced
    /// through a `role=status` region, for the rest of the session. Two
    /// different things were wearing one name:
    ///
    /// - **Eviction is not loss.** A bounded history drops its oldest line;
    ///   what remains is still contiguous, and a viewer shown it has missed
    ///   nothing *between* its lines.
    /// - **A hole is.** Lag or a missed fence means lines never reached the
    ///   hub, so the history holds text on both sides of something absent.
    ///
    /// A hole is only worth reporting while text precedes it. Once eviction
    /// has carried every earlier line away, the hole is at the front, which is
    /// indistinguishable from bounded history -- so the count reaching zero is
    /// the moment the claim stops being true, and it is cleared then.
    lines_before_gap: Option<usize>,
    /// The view last published, so a refresh that changes nothing sends
    /// nothing. See [`Hub::publish`].
    published: Option<SessionView>,
    /// A publish was skipped as unpresentable, so connected viewers missed
    /// its lines: the next one must be a full snapshot.
    resync: bool,
}

impl Hub {
    pub(crate) fn new() -> Self {
        Self {
            map_projection: None,
            snapshot: None,
            session: String::new(),
            generation: String::new(),
            updates: broadcast::channel(16).0,
            sequence: 0,
            history: VecDeque::new(),
            history_bytes: 0,
            lines_before_gap: None,
            published: None,
            resync: false,
        }
    }

    /// The view last published, for the hub page's card; `None` before the
    /// first publish.
    pub(crate) fn view(&self) -> Option<&SessionView> {
        self.published.as_ref()
    }

    /// Whether the retained history has a hole inside it.
    pub(super) fn gap_retained(&self) -> bool {
        self.lines_before_gap.is_some_and(|before| before > 0)
    }

    fn retain(&mut self, line: StoryLine) {
        self.history_bytes += line_bytes(&line);
        self.history.push_back(line);
        while self.history.len() > MAX_HISTORY_LINES || self.history_bytes > MAX_HISTORY_BYTES {
            self.evict_oldest();
        }
    }

    fn evict_oldest(&mut self) {
        if let Some(old) = self.history.pop_front() {
            self.history_bytes -= line_bytes(&old);
            // Ordinary eviction, NOT a gap (see `lines_before_gap`). It only
            // moves an existing hole one line nearer the front.
            self.lines_before_gap = self
                .lines_before_gap
                .and_then(|before| before.checked_sub(1))
                .filter(|before| *before > 0);
        }
    }

    fn snapshot_message(&self, view: &SessionView, history_gap: bool) -> ServerMessage {
        ServerMessage::Snapshot {
            version: WIRE_VERSION,
            session: self.session.clone(),
            generation: self.generation.clone(),
            cursor: self.sequence.to_string(),
            view: view.clone(),
            story: self.history.iter().cloned().collect(),
            history_gap,
        }
    }

    /// Present `snapshot` plus the lines assembled since the last publish.
    ///
    /// # Nothing changed, nothing sent
    ///
    /// The pump refreshes every 100 ms while a roundtime runs, and this used
    /// to publish on every one: a fresh presentation cursor, a full snapshot
    /// re-encoded (up to ~100 KB of history), and a broadcast to every viewer
    /// -- whose browser then rebuilt its stream panes and snapped them to the
    /// bottom ten times a second, so nobody could scroll back mid-fight. The
    /// only thing that moved was `remaining_seconds`, which changes once a
    /// second and which the viewer already counts down itself. Worse, 16
    /// queued messages at 10 Hz is 1.6 s of backlog, under the 2 s write
    /// timeout, so a merely slow viewer was closed as "lagged".
    ///
    /// So a publish with no lines, no gap, and a view equal to the last one
    /// is dropped here: no cursor, no encode, no broadcast. The roundtime
    /// view then changes at most at each seconds boundary.
    ///
    /// # Too large is degraded, not fatal
    ///
    /// Exceeding [`MAX_WIRE_BYTES`] used to stop the whole server. History
    /// is bounded in encoded bytes, so only the *view* can get there (a giant
    /// room): the snapshot is then sent with its text fields unknown
    /// ([`degraded`]), then with history trimmed, and only if even that cannot
    /// fit is the publish skipped -- with the next forced to a full snapshot
    /// so no connected viewer silently misses the lines.
    ///
    /// # Errors
    /// Only when the presentation sequence would overflow `u64`.
    pub(crate) fn publish(
        &mut self,
        snapshot: &Snapshot,
        mut lines: Vec<StoryLine>,
        gap: bool,
    ) -> Result<(), ()> {
        let session = snapshot.session.0.to_string();
        let generation = snapshot.generation.0.to_string();
        let mut view = SessionView::project(
            &snapshot.state,
            lifecycle(snapshot),
            snapshot.state.game_time_now(),
        );
        if snapshot.lifecycle == State::Ready {
            view.map_location = self
                .map_projection
                .as_ref()
                .map(|project| project(snapshot));
        }
        let resync = gap || std::mem::take(&mut self.resync);
        if !resync
            && lines.is_empty()
            && session == self.session
            && generation == self.generation
            && self.published.as_ref() == Some(&view)
        {
            return Ok(());
        }
        self.sequence = self.sequence.checked_add(1).ok_or(())?;
        self.session = session;
        self.generation = generation;
        if gap {
            // The hole is between what is retained now and what follows.
            self.lines_before_gap = Some(self.history.len()).filter(|before| *before > 0);
        }
        // Stamp each line with what its stream does when its window is closed.
        // Here rather than in `LineAssembler` because this is where the model
        // is: the declarations come from `<streamWindow ifClosed=>` and the
        // viewer cannot be trusted to know them.
        for line in &mut lines {
            line.closed = declared(&snapshot.state, &line.stream);
        }
        for line in &lines {
            self.retain(line.clone());
        }

        let mut shown = view.clone();
        let mut trimmed = false;
        let full = loop {
            if let Ok(full) = encode(&self.snapshot_message(&shown, self.gap_retained())) {
                break Some(full);
            }
            if shown == view {
                shown = degraded(&view);
            } else if self.history.is_empty() {
                break None;
            } else {
                trimmed = true;
                for _ in 0..self.history.len().div_ceil(4) {
                    self.evict_oldest();
                }
            }
        };
        let Some(full) = full else {
            // Unpresentable even as unknowns with no history: keep the last
            // good snapshot, and make the next publish a full one.
            self.sequence -= 1;
            self.resync = true;
            return Ok(());
        };
        // **Whoever is connected at a hole is told**, even when nothing of the
        // retained history precedes it: they had text before it and have not
        // seen what fell in between. A viewer attaching later is told only
        // while the hole is inside what it receives -- the cached `full`.
        let outgoing = if resync || trimmed {
            let told = gap || trimmed || self.gap_retained();
            if told == self.gap_retained() {
                Arc::clone(&full)
            } else {
                encode(&self.snapshot_message(&shown, told)).unwrap_or_else(|()| Arc::clone(&full))
            }
        } else {
            encode(&ServerMessage::Update {
                version: WIRE_VERSION,
                session: self.session.clone(),
                generation: self.generation.clone(),
                cursor: self.sequence.to_string(),
                view: shown,
                lines,
            })
            // An update that cannot fit falls back to the snapshot, which does.
            .unwrap_or_else(|()| Arc::clone(&full))
        };
        self.snapshot = Some(full);
        self.published = Some(view);
        let _ = self.updates.send(outgoing);
        Ok(())
    }
}

/// `view` with every unbounded text field made unknown: what a viewer is
/// sent when the real view cannot fit in one message.
///
/// Unknown rather than cut short, because a room description stopped
/// mid-sentence reads as the whole description; "Description unknown" does
/// not. What stays is small by construction: vitals, roundtime, lifecycle.
fn degraded(view: &SessionView) -> SessionView {
    SessionView {
        map_location: None,
        room: cena_ui::RoomView {
            id: None,
            title: None,
            description: None,
            exits: None,
            creatures: None,
            objects: None,
            players: None,
        },
        left_hand: cena_ui::HandView::Unknown,
        right_hand: cena_ui::HandView::Unknown,
        prompt: None,
        unknown_tags: Vec::new(),
        ..view.clone()
    }
}

pub(crate) fn encode(message: &ServerMessage) -> Result<Arc<str>, ()> {
    // Stop serialization at the cap too: a giant native room must not first
    // allocate an unbounded JSON buffer only to be rejected afterwards.
    struct Bounded(Vec<u8>);
    impl std::io::Write for Bounded {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.0.len().saturating_add(bytes.len()) > MAX_WIRE_BYTES {
                return Err(std::io::Error::other("presentation message limit"));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut bounded = Bounded(Vec::new());
    serde_json::to_writer(&mut bounded, message).map_err(|_| ())?;
    String::from_utf8(bounded.0).map(Arc::from).map_err(|_| ())
}

/// What `stream` declared it does when its window is closed.
///
/// Translates the model's answer into the wire DTO. The main window is not a
/// stream with a fallback -- it *is* the fallback -- and an undeclared stream
/// falls through to main rather than being hidden, for the reason
/// `stream_windows::Windows::route` gives: 16 ids are declared against 8 ever
/// pushed to, and the pushed set is not closed -- so an unknown stream is
/// likelier to be one this build has not seen than one the server means to
/// suppress.
fn declared(state: &cena_session::GameState, stream: &str) -> Closed {
    use cena_session::stream_windows::{Closed as Model, MAIN};
    if stream.is_empty() || stream == MAIN {
        return Closed::Main;
    }
    match state.stream_windows().declared(stream) {
        Some(Model::Drop) => Closed::Drop,
        Some(Model::Styled(style)) => Closed::Styled {
            style: style.clone(),
        },
        Some(Model::Route(window)) => Closed::Route {
            window: window.clone(),
        },
        Some(Model::Main) | None => Closed::Main,
    }
}

pub(super) fn lifecycle(snapshot: &Snapshot) -> LifecycleView {
    match snapshot.lifecycle {
        State::Ready => LifecycleView::Ready,
        State::Closed => LifecycleView::Closed { detail: None },
        State::Reconnecting => LifecycleView::Reconnecting {
            attempt: snapshot.retry.as_ref().map(|retry| retry.attempt),
            retry_delay_ms: snapshot
                .retry
                .as_ref()
                .map(|retry| u64::try_from(retry.delay.as_millis()).unwrap_or(u64::MAX)),
            detail: snapshot.retry.as_ref().map(|retry| retry.detail.clone()),
        },
        State::Connecting | State::Authenticating | State::Syncing => LifecycleView::Connecting,
    }
}

/// An upper bound on the bytes `line` adds to an encoded message.
///
/// **Counted as JSON, not as text.** This used to sum raw string lengths,
/// and JSON escaping is not 1:1: a control character is written as `\u00XX`,
/// six bytes for one. A history held to 96 KB of raw text could therefore
/// encode to ~576 KB and cross [`MAX_WIRE_BYTES`] -- which stopped the
/// server. The 64 per run and per line covers field names and punctuation.
pub(super) fn line_bytes(line: &StoryLine) -> usize {
    let closed = match &line.closed {
        Closed::Styled { style } => json_len(style),
        Closed::Route { window } => json_len(window),
        Closed::Main | Closed::Drop => 0,
    };
    64 + json_len(&line.stream)
        + closed
        + line
            .runs
            .iter()
            .map(|run| json_len(&run.text) + run.preset.as_deref().map_or(0, json_len) + 64)
            .sum::<usize>()
}

/// Exactly what `serde_json` writes for `text`'s characters, without quotes:
/// two bytes for the short escapes, six for any other control character, and
/// UTF-8 as-is (it escapes nothing else).
fn json_len(text: &str) -> usize {
    text.chars()
        .map(|c| match c {
            '"' | '\\' | '\n' | '\r' | '\t' | '\u{8}' | '\u{c}' => 2,
            c if u32::from(c) < 0x20 => 6,
            c => c.len_utf8(),
        })
        .sum()
}
