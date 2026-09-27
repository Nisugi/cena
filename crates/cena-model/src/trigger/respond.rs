//! What a character's triggers do to a finished line: the lines a viewer is
//! given in its place (`plan/45` §3b, §4).
//!
//! | Response | Does | When several apply |
//! |---|---|---|
//! | squelch | nothing is shown | any one hides the line, whatever else matched |
//! | substitute | its text replaces the match; a regex's `$1` or `${1}` is a group | in rank order; one that overlaps a substitute already made is skipped |
//! | look | paints the match, a group, or the line | colour, background and bold each come from the best look over each character (below) |
//! | redirect | the line is shown on another stream, instead or as well | the best-ranked one decides |
//!
//! **The best look** is the highest priority, then a look on the match or a
//! group over one on the whole line, then file order. A whole-line look is
//! the backdrop and a word's look sits on it, unless priority says
//! otherwise. Each field is decided on its own, so a line's background shows
//! behind a word's colour. Bold is on if any look over a character sets it:
//! a look cannot unset it.
//!
//! **Matched once, on the game's text.** A look over what a substitute
//! replaced covers the replacement. A substitute's text is not matched again,
//! so no trigger feeds on another's output.
//!
//! A substitute keeps the style and link of the run it starts in: a
//! substituted name is still the link it was.
//!
//! **Every trigger that matched fired**, whatever the looks decided and even
//! on a squelched line ([`Answer::fired`]), so a trigger's flag is set on a
//! line nobody is shown: `plan/45` §4's *"hide this but tell me"*.

use std::cmp::Reverse;
use std::ops::Range;

use cena_protocol::runs::{Run, Runs};

use super::{Attention, Color, Hit, Look, Matcher, Rule, Span};
use crate::GameState;
use crate::line::Line;

/// What a character's triggers make of one finished line.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Answer {
    /// The lines a viewer is given in its place: none when it is squelched,
    /// two when it is redirected as a copy, otherwise one.
    pub lines: Vec<Line>,
    /// The triggers that fired, by rank in [`Matcher::triggers`], each once,
    /// in rank order.
    pub fired: Vec<usize>,
    /// What those that call for attention call for, at each one's first hit.
    pub attention: Vec<Attention>,
}

/// A look, resolved: what one stretch of a line is painted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paint {
    /// Where, in bytes of the line's text.
    pub span: Range<usize>,
    /// The text's colour.
    pub color: Option<Color>,
    /// Behind the text.
    pub background: Option<Color>,
    /// Bold.
    pub bold: bool,
}

/// A look over a stretch of the shown text, with what ranks it.
struct Laid<'a> {
    span: Range<usize>,
    look: &'a Look,
    /// Priority, highest first; then the match or a group over the line;
    /// then the trigger's rank.
    rank: (Reverse<i32>, bool, usize),
}

/// A substitute: this stretch of the game's text, replaced.
struct Cut {
    span: Range<usize>,
    with: String,
}

impl Matcher {
    /// What the triggers make of `line`, for the character `state` is: what
    /// is shown, and what fired.
    #[must_use]
    pub fn answer(&self, line: &Line, state: &GameState) -> Answer {
        self.answer_for(line, Some(state))
    }

    /// The lines a viewer is given for `line`, with no character to read:
    /// as if every `only_if` held. What `;trigger test` previews, naming
    /// each so the player knows it was not read.
    #[must_use]
    pub fn respond(&self, line: &Line) -> Vec<Line> {
        self.answer_for(line, None).lines
    }

    fn answer_for(&self, line: &Line, state: Option<&GameState>) -> Answer {
        if self.triggers.is_empty() {
            return Answer {
                lines: vec![line.clone()],
                ..Answer::default()
            };
        }
        let text = line.text();
        let hits = self.screened(line, &text, state);
        let mut firsts: Vec<&Hit> = hits.iter().collect();
        firsts.dedup_by_key(|hit| hit.trigger);
        Answer {
            lines: self.shown(line, &text, &hits),
            fired: firsts.iter().map(|hit| hit.trigger).collect(),
            attention: firsts
                .iter()
                .filter_map(|hit| self.attention(hit.trigger, Some(hit), &text))
                .collect(),
        }
    }

    /// The lines `hits` make of `line`, whose text is `text`.
    fn shown(&self, line: &Line, text: &str, hits: &[Hit]) -> Vec<Line> {
        let fired: Vec<(&Hit, &Rule)> = hits
            .iter()
            .filter_map(|hit| Some((hit, &self.triggers.get(hit.trigger)?.rule)))
            .collect();
        if fired.is_empty() {
            return vec![line.clone()];
        }
        if fired.iter().any(|(_, rule)| rule.squelch) {
            return Vec::new();
        }
        let cuts = self.cuts(&fired, text);
        let mut looks: Vec<Laid> = fired
            .iter()
            .filter_map(|(hit, rule)| {
                let look = rule.look.as_ref()?;
                let span = match look.span {
                    Span::Match => hit.span.clone(),
                    Span::Line => 0..text.len(),
                    Span::Group(group) => hit.groups.get(group.checked_sub(1)?)?.clone()?,
                };
                Some(Laid {
                    span: moved(&cuts, &span),
                    look,
                    rank: (Reverse(rule.priority), look.span == Span::Line, hit.trigger),
                })
            })
            .collect();
        looks.sort_by_key(|laid| laid.rank);
        let shown = Line {
            stream: line.stream.clone(),
            runs: if cuts.is_empty() {
                line.runs.clone()
            } else {
                cut(&line.runs, &cuts)
            },
            paint: paint(&looks),
        };
        match fired.iter().find_map(|(_, rule)| rule.redirect.as_ref()) {
            None => vec![shown],
            Some(to) if to.copy => {
                let copy = Line {
                    stream: to.stream.clone(),
                    ..shown.clone()
                };
                vec![shown, copy]
            }
            Some(to) => vec![Line {
                stream: to.stream.clone(),
                ..shown
            }],
        }
    }

    /// The substitutes to make, in the order they fall in `text`.
    fn cuts(&self, fired: &[(&Hit, &Rule)], text: &str) -> Vec<Cut> {
        let mut cuts: Vec<Cut> = Vec::new();
        for (hit, rule) in fired {
            let Some(template) = &rule.substitute else {
                continue;
            };
            let overlaps = cuts
                .iter()
                .any(|cut| cut.span.start < hit.span.end && hit.span.start < cut.span.end);
            if overlaps {
                continue;
            }
            cuts.push(Cut {
                span: hit.span.clone(),
                with: self.expand(hit, template, text),
            });
        }
        cuts.sort_by_key(|cut| cut.span.start);
        cuts
    }

    /// `template` for `hit`: its groups filled in for a regex, as written
    /// for a literal.
    pub(super) fn expand(&self, hit: &Hit, template: &str, text: &str) -> String {
        let captures = self
            .regexes
            .iter()
            .find(|(rank, _)| *rank == hit.trigger)
            .and_then(|(_, regex)| regex.captures_at(text, hit.span.start))
            .filter(|captures| {
                captures.get(0).map(|whole| whole.range()) == Some(hit.span.clone())
            });
        let Some(captures) = captures else {
            return template.to_owned();
        };
        let mut with = String::new();
        captures.expand(template, &mut with);
        with
    }
}

/// `runs` with each cut made: the text outside the cuts keeps its runs, and a
/// cut's text takes the style and link of the run it starts in.
fn cut(runs: &Runs, cuts: &[Cut]) -> Runs {
    let mut at = 0;
    let placed: Vec<(Range<usize>, &Run)> = runs
        .runs
        .iter()
        .map(|run| {
            let span = at..at + run.text.len();
            at = span.end;
            (span, run)
        })
        .collect();
    let mut out = Vec::new();
    let mut from = 0;
    for cut in cuts {
        keep(&placed, from..cut.span.start, &mut out);
        let owner = placed
            .iter()
            .find(|(span, _)| span.contains(&cut.span.start))
            .or_else(|| placed.last());
        if let Some((_, run)) = owner
            && !cut.with.is_empty()
        {
            out.push(Run {
                text: cut.with.clone(),
                ..(*run).clone()
            });
        }
        from = cut.span.end;
    }
    keep(&placed, from..at, &mut out);
    Runs { runs: out }
}

/// The parts of `placed` inside `range`, onto `out`.
fn keep(placed: &[(Range<usize>, &Run)], range: Range<usize>, out: &mut Vec<Run>) {
    for (span, run) in placed {
        let start = span.start.max(range.start);
        let end = span.end.min(range.end);
        if start >= end {
            continue;
        }
        if let Some(text) = run.text.get(start - span.start..end - span.start) {
            out.push(Run {
                text: text.to_owned(),
                ..(*run).clone()
            });
        }
    }
}

/// `span` of the game's text, where it lands once `cuts` are made. An end
/// inside a cut lands at the end of its text, and a start inside one at its
/// start, so a look over replaced words covers what replaced them.
fn moved(cuts: &[Cut], span: &Range<usize>) -> Range<usize> {
    let at = |old: usize, is_end: bool| {
        let (mut added, mut removed) = (0, 0);
        for cut in cuts {
            let (start, end) = (cut.span.start, cut.span.end);
            if start < old && old < end {
                let landed = start.saturating_sub(removed) + added;
                return if is_end {
                    landed + cut.with.len()
                } else {
                    landed
                };
            }
            if end <= old && start < old {
                added += cut.with.len();
                removed += end - start;
            }
        }
        old.saturating_sub(removed) + added
    };
    at(span.start, false)..at(span.end, true)
}

/// Resolve `looks`, best first, into the paint each stretch of the line gets.
fn paint(looks: &[Laid]) -> Vec<Paint> {
    let mut edges: Vec<usize> = looks
        .iter()
        .flat_map(|laid| [laid.span.start, laid.span.end])
        .collect();
    edges.sort_unstable();
    edges.dedup();
    let mut out: Vec<Paint> = Vec::new();
    for pair in edges.windows(2) {
        let &[start, end] = pair else {
            continue;
        };
        let (mut color, mut background, mut bold) = (None, None, false);
        for laid in looks
            .iter()
            .filter(|laid| laid.span.start <= start && end <= laid.span.end)
        {
            color = color.or(laid.look.color);
            background = background.or(laid.look.background);
            bold |= laid.look.bold;
        }
        if color.is_none() && background.is_none() && !bold {
            continue;
        }
        if let Some(last) = out.last_mut()
            && last.span.end == start
            && (last.color, last.background, last.bold) == (color, background, bold)
        {
            last.span.end = end;
            continue;
        }
        out.push(Paint {
            span: start..end,
            color,
            background,
            bold,
        });
    }
    out
}
