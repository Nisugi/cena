//! One compiled matcher over a character's triggers (`plan/45` §4): an
//! Aho-Corasick automaton for the literals where case matters, a second for
//! the literals where it does not, and a `RegexSet` for the regexes.
//! `VellumFE` compiles the same rules three times, with three meanings
//! (`plan/45` §2a); this is compiled once, and every response reads the same
//! hits.
//!
//! **Every place every trigger matches.** The automata are searched
//! overlapping, so `stun` and `you stun` both hit `you stun the rat`, and a
//! trigger hits each place its words stand, not only the first. Where one
//! trigger's own places overlap (`aa` in `aaa`), the earlier is kept.
//! Choosing between two triggers' looks is not the matcher's: it reports
//! both, in order.
//!
//! **The order is fixed.** Triggers rank by priority, highest first, then in
//! the order given, which the file makes category then name; hits come in
//! rank order, then by where they start. Nothing is gathered in hash order,
//! which is how `VellumFE`'s regex matches could come out differently from
//! one run to the next (`plan/45` §2a).
//!
//! **The set only nominates**, as in the crit tables
//! (`crates/cena-model/src/crit/match_index.rs`): it says which regexes match
//! at all, with the same raised lazy-DFA budget, and each nominee then finds
//! its own places. If the set cannot be built, every regex is tried, which is
//! slower and gives the same hits.

use std::cmp::Reverse;
use std::ops::Range;

use aho_corasick::AhoCorasick;
use regex::{Regex, RegexSet, RegexSetBuilder};

use super::{Pattern, Trigger, regex};

/// The regex set's lazy-DFA budget: the crit tables' `RESIDUAL_DFA_LIMIT`.
const DFA_LIMIT: usize = 1 << 26;

/// A character's triggers, compiled to be matched against each finished
/// line. The default has none, and matches nothing.
#[derive(Debug, Clone, Default)]
pub struct Matcher {
    pub(super) triggers: Vec<Trigger>,
    sensitive: Literals,
    insensitive: Literals,
    /// Which regexes match a line; `None` when it could not be built.
    set: Option<RegexSet>,
    /// Each regex, by its place in the set, with its trigger's rank.
    pub(super) regexes: Vec<(usize, Regex)>,
}

/// One trigger matching one place in a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// The trigger, by its rank in [`Matcher::triggers`].
    pub trigger: usize,
    /// Where, in bytes of the line's text.
    pub span: Range<usize>,
    /// A regex's groups, from 1, where each matched; empty for a literal.
    pub groups: Vec<Option<Range<usize>>>,
}

/// One automaton's literals, each with its trigger's rank.
#[derive(Debug, Clone, Default)]
struct Literals {
    automaton: Option<AhoCorasick>,
    /// By pattern: the trigger's rank, and whether it must stand alone.
    patterns: Vec<(usize, bool)>,
}

impl Matcher {
    /// Compile `triggers`, given in file order.
    ///
    /// # Errors
    ///
    /// A regex or an automaton cannot be built. A trigger read from the file
    /// has already been checked, so this is a trigger built some other way.
    pub fn new(mut triggers: Vec<Trigger>) -> Result<Self, String> {
        // Stable, so equal priorities keep the order they were given in.
        triggers.sort_by_key(|trigger| Reverse(trigger.rule.priority));
        let (mut sensitive, mut insensitive) = (Vec::new(), Vec::new());
        let (mut sources, mut regexes) = (Vec::new(), Vec::new());
        for (rank, trigger) in triggers.iter().enumerate() {
            let case_sensitive = trigger.rule.case_sensitive;
            match &trigger.rule.pattern {
                Pattern::Literal { text, whole_word } => {
                    let into = if case_sensitive {
                        &mut sensitive
                    } else {
                        &mut insensitive
                    };
                    into.push((text.as_str(), rank, *whole_word));
                }
                Pattern::Regex(source) => {
                    let built = regex(source, case_sensitive)
                        .map_err(|e| format!("`{}`: {e}", trigger.name))?;
                    sources.push(if case_sensitive {
                        source.clone()
                    } else {
                        format!("(?i){source}")
                    });
                    regexes.push((rank, built));
                }
            }
        }
        let set = RegexSetBuilder::new(&sources)
            .dfa_size_limit(DFA_LIMIT)
            .build()
            .ok();
        Ok(Self {
            sensitive: Literals::new(&sensitive, false)?,
            insensitive: Literals::new(&insensitive, true)?,
            triggers,
            set,
            regexes,
        })
    }

    /// The triggers, in rank order: what a [`Hit::trigger`] indexes.
    #[must_use]
    pub fn triggers(&self) -> &[Trigger] {
        &self.triggers
    }

    /// Every trigger that matches `text`, a finished line on `stream`, at
    /// every place it matches: in rank order, then by where each starts.
    #[must_use]
    pub fn hits(&self, stream: &str, text: &str) -> Vec<Hit> {
        let mut hits = Vec::new();
        self.sensitive.find(text, &mut hits);
        self.insensitive.find(text, &mut hits);
        let nominated: Vec<usize> = match &self.set {
            Some(set) => set.matches(text).into_iter().collect(),
            None => (0..self.regexes.len()).collect(),
        };
        for index in nominated {
            let Some((rank, regex)) = self.regexes.get(index) else {
                continue;
            };
            for captures in regex.captures_iter(text) {
                let Some(whole) = captures.get(0) else {
                    continue;
                };
                hits.push(Hit {
                    trigger: *rank,
                    span: whole.range(),
                    groups: captures
                        .iter()
                        .skip(1)
                        .map(|group| group.map(|g| g.range()))
                        .collect(),
                });
            }
        }
        hits.retain(|hit| {
            self.triggers
                .get(hit.trigger)
                .is_some_and(|trigger| watches(trigger.rule.stream.as_deref(), stream))
        });
        hits.sort_by_key(|hit| (hit.trigger, hit.span.start));
        let mut kept: Vec<Hit> = Vec::with_capacity(hits.len());
        for hit in hits {
            let overlaps_own = kept
                .last()
                .is_some_and(|last| last.trigger == hit.trigger && hit.span.start < last.span.end);
            if !overlaps_own {
                kept.push(hit);
            }
        }
        kept
    }
}

impl Literals {
    fn new(literals: &[(&str, usize, bool)], ignore_case: bool) -> Result<Self, String> {
        if literals.is_empty() {
            return Ok(Self::default());
        }
        let automaton = AhoCorasick::builder()
            .ascii_case_insensitive(ignore_case)
            .build(literals.iter().map(|(text, _, _)| text))
            .map_err(|e| format!("the triggers' words cannot be built together: {e}"))?;
        Ok(Self {
            automaton: Some(automaton),
            patterns: literals
                .iter()
                .map(|&(_, rank, whole_word)| (rank, whole_word))
                .collect(),
        })
    }

    fn find(&self, text: &str, hits: &mut Vec<Hit>) {
        let Some(automaton) = &self.automaton else {
            return;
        };
        for found in automaton.find_overlapping_iter(text) {
            let Some(&(rank, whole_word)) = self.patterns.get(found.pattern().as_usize()) else {
                continue;
            };
            if whole_word && !stands_alone(text, &found.range()) {
                continue;
            }
            hits.push(Hit {
                trigger: rank,
                span: found.range(),
                groups: Vec::new(),
            });
        }
    }
}

/// Whether a trigger limited to `wanted` looks at a line on `stream`; the
/// main stream is `""` or `main`.
fn watches(wanted: Option<&str>, stream: &str) -> bool {
    let main = |s: &str| s.is_empty() || s.eq_ignore_ascii_case("main");
    wanted
        .is_none_or(|wanted| wanted.eq_ignore_ascii_case(stream) || (main(wanted) && main(stream)))
}

/// Whether `span` is whole words: at each end, an edge that is a letter, a
/// digit or `_` is not touched by another (the `trigger` module docs).
fn stands_alone(text: &str, span: &Range<usize>) -> bool {
    let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    let matched = text.get(span.clone()).unwrap_or_default();
    let before = text.get(..span.start).and_then(|s| s.chars().next_back());
    let after = text.get(span.end..).and_then(|s| s.chars().next());
    let joined_before = word(matched.chars().next()) && word(before);
    let joined_after = word(matched.chars().next_back()) && word(after);
    !joined_before && !joined_after
}
