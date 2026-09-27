//! What a character's triggers cost a line (`plan/45` §6, Stage 1 step 7).
//!
//! **A measurement, not a check**, so it is `#[ignore]`d: a debug build's
//! timings say nothing about the budget, and the author builds debug for
//! testing (`CLAUDE.md`). The budget is set from a release run of this file,
//! which the author runs or approves:
//!
//! ```text
//! cargo test -p cena-model --release --test trigger_bench -- --ignored --nocapture
//! ```
//!
//! The lines are every line the model finishes from the 23 golden fixtures
//! (`crates/cena-protocol/tests/fixtures`), found as the session finds them.
//! The triggers are synthetic: a quarter on words those lines hold, so some
//! hit as a real set's do, the rest on names that never appear; one in ten a
//! regex, near `VellumFE`'s measured 53 of 505 (`plan/45` §2a). 220 is the
//! size of the author's Wrayth set (§2d), 1,500 a veteran's (§2b). The
//! author's own set is measured when Stage 4 can import it.

use std::hint::black_box;
use std::path::Path;
use std::time::{Duration, Instant};

use cena_model::GameState;
use cena_model::line::Line;
use cena_model::trigger::{Color, Look, Matcher, Pattern, Rule, Span, Trigger};
use cena_protocol::Frame;

/// Every line the model finishes from the golden fixtures.
fn lines() -> Option<Vec<Line>> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../cena-protocol/tests/fixtures");
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| Some(entry.ok()?.path()))
        .filter(|path| path.extension().is_some_and(|e| e == "xml"))
        .collect();
    paths.sort();
    let mut lines = Vec::new();
    for path in paths {
        let bytes = std::fs::read(path).ok()?;
        let mut parser = cena_protocol::Parser::new();
        let mut state = GameState::default();
        for frame in parser.push_bytes(&bytes) {
            let before = state.lines_seen();
            state.apply(&frame);
            if let Frame::Text(text) = &frame
                && state.lines_seen() != before
                && let Some(runs) = state.stream(&text.stream).last()
            {
                lines.push(Line::new(text.stream.clone(), runs.clone()));
            }
        }
    }
    Some(lines)
}

/// `count` synthetic triggers over `lines`.
fn triggers(count: usize, lines: &[Line]) -> Vec<Trigger> {
    let mut words: Vec<String> = lines
        .iter()
        .flat_map(|line| {
            line.text()
                .split_whitespace()
                .filter(|word| word.len() > 4 && word.chars().all(char::is_alphabetic))
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect();
    words.sort();
    words.dedup();
    (0..count)
        .map(|index| {
            let word = words
                .get(index / 4 % words.len().max(1))
                .filter(|_| index % 4 == 0)
                .cloned()
                .unwrap_or_else(|| format!("Name{index}"));
            let pattern = if index % 10 == 9 {
                Pattern::Regex(format!(r"\b{word} (\w+)"))
            } else {
                Pattern::Literal {
                    text: word,
                    whole_word: true,
                }
            };
            Trigger {
                name: format!("t{index}"),
                rule: Rule {
                    pattern: Some(pattern),
                    look: Some(Look {
                        color: Some(Color {
                            red: 0xff,
                            green: 0x40,
                            blue: 0x40,
                        }),
                        background: None,
                        bold: false,
                        span: Span::Match,
                    }),
                    ..Rule::default()
                },
            }
        })
        .collect()
}

/// `respond` over every line, `rounds` times: the time it took, and the
/// lines answered.
fn run(matcher: &Matcher, lines: &[Line], rounds: usize) -> (Duration, usize) {
    let start = Instant::now();
    for _ in 0..rounds {
        for line in lines {
            black_box(matcher.respond(black_box(line)));
        }
    }
    (start.elapsed(), rounds * lines.len())
}

#[test]
#[ignore = "a measurement: run it in release to set the budget (plan/45 section 6, step 7)"]
fn what_triggers_cost_a_line() {
    let lines = lines().unwrap();
    println!("{} lines from the golden fixtures", lines.len());
    for count in [0, 220, 1500] {
        let matcher = Matcher::new(triggers(count, &lines)).unwrap();
        let hits: usize = lines
            .iter()
            .map(|line| matcher.hits(&line.stream, &line.text()).len())
            .sum();
        let _ = run(&matcher, &lines, 2);
        let (took, answered) = run(&matcher, &lines, 20);
        let per_line = took / u32::try_from(answered).unwrap();
        println!("{count:>5} triggers: {per_line:?} a line ({hits} hits over the lines)");
    }
}
