//! Assemble UI text fragments, independently of parser frame boundaries.

use std::collections::VecDeque;

use crate::projection::bounded_text;
use crate::view::Closed;
use crate::{StoryLine, StyledRun};

pub const MAX_LINE_BYTES: usize = 16 * 1024;
pub const MAX_LINE_RUNS: usize = 256;
pub const MAX_PENDING_STREAMS: usize = 32;
const MAX_STREAM_BYTES: usize = 128;
const MAX_PRESET_BYTES: usize = 128;

/// Bounded unfinished lines, independent per stream and ordered by recent use.
///
/// A stream switch does not end its line: the enclosing stream can resume after
/// an interleaved thought. A prompt calls `flush`; generation changes and lag
/// recovery call `reset` so text from different histories is never joined.
#[derive(Debug, Default)]
pub struct LineAssembler {
    pending: VecDeque<(String, Pending)>,
}

#[derive(Debug, Default)]
struct Pending {
    runs: Vec<StyledRun>,
    bytes: usize,
    truncated: bool,
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
                    lines.push(old.finish(key));
                }
                Pending::default()
            };
        let key = bounded_text(stream, MAX_STREAM_BYTES).to_owned();
        for (index, piece) in run.text.split('\n').enumerate() {
            if index > 0 {
                lines.push(std::mem::take(&mut pending).finish(key.clone()));
            }
            pending.append(piece, run);
        }
        if stream.len() > MAX_STREAM_BYTES {
            pending.truncated = true;
            lines.push(pending.finish(key));
            // Even complete lines preceding the tail have a shortened stream.
            for line in &mut lines {
                line.truncated = true;
            }
        } else if ends_line {
            lines.push(pending.finish(key));
        } else {
            self.pending.push_back((key, pending));
        }
        lines
    }

    /// Close nonempty partial lines at a prompt or clean stream end.
    pub fn flush(&mut self) -> Vec<StoryLine> {
        self.pending
            .drain(..)
            .filter(|(_, pending)| !pending.runs.is_empty() || pending.truncated)
            .map(|(stream, pending)| pending.finish(stream))
            .collect()
    }

    /// Discard fragments after native invalidation or a lost event interval.
    pub fn reset(&mut self) {
        self.pending.clear();
    }

    /// Clear the named stream's unfinished line at its native clear boundary.
    pub fn clear_stream(&mut self, stream: &str) {
        self.pending.retain(|(key, _)| key != stream);
    }
}

impl Pending {
    fn append(&mut self, text: &str, style: &StyledRun) {
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
