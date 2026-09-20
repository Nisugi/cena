//! The prompt-bounded chunk: one command's output, buffered and handed on.
//!
//! **The prompt is the universal boundary**, not an `info`-specific one. Lich
//! relies on it in three unrelated subsystems, and that is why this is a shared
//! mechanism rather than part of any one consumer:
//!
//! | Subsystem | What the prompt closes |
//! |---|---|
//! | container fills | `xmlparser.rb:658-660`: *"A prompt terminates the command burst and is the reliable close signal for clearContainer/inv container fills, **which have no closing tag of their own**."* |
//! | combat | `combat/tracker.rb:532`: the downstream hook buffers every line and flushes `@buffer` when `server_string.include?('<prompt time=')` |
//! | the parser FSM | `infomon/xmlparser.rb:620`: `StatusPrompt` resets `Parser::State` to `Ready` -- the only thing that unwedges a stuck block |
//!
//! MEASURED independently in `plan/15` §2a.4b: across six captures, **1,712
//! blobs** carrying `Roundtime:` prose, all 1,712 also carrying the opening tag,
//! and **zero** prompts between a tag and its prose. The blob is real.
//!
//! # What this owns, and what it does not
//!
//! It owns the buffer and the boundary. It does **not** know what any line
//! means: consumers register interest and are handed each completed line, then
//! told when the chunk closed. That split is `plan/12` §3a -- the chunker
//! remembers position, classifiers stay stateless, and the consumer above holds
//! game situation.
//!
//! # Lines, not frames
//!
//! The input is a **reassembled line** with its bold spans, because a frame
//! boundary is not a line boundary: an enhancive stat line is five frames with
//! one `ends_line` (`character/stats.rs`'s module doc measures it). `route_text`
//! already does that reassembly for the stream buffers; this reuses its output
//! rather than re-deriving it.
//!
//! # Why not one buffer of raw strings, as Lich has
//!
//! Lich's tracker buffers the raw `server_string` and the combat parser then
//! re-scans it with `TARGET_LINK_PATTERN` and `BOLD_WRAPPER_PATTERN`
//! (`combat/parser.rb:23-31`) to recover `exist`, `noun` and bold -- facts the
//! XML parser had already extracted and thrown away. Four of that parser's
//! recorded bugs are re-scanning bugs: an attacker logged as `"his"`, a phantom
//! creature named `"grim gigas skald's"`, a doubled space in
//! `"halfling  cannibal"`, and ten unattributed bleed ticks.
//!
//! Cena's frames carry `LinkKind::Exist` and `bold_depth` already, so a chunk
//! here holds **parsed** lines and no consumer needs a second parser.
//!
//! > **CORRECTED 2026-09-20.** The paragraph above was true of the pipeline and
//! > false of the type. [`ChunkLine`] was built as `{ text, bold }` -- the plain
//! > text and the bold fragments -- and **discarded every `Link`** on the way in
//! > (`streams.rs`, the `push_line` call). The `info` reader never noticed,
//! > because a stat line has no links; the combat port would have, on its first
//! > line, because a swing's target is a link and nothing else. Found while
//! > inventorying the combat tracker (`inventory/11` §6a). A line now holds the
//! > [`Runs`] it was reassembled from, and text and bold are derived.

use cena_protocol::frame::Style;
use cena_protocol::runs::{Run, Runs};

/// One completed line of a chunk, as the parser reassembled it.
///
/// Holds the runs rather than a rendering of them, so every fact the parser
/// extracted stays reachable: the text, which spans arrived bolded, and **which
/// spans were links** -- `exist` and `noun` for a creature, the id a combat
/// consumer needs to say *who* was hit. Bold matters to the `info` and `skill`
/// readers (`plan/15` §2c); links matter to combat.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChunkLine {
    /// The runs, in wire order, with their styles and links.
    pub runs: Runs,
}

impl ChunkLine {
    /// A line of plain text with no markup, for tests and synthetic input.
    #[must_use]
    pub fn plain(text: &str) -> Self {
        Self {
            runs: Runs {
                runs: vec![Run {
                    text: text.to_owned(),
                    style: Style::default(),
                    link: None,
                }],
            },
        }
    }

    /// The reassembled text, markup removed.
    #[must_use]
    pub fn text(&self) -> String {
        self.runs.plain()
    }

    /// The fragments that arrived with `bold_depth > 0`, in wire order.
    #[must_use]
    pub fn bold(&self) -> Vec<String> {
        self.runs.bold_fragments()
    }

    /// The bold fragments as string slices, for a classifier that takes `&[&str]`.
    #[must_use]
    pub fn bold_refs(&self) -> Vec<&str> {
        self.runs
            .runs
            .iter()
            .filter(|r| r.style.bold_depth > 0)
            .map(|r| r.text.as_str())
            .collect()
    }

    /// Every link on the line, in order.
    ///
    /// What a combat consumer reads to learn a target's `exist` and `noun`
    /// without a second parser -- `plan/12` §3a's bargain, kept by the type
    /// rather than only by the pipeline.
    pub fn links(&self) -> impl Iterator<Item = &cena_protocol::frame::Link> {
        self.runs.links()
    }
}

/// Lines accumulated since the last prompt.
///
/// **Bounded.** Lich caps its buffer at 200 lines and drops the oldest on
/// overflow (`combat/tracker.rb` `DEFAULT_SETTINGS[:buffer_size]`, and the
/// `@buffer.shift` at `:556`). A chunk that never terminates -- a disconnect
/// mid-report -- must not grow without limit, and dropping the oldest keeps the
/// most recent lines, which are the ones a terminator would have applied.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Chunk {
    lines: Vec<ChunkLine>,
    /// Lines discarded to the cap since this chunk opened.
    ///
    /// Reported rather than silent: a truncated chunk is a fact a consumer may
    /// want to refuse to act on, and Rule 2.2's floor is that nothing is
    /// dropped without saying so.
    dropped: usize,
}

/// How many lines one chunk may hold before the oldest are dropped.
///
/// Lich's value, ported (`combat/tracker.rb`, `buffer_size: 200`). Chosen there
/// as a setting; fixed here until something needs it configurable, per Rule -1
/// (no config option with one value).
pub const MAX_CHUNK_LINES: usize = 200;

impl Chunk {
    /// Every line in the chunk, oldest first.
    #[must_use]
    pub fn lines(&self) -> &[ChunkLine] {
        &self.lines
    }

    /// How many lines were dropped to the cap.
    #[must_use]
    pub const fn dropped(&self) -> usize {
        self.dropped
    }

    /// Did this chunk overflow, so that its oldest lines are gone?
    #[must_use]
    pub const fn is_truncated(&self) -> bool {
        self.dropped > 0
    }

    /// Nothing accumulated.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Add a completed line, dropping the oldest if the cap is reached.
    ///
    /// Public so a test can build a chunk directly: the alternative is only
    /// ever constructing one through the parser, which makes a chunk-level
    /// property (the cap, the drop count) expensive to state.
    pub fn push_line(&mut self, line: ChunkLine) {
        if self.lines.len() >= MAX_CHUNK_LINES {
            self.lines.remove(0);
            self.dropped = self.dropped.saturating_add(1);
        }
        self.lines.push(line);
    }

    /// Take the chunk, leaving an empty one behind.
    ///
    /// **`dropped` resets with it.** The count describes one chunk, and carrying
    /// it into the next would make an old overflow look like a new one.
    pub(crate) fn take(&mut self) -> Self {
        std::mem::take(self)
    }
}

impl super::GameState {
    /// How many lines the currently-open chunk holds.
    ///
    /// The chunk is mid-flight state, not a fact about the character, so it is
    /// not part of the model's public shape -- but its size is observable, which
    /// is what lets a test assert that a reconnect dropped it.
    #[must_use]
    pub fn open_chunk_len(&self) -> usize {
        self.chunk.lines().len()
    }

    /// The currently-open chunk, read-only.
    ///
    /// Exposed so a test can assert what a line *carries* -- its links, its
    /// bold spans -- before the prompt hands it to consumers and it is gone.
    /// The guard for the 2026-09-20 correction above lives on this.
    #[must_use]
    pub const fn open_chunk(&self) -> &Chunk {
        &self.chunk
    }

    /// Hand the completed chunk to every consumer, then clear it.
    ///
    /// Called from exactly one place -- the `Frame::Prompt` arm -- because the
    /// prompt is the boundary. Consumers are called in a fixed order so a
    /// replay is deterministic (criterion 7).
    ///
    /// **A consumer is offered the whole chunk, not each line as it arrives.**
    /// That is what lets one look at a line's neighbours: a `skill` table's
    /// rows mean different things depending on the header above them, and a
    /// combat exchange is several lines and one event (`plan/12` §3a's
    /// "multi-line events"). Line-at-a-time delivery would push that memory
    /// back into the consumers, which is what this refactor removes.
    pub(super) fn close_chunk(&mut self) {
        let chunk = self.chunk.take();
        if chunk.is_empty() {
            return;
        }
        // `info`, and later `skill`. Others register here as they are built --
        // a combat tracker reads the same chunk with no new buffering.
        self.character.consume_chunk(&chunk);
    }
}
