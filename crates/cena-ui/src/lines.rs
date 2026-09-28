//! One finished line, as the story lines a viewer draws.
//!
//! This used to assemble lines itself, frame by frame, and so it kept what
//! the model already keeps: a partial line per stream, flushed at a prompt,
//! reset on a new connection. The session now publishes each finished line,
//! the model's own (`plan/45` §4a), already sorted when `;sorter` is on and
//! already answered by the character's triggers, so what is left here is per
//! line and holds no state: split its runs where a trigger's paint starts and
//! stops ([`painted`]), bound it, and split it at any embedded newline.

use cena_model::line::Line;

use crate::projection::bounded_text;
use crate::view::Closed;
use crate::{StoryLine, StyledRun};

/// A published line's runs as the story draws them: each split where a
/// trigger's paint starts or stops, and each piece given that paint's colour,
/// background and bold. The paint was resolved in the session, once, for
/// every viewer (`plan/45` §4); this only lays it over the runs.
#[must_use]
pub fn painted(line: &Line) -> Vec<StyledRun> {
    let mut out = Vec::new();
    let mut at = 0;
    for run in &line.runs.runs {
        let span = at..at + run.text.len();
        at = span.end;
        let mut edges = vec![span.start, span.end];
        for paint in &line.paint {
            for edge in [paint.span.start, paint.span.end] {
                if span.start < edge && edge < span.end {
                    edges.push(edge);
                }
            }
        }
        edges.sort_unstable();
        edges.dedup();
        for pair in edges.windows(2) {
            let &[start, end] = pair else {
                continue;
            };
            let Some(text) = run.text.get(start - span.start..end - span.start) else {
                continue;
            };
            let paint = line
                .paint
                .iter()
                .find(|paint| paint.span.start <= start && end <= paint.span.end);
            out.push(StyledRun {
                text: text.to_owned(),
                bold: run.style.bold_depth > 0 || paint.is_some_and(|paint| paint.bold),
                monospace: run.style.mono,
                preset: run.style.preset.clone(),
                color: paint.and_then(|paint| paint.color).map(|c| c.to_string()),
                background: paint
                    .and_then(|paint| paint.background)
                    .map(|c| c.to_string()),
                link: run.link.as_ref().and_then(crate::RunLink::of),
            });
        }
    }
    out
}

/// Text bytes kept per line; overflow marks the line `truncated`.
pub const MAX_LINE_BYTES: usize = 16 * 1024;
/// Runs kept per line; overflow marks the line `truncated`.
pub const MAX_LINE_RUNS: usize = 256;
const MAX_STREAM_BYTES: usize = 128;
const MAX_PRESET_BYTES: usize = 128;

/// The story lines for one finished line on `stream`.
///
/// `runs` is the line in wire order, each run's text and style.
///
/// Usually one line. None when there are no runs at all (an empty component
/// body). More when the text carries a newline (a component body can). An
/// oversized stream name is shortened and every line marked `truncated`, so
/// shortened names can never pass for each other.
pub fn story_lines<I>(stream: &str, runs: I) -> Vec<StoryLine>
where
    I: IntoIterator<Item = StyledRun>,
{
    let mut runs = runs.into_iter().peekable();
    if runs.peek().is_none() {
        return Vec::new();
    }
    let key = bounded_text(stream, MAX_STREAM_BYTES).to_owned();
    let mut lines = Vec::new();
    let mut line = Building::default();
    for run in runs {
        for (index, piece) in run.text.split('\n').enumerate() {
            if index > 0 {
                lines.push(std::mem::take(&mut line).finish(key.clone()));
            }
            line.append(piece, &run);
        }
    }
    if stream.len() > MAX_STREAM_BYTES {
        line.truncated = true;
    }
    lines.push(line.finish(key));
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
}

impl Building {
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
            && last.color == style.color
            && last.background == style.background
            && last.link == style.link
        {
            last.text.push_str(piece);
        } else if self.runs.len() < MAX_LINE_RUNS {
            self.runs.push(StyledRun {
                text: piece.to_owned(),
                bold: style.bold,
                monospace: style.monospace,
                preset,
                color: style.color.clone(),
                background: style.background.clone(),
                link: style.link.clone(),
            });
        } else {
            self.truncated = true;
            return;
        }
        self.bytes += piece.len();
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

    fn plain(line: &StoryLine) -> String {
        line.runs.iter().map(|run| run.text.as_str()).collect()
    }

    const RED: cena_model::trigger::Color = cena_model::trigger::Color {
        red: 0xff,
        green: 0x40,
        blue: 0x40,
    };

    /// A published line of these runs, with this paint.
    fn published(texts: &[&str], paint: Vec<cena_model::trigger::Paint>) -> Line {
        let mut runs = cena_model::ChunkLine::plain("template").runs;
        let template = runs.runs.first().cloned();
        runs.runs = texts
            .iter()
            .filter_map(|text| {
                let mut run = template.clone()?;
                run.text = (*text).to_owned();
                Some(run)
            })
            .collect();
        Line {
            stream: String::new(),
            runs,
            paint,
        }
    }

    fn red(span: std::ops::Range<usize>, bold: bool) -> cena_model::trigger::Paint {
        cena_model::trigger::Paint {
            span,
            color: Some(RED),
            background: None,
            bold,
        }
    }

    fn link(kind: cena_model::LinkKind, text: &str) -> cena_model::Link {
        cena_model::Link {
            kind,
            text: text.to_owned(),
            coord: None,
        }
    }

    /// A link reaches the runs the story draws as what a click on it does,
    /// and is never merged into the plain text beside it.
    #[test]
    fn a_link_is_kept_and_kept_apart() {
        let mut line = published(&["You see ", "a kobold", "."], Vec::new());
        let kobold = cena_model::LinkKind::Exist {
            id: "123".to_owned(),
            noun: "kobold".to_owned(),
        };
        if let Some(run) = line.runs.runs.get_mut(1) {
            run.link = Some(link(kobold, "a kobold"));
        }
        let lines = story_lines("", painted(&line));
        let runs = &lines[0].runs;
        let texts: Vec<&str> = runs.iter().map(|run| run.text.as_str()).collect();
        assert_eq!(texts, ["You see ", "a kobold", "."], "kept apart");
        assert_eq!(
            runs[1].link,
            Some(crate::RunLink::Object {
                exist: "123".to_owned(),
                noun: "kobold".to_owned(),
                coord: None,
            })
        );
        assert_eq!((runs[0].link.as_ref(), runs[2].link.as_ref()), (None, None));
    }

    /// Each kind of link as a click acts on it: a command written in the
    /// link or its own text, a web address; one that does nothing is none.
    #[test]
    fn each_link_does_what_a_click_on_it_should() {
        use crate::RunLink;
        use cena_model::LinkKind;
        let command = |command: &str| {
            Some(RunLink::Command {
                command: command.to_owned(),
            })
        };
        let direct = LinkKind::Direct {
            cmd: "go north".to_owned(),
        };
        assert_eq!(RunLink::of(&link(direct, "north")), command("go north"));
        assert_eq!(
            RunLink::of(&link(LinkKind::DirectText, "look")),
            command("look")
        );
        let url = LinkKind::Url {
            href: "https://play.net".to_owned(),
        };
        assert_eq!(
            RunLink::of(&link(url, "play.net")),
            Some(RunLink::Url {
                href: "https://play.net".to_owned()
            })
        );
        assert_eq!(RunLink::of(&link(LinkKind::NotActionable, "Maravel")), None);
    }

    #[test]
    fn paint_splits_a_run_where_it_starts_and_stops() {
        let runs = painted(&published(&["You are stunned!"], vec![red(8..15, true)]));
        let texts: Vec<&str> = runs.iter().map(|run| run.text.as_str()).collect();
        assert_eq!(texts, ["You are ", "stunned", "!"]);
        assert_eq!(runs[1].color.as_deref(), Some("#ff4040"));
        assert!(runs[1].bold, "a look's bold");
        assert_eq!((runs[0].color.as_deref(), runs[0].bold), (None, false));
        assert_eq!(runs[2].color, None);
    }

    #[test]
    fn paint_over_two_runs_paints_each_piece_and_story_lines_keeps_it() {
        let runs = painted(&published(
            &["You are ", "stunned!"],
            vec![red(4..15, false)],
        ));
        let pieces: Vec<(&str, bool)> = runs
            .iter()
            .map(|run| (run.text.as_str(), run.color.is_some()))
            .collect();
        assert_eq!(
            pieces,
            [
                ("You ", false),
                ("are ", true),
                ("stunned", true),
                ("!", false)
            ]
        );
        // Drawn: the two painted pieces join, and paint keeps them apart from
        // the unpainted ones that are otherwise styled alike.
        let lines = story_lines("", runs);
        let drawn: Vec<&str> = lines[0].runs.iter().map(|run| run.text.as_str()).collect();
        assert_eq!(drawn, ["You ", "are stunned", "!"]);
    }

    #[test]
    fn an_unpainted_run_is_written_without_paint() {
        let plain = serde_json::to_value(run("x")).unwrap();
        assert!(
            plain.get("color").is_none() && plain.get("background").is_none(),
            "{plain}"
        );
        let mut coloured = run("x");
        coloured.color = Some("#ff4040".into());
        let written = serde_json::to_value(&coloured).unwrap();
        assert_eq!(written["color"], "#ff4040");
        assert_eq!(
            serde_json::from_value::<StyledRun>(written).unwrap(),
            coloured
        );
    }

    #[test]
    fn a_line_split_at_markup_is_drawn_as_one_line() {
        let mut bold = run("leather doublet");
        bold.bold = true;
        let lines = story_lines("", vec![run("  a "), bold, run(".")]);
        assert_eq!(lines.len(), 1);
        assert_eq!(plain(&lines[0]), "  a leather doublet.");
        assert_eq!(lines[0].runs.len(), 3);
        assert!(lines[0].runs[1].bold);
    }

    #[test]
    fn no_runs_draw_no_line_and_empty_text_draws_an_empty_one() {
        assert!(story_lines("room objs", Vec::new()).is_empty());
        assert_eq!(story_lines("", vec![run("")]).len(), 1);
    }

    #[test]
    fn embedded_newlines_and_blank_lines_are_real_boundaries() {
        let lines = story_lines("", vec![run("one\n\nthree")]);
        assert_eq!(
            lines.iter().map(plain).collect::<Vec<_>>(),
            ["one", "", "three"]
        );
    }

    #[test]
    fn long_lines_truncate_at_utf8_boundaries() {
        let lines = story_lines(
            "",
            vec![run(&"🦀".repeat(MAX_LINE_BYTES)), run("discarded tail")],
        );
        assert!(lines[0].truncated);
        assert_eq!(plain(&lines[0]).len(), MAX_LINE_BYTES);
    }

    #[test]
    fn run_and_preset_limits_bound_a_line() {
        let runs: Vec<StyledRun> = (0..=MAX_LINE_RUNS)
            .map(|index| {
                let mut fragment = run("x");
                fragment.bold = index % 2 == 0;
                fragment
            })
            .collect();
        let lines = story_lines("", runs);
        assert_eq!(lines[0].runs.len(), MAX_LINE_RUNS);
        assert!(lines[0].truncated);
        let mut long_preset = run("styled");
        long_preset.preset = Some("x".repeat(MAX_PRESET_BYTES + 1));
        let lines = story_lines("", vec![long_preset]);
        assert!(lines[0].truncated);
        assert_eq!(
            lines[0].runs[0].preset.as_ref().unwrap().len(),
            MAX_PRESET_BYTES
        );
    }

    #[test]
    fn oversized_stream_names_are_marked_and_kept_apart() {
        let prefix = "a".repeat(MAX_STREAM_BYTES);
        let a = story_lines(&format!("{prefix}one"), vec![run("first")]);
        let b = story_lines(&format!("{prefix}two"), vec![run("second")]);
        assert_eq!(plain(&a[0]), "first");
        assert_eq!(plain(&b[0]), "second");
        assert!(a[0].truncated && b[0].truncated);
    }
}
