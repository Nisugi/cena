//! One finished line, as the story lines a viewer draws.
//!
//! This used to assemble lines itself, frame by frame, and so it kept what
//! the model already keeps: a partial line per stream, flushed at a prompt,
//! reset on a new connection. The session now publishes each finished line,
//! the model's own (`plan/45` §4a), so what is left here is per line and
//! holds no state: bound it, split it at any embedded newline, and sort it
//! when `;sorter` is on.

use crate::projection::bounded_text;
use crate::sorter::{self, Piece};
use crate::view::Closed;
use crate::{StoryLine, StyledRun};

/// Text bytes kept per line; overflow marks the line `truncated`.
pub const MAX_LINE_BYTES: usize = 16 * 1024;
/// Runs kept per line; overflow marks the line `truncated`.
pub const MAX_LINE_RUNS: usize = 256;
const MAX_STREAM_BYTES: usize = 128;
const MAX_PRESET_BYTES: usize = 128;
/// Pieces kept for the sorter per line; past it the line is shown unsorted.
/// MEASURED: the longest container look in a month of the author's logs
/// names 108 items, which is 217 pieces (`crate::sorter`).
const MAX_SORTED_PIECES: usize = 4 * MAX_LINE_RUNS;

/// The story lines for one finished line on `stream`.
///
/// `pieces` is the line in wire order: each run's text and style, and the
/// `noun=` of the object it names, which only the sorter reads. `sorting` is
/// `;sorter`: a main-stream container look then finishes as one line per
/// category.
///
/// Usually one line. None when there are no pieces at all (an empty
/// component body). More when the text carries a newline (a component body
/// can), or when the sorter rewrote a look. An oversized stream name is
/// shortened and every line marked `truncated`, so shortened names can never
/// pass for each other.
pub fn story_lines<I>(stream: &str, pieces: I, sorting: bool) -> Vec<StoryLine>
where
    I: IntoIterator<Item = (StyledRun, Option<String>)>,
{
    let sorts = sorting && (stream.is_empty() || stream == "main");
    let fresh = || Building {
        pieces: sorts.then(Vec::new),
        ..Building::default()
    };
    let mut pieces = pieces.into_iter().peekable();
    if pieces.peek().is_none() {
        return Vec::new();
    }
    let key = bounded_text(stream, MAX_STREAM_BYTES).to_owned();
    let mut lines = Vec::new();
    let mut line = fresh();
    for (run, noun) in pieces {
        for (index, piece) in run.text.split('\n').enumerate() {
            if index > 0 {
                std::mem::replace(&mut line, fresh()).finish_into(key.clone(), &mut lines);
            }
            line.append(piece, &run, noun.as_deref());
        }
    }
    if stream.len() > MAX_STREAM_BYTES {
        line.truncated = true;
    }
    line.finish_into(key, &mut lines);
    if stream.len() > MAX_STREAM_BYTES {
        for line in &mut lines {
            line.truncated = true;
        }
    }
    lines
}

/// A line being built, bounded as it grows.
#[derive(Debug, Default)]
struct Building {
    runs: Vec<StyledRun>,
    bytes: usize,
    truncated: bool,
    /// The line piece by piece, with the object each names: what the sorter
    /// reads, since merging same-style runs loses where a link began. `None`
    /// unless the line may be sorted, so no other line pays for it.
    pieces: Option<Vec<Piece>>,
}

impl Building {
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
    /// The declaration comes from `<streamWindow ifClosed=>` and this has no
    /// model to ask. The pump that owns the `GameState` stamps it
    /// (`cena-web/src/presentation.rs`), so the wire's rule is read in one
    /// place.
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

    fn line(stream: &str, runs: Vec<StyledRun>) -> Vec<StoryLine> {
        story_lines(stream, runs.into_iter().map(|run| (run, None)), false)
    }

    fn plain(line: &StoryLine) -> String {
        line.runs.iter().map(|run| run.text.as_str()).collect()
    }

    #[test]
    fn a_line_split_at_markup_is_drawn_as_one_line() {
        let mut bold = run("leather doublet");
        bold.bold = true;
        let lines = line("", vec![run("  a "), bold, run(".")]);
        assert_eq!(lines.len(), 1);
        assert_eq!(plain(&lines[0]), "  a leather doublet.");
        assert_eq!(lines[0].runs.len(), 3);
        assert!(lines[0].runs[1].bold);
    }

    #[test]
    fn no_pieces_draw_no_line_and_empty_text_draws_an_empty_one() {
        assert!(line("room objs", Vec::new()).is_empty());
        assert_eq!(line("", vec![run("")]).len(), 1);
    }

    #[test]
    fn embedded_newlines_and_blank_lines_are_real_boundaries() {
        let lines = line("", vec![run("one\n\nthree")]);
        assert_eq!(
            lines.iter().map(plain).collect::<Vec<_>>(),
            ["one", "", "three"]
        );
    }

    #[test]
    fn long_lines_truncate_at_utf8_boundaries() {
        let lines = line(
            "",
            vec![run(&"🦀".repeat(MAX_LINE_BYTES)), run("discarded tail")],
        );
        assert!(lines[0].truncated);
        assert_eq!(plain(&lines[0]).len(), MAX_LINE_BYTES);
    }

    #[test]
    fn run_and_preset_limits_bound_a_line() {
        let runs = (0..=MAX_LINE_RUNS)
            .map(|index| {
                let mut fragment = run("x");
                fragment.bold = index % 2 == 0;
                fragment
            })
            .collect();
        let lines = line("", runs);
        assert_eq!(lines[0].runs.len(), MAX_LINE_RUNS);
        assert!(lines[0].truncated);
        let mut long_preset = run("styled");
        long_preset.preset = Some("x".repeat(MAX_PRESET_BYTES + 1));
        let lines = line("", vec![long_preset]);
        assert!(lines[0].truncated);
        assert_eq!(
            lines[0].runs[0].preset.as_ref().unwrap().len(),
            MAX_PRESET_BYTES
        );
    }

    #[test]
    fn oversized_stream_names_are_marked_and_kept_apart() {
        let prefix = "a".repeat(MAX_STREAM_BYTES);
        let a = line(&format!("{prefix}one"), vec![run("first")]);
        let b = line(&format!("{prefix}two"), vec![run("second")]);
        assert_eq!(plain(&a[0]), "first");
        assert_eq!(plain(&b[0]), "second");
        assert!(a[0].truncated && b[0].truncated);
    }
}
