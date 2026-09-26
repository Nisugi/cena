//! Assemble UI text fragments, independently of parser frame boundaries.

use std::collections::VecDeque;

use crate::projection::bounded_text;
use crate::sorter::{self, Piece};
use crate::view::Closed;
use crate::{StoryLine, StyledRun};

/// Text bytes kept per unfinished line; overflow marks the line `truncated`.
pub const MAX_LINE_BYTES: usize = 16 * 1024;
/// Runs kept per unfinished line; overflow marks the line `truncated`.
pub const MAX_LINE_RUNS: usize = 256;
/// Streams that may hold an unfinished line at once; a new stream beyond this
/// emits the least recently used partial, marked `truncated`.
pub const MAX_PENDING_STREAMS: usize = 32;
const MAX_STREAM_BYTES: usize = 128;
const MAX_PRESET_BYTES: usize = 128;
/// Pieces kept for the sorter per unfinished line; past it the line is shown
/// unsorted. MEASURED: the longest container look in a month of the author's
/// logs names 108 items, which is 217 pieces (`crate::sorter`).
const MAX_SORTED_PIECES: usize = 4 * MAX_LINE_RUNS;

/// Bounded unfinished lines, independent per stream and ordered by recent use.
///
/// A stream switch does not end its line: the enclosing stream can resume after
/// an interleaved thought. A prompt calls `flush`; generation changes and lag
/// recovery call `reset` so text from different histories is never joined.
///
/// With [`Self::sort_containers`] on, a main-stream container look finishes
/// as one line per category (`;sorter`).
#[derive(Debug, Default)]
pub struct LineAssembler {
    pending: VecDeque<(String, Pending)>,
    /// `;sorter`, for lines begun from now on. Off until the session asks.
    sorting: bool,
}

#[derive(Debug, Default)]
struct Pending {
    runs: Vec<StyledRun>,
    bytes: usize,
    truncated: bool,
    /// The line piece by piece, as pushed, with the object each names: what
    /// the sorter reads, since merging same-style runs loses where a link
    /// began. `None` unless sorting was on when a main-stream line began, so
    /// no other line pays for it.
    pieces: Option<Vec<Piece>>,
}

impl LineAssembler {
    /// Accept one UI run. Embedded newlines and `ends_line` are boundaries;
    /// ordinary calls and stream switches are not.
    ///
    /// Bounds limit retained partial text, run metadata, and stream keys. When
    /// all stream slots are occupied, the oldest partial is emitted with its
    /// truncation flag. Oversized stream names are emitted immediately, marked
    /// truncated, so shortened keys can never merge unrelated streams.
    pub fn push(&mut self, stream: &str, run: &StyledRun, ends_line: bool) -> Vec<StoryLine> {
        self.push_naming(stream, run, None, ends_line)
    }

    /// [`Self::push`] for a run that names a game object: the `noun=` of the
    /// `<a exist= noun=>` link it sits in (`TextFrame::object`). Only the
    /// sorter reads it; a line's runs are the same either way.
    pub fn push_naming(
        &mut self,
        stream: &str,
        run: &StyledRun,
        noun: Option<&str>,
        ends_line: bool,
    ) -> Vec<StoryLine> {
        let mut lines = Vec::new();
        let mut pending =
            if let Some(index) = self.pending.iter().position(|(key, _)| key == stream) {
                self.pending
                    .remove(index)
                    .map(|(_, pending)| pending)
                    .unwrap_or_default()
            } else {
                if self.pending.len() == MAX_PENDING_STREAMS
                    && let Some((key, mut old)) = self.pending.pop_front()
                {
                    old.truncated = true;
                    old.finish_into(key, &mut lines);
                }
                self.fresh(stream)
            };
        let key = bounded_text(stream, MAX_STREAM_BYTES).to_owned();
        for (index, piece) in run.text.split('\n').enumerate() {
            if index > 0 {
                let next = self.fresh(stream);
                std::mem::replace(&mut pending, next).finish_into(key.clone(), &mut lines);
            }
            pending.append(piece, run, noun);
        }
        if stream.len() > MAX_STREAM_BYTES {
            pending.truncated = true;
            pending.finish_into(key, &mut lines);
            // Even complete lines preceding the tail have a shortened stream.
            for line in &mut lines {
                line.truncated = true;
            }
        } else if ends_line {
            pending.finish_into(key, &mut lines);
        } else {
            self.pending.push_back((key, pending));
        }
        lines
    }

    /// Turn `;sorter` on or off: a main-stream container look begun from now
    /// on finishes as one line per category, or as it came.
    pub fn sort_containers(&mut self, on: bool) {
        self.sorting = on;
    }

    /// Close nonempty partial lines at a prompt or clean stream end.
    pub fn flush(&mut self) -> Vec<StoryLine> {
        let mut lines = Vec::new();
        for (stream, pending) in self.pending.drain(..) {
            if !pending.runs.is_empty() || pending.truncated {
                pending.finish_into(stream, &mut lines);
            }
        }
        lines
    }

    /// Discard fragments after native invalidation or a lost event interval.
    pub fn reset(&mut self) {
        self.pending.clear();
    }

    /// Clear the named stream's unfinished line at its native clear boundary.
    pub fn clear_stream(&mut self, stream: &str) {
        self.pending.retain(|(key, _)| key != stream);
    }

    /// A new line on `stream`, keeping pieces for the sorter only when it
    /// will read them: sorting is on, and this is the main stream, where a
    /// look arrives (`VellumFE` sorts `main` only, `flush_line.rs:426`).
    fn fresh(&self, stream: &str) -> Pending {
        Pending {
            pieces: (self.sorting && (stream.is_empty() || stream == "main")).then(Vec::new),
            ..Pending::default()
        }
    }
}

impl Pending {
    fn append(&mut self, text: &str, style: &StyledRun, noun: Option<&str>) {
        if text.is_empty() || self.truncated {
            return;
        }
        let piece = bounded_text(text, MAX_LINE_BYTES.saturating_sub(self.bytes));
        let preset = style
            .preset
            .as_deref()
            .map(|value| bounded_text(value, MAX_PRESET_BYTES).to_owned());
        self.truncated = piece.len() != text.len()
            || style
                .preset
                .as_ref()
                .is_some_and(|value| value.len() > MAX_PRESET_BYTES);
        if piece.is_empty() {
            return;
        }
        if let Some(last) = self.runs.last_mut()
            && last.bold == style.bold
            && last.monospace == style.monospace
            && last.preset == preset
        {
            last.text.push_str(piece);
        } else if self.runs.len() < MAX_LINE_RUNS {
            self.runs.push(StyledRun {
                text: piece.to_owned(),
                bold: style.bold,
                monospace: style.monospace,
                preset,
            });
        } else {
            self.truncated = true;
            return;
        }
        self.bytes += piece.len();
        if let Some(pieces) = &mut self.pieces {
            if pieces.len() < MAX_SORTED_PIECES && !self.truncated {
                pieces.push(Piece {
                    run: StyledRun {
                        text: piece.to_owned(),
                        bold: style.bold,
                        monospace: style.monospace,
                        preset: style.preset.clone(),
                    },
                    noun: noun.map(str::to_owned),
                });
            } else {
                self.pieces = None;
            }
        }
    }

    /// Finish into `lines`: sorted, when this is a whole container look the
    /// sorter takes (`crate::sorter`), and otherwise as it came.
    fn finish_into(self, stream: String, lines: &mut Vec<StoryLine>) {
        if !self.truncated
            && let Some(sorted) = self.pieces.as_deref().and_then(sorter::sort)
        {
            for runs in sorted {
                let mut line = Self::default();
                for run in &runs {
                    line.append(&run.text, run, None);
                }
                lines.push(line.finish(stream.clone()));
            }
            return;
        }
        lines.push(self.finish(stream));
    }

    /// **`closed` is left as [`Closed::Main`] here, deliberately.**
    ///
    /// The declaration comes from `<streamWindow ifClosed=>` and this
    /// assembler has no model to ask. The pump that owns the `GameState`
    /// stamps it (`cena-web/src/presentation.rs`), so the wire's rule is read
    /// in one place.
    ///
    /// `Main` rather than an `Option`: a line nobody classified is a line that
    /// shows in the story, which is the safe direction -- the unsafe one is
    /// hiding text because a declaration was missing.
    fn finish(self, stream: String) -> StoryLine {
        StoryLine {
            stream,
            runs: self.runs,
            truncated: self.truncated,
            closed: Closed::Main,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str) -> StyledRun {
        StyledRun {
            text: text.to_owned(),
            ..StyledRun::default()
        }
    }

    fn plain(line: &StoryLine) -> String {
        line.runs.iter().map(|run| run.text.as_str()).collect()
    }

    #[test]
    fn markup_fragment_boundaries_do_not_create_display_lines() {
        let mut assembly = LineAssembler::default();
        assert!(assembly.push("", &run("  a "), false).is_empty());
        let mut bold = run("leather doublet");
        bold.bold = true;
        assert!(assembly.push("", &bold, false).is_empty());
        let lines = assembly.push("", &run("."), true);
        assert_eq!(lines.len(), 1);
        assert_eq!(plain(&lines[0]), "  a leather doublet.");
        assert_eq!(lines[0].runs.len(), 3);
        assert!(lines[0].runs[1].bold);
    }

    #[test]
    fn interleaved_streams_resume_their_own_partial_lines() {
        let mut assembly = LineAssembler::default();
        assembly.push("", &run("Story "), false);
        let thoughts = assembly.push("thoughts", &run("a thought"), true);
        assert_eq!(thoughts[0].stream, "thoughts");
        let story = assembly.push("", &run("resumes"), true);
        assert_eq!(plain(&story[0]), "Story resumes");
    }

    #[test]
    fn embedded_newlines_and_blank_lines_are_real_boundaries() {
        let mut assembly = LineAssembler::default();
        let lines = assembly.push("", &run("one\n\nthree"), true);
        assert_eq!(
            lines.iter().map(plain).collect::<Vec<_>>(),
            ["one", "", "three"]
        );
        assert!(assembly.flush().is_empty());
    }

    #[test]
    fn prompt_flushes_and_generation_reset_discards_fragments() {
        let mut assembly = LineAssembler::default();
        assembly.push("", &run("before prompt"), false);
        assert_eq!(plain(&assembly.flush()[0]), "before prompt");
        assembly.push("", &run("stale"), false);
        assembly.reset();
        assert_eq!(
            plain(&assembly.push("", &run("new generation"), true)[0]),
            "new generation"
        );
        assembly.push("inv", &run("old list"), false);
        assembly.clear_stream("inv");
        assert!(assembly.flush().is_empty());
    }

    #[test]
    fn long_lines_truncate_at_utf8_boundaries_until_the_real_terminator() {
        let mut assembly = LineAssembler::default();
        assembly.push("", &run(&"🦀".repeat(MAX_LINE_BYTES)), false);
        assert!(assembly.pending[0].1.bytes <= MAX_LINE_BYTES);
        let lines = assembly.push("", &run("discarded tail"), true);
        assert!(lines[0].truncated);
        assert_eq!(plain(&lines[0]).len(), MAX_LINE_BYTES);
        assert_eq!(plain(&assembly.push("", &run("next"), true)[0]), "next");
    }

    #[test]
    fn stream_and_metadata_limits_prevent_unbounded_partial_state() {
        let mut assembly = LineAssembler::default();
        for index in 0..MAX_PENDING_STREAMS {
            assert!(
                assembly
                    .push(&index.to_string(), &run("partial"), false)
                    .is_empty()
            );
        }
        let evicted = assembly.push("extra", &run("new partial"), false);
        assert_eq!(evicted.len(), 1);
        assert!(evicted[0].truncated);
        assert_eq!(evicted[0].stream, "0");
        assert_eq!(assembly.pending.len(), MAX_PENDING_STREAMS);
        assembly.reset();
        for index in 0..=MAX_LINE_RUNS {
            let mut fragment = run("x");
            fragment.bold = index % 2 == 0;
            assembly.push("", &fragment, false);
        }
        let lines = assembly.flush();
        assert_eq!(lines[0].runs.len(), MAX_LINE_RUNS);
        assert!(lines[0].truncated);
        let mut long_preset = run("styled");
        long_preset.preset = Some("x".repeat(MAX_PRESET_BYTES + 1));
        let lines = assembly.push("", &long_preset, true);
        assert!(lines[0].truncated);
        assert_eq!(
            lines[0].runs[0].preset.as_ref().unwrap().len(),
            MAX_PRESET_BYTES
        );
    }

    #[test]
    fn oversized_stream_names_do_not_alias_each_others_fragments() {
        let mut assembly = LineAssembler::default();
        let prefix = "a".repeat(MAX_STREAM_BYTES);
        let a = assembly.push(&format!("{prefix}one"), &run("first"), false);
        let b = assembly.push(&format!("{prefix}two"), &run("second"), false);
        assert_eq!(plain(&a[0]), "first");
        assert_eq!(plain(&b[0]), "second");
        assert!(a[0].truncated && b[0].truncated);
        assert!(assembly.flush().is_empty());
    }
}
