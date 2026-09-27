//! What a character's triggers do to a finished line
//! (`crates/cena-model/src/trigger/respond.rs`): squelch, substitute, look
//! and redirect, over lines the real parser and model finish.

use cena_model::GameState;
use cena_model::line::Line;
use cena_model::trigger::{Color, Look, Matcher, Paint, Pattern, Redirect, Rule, Span, Trigger};
use cena_protocol::frame::LinkKind;

/// The main-stream line the model finishes from `wire`.
fn finished(wire: &str) -> Option<Line> {
    let mut parser = cena_protocol::Parser::new();
    let mut state = GameState::default();
    for frame in parser.push_bytes(format!("{wire}\n").as_bytes()) {
        state.apply(&frame);
    }
    Some(Line::new("", state.stream("").last()?.clone()))
}

/// A trigger on `pattern` that does nothing until `edit` says what.
fn trigger(name: &str, pattern: Pattern, edit: impl FnOnce(&mut Rule)) -> Trigger {
    let mut rule = Rule {
        pattern: Some(pattern),
        ..Rule::default()
    };
    edit(&mut rule);
    Trigger {
        name: name.into(),
        rule,
    }
}

fn words(text: &str) -> Pattern {
    Pattern::Literal {
        text: text.into(),
        whole_word: true,
    }
}

fn regex(source: &str) -> Pattern {
    Pattern::Regex(source.into())
}

fn look(color: Option<Color>, background: Option<Color>, span: Span) -> Look {
    Look {
        color,
        background,
        bold: false,
        span,
    }
}

const RED: Color = Color {
    red: 0xff,
    green: 0,
    blue: 0,
};
const GREEN: Color = Color {
    red: 0,
    green: 0xff,
    blue: 0,
};
const BLUE: Color = Color {
    red: 0,
    green: 0,
    blue: 0xff,
};

/// What `triggers` make of `line`.
fn respond(triggers: Vec<Trigger>, line: &Line) -> Option<Vec<Line>> {
    Some(Matcher::new(triggers).ok()?.respond(line))
}

/// The one line `triggers` make of `line`.
fn one(triggers: Vec<Trigger>, line: &Line) -> Option<Line> {
    let mut shown = respond(triggers, line)?;
    (shown.len() == 1).then(|| shown.remove(0))
}

fn paint(span: std::ops::Range<usize>, color: Option<Color>, background: Option<Color>) -> Paint {
    Paint {
        span,
        color,
        background,
        bold: false,
    }
}

#[test]
fn a_line_nothing_matches_is_given_as_it_came() {
    let line = finished("You see <a exist=\"1\" noun=\"rat\">a pale green rat</a> here.").unwrap();
    let none = one(
        vec![trigger("orc", words("orc"), |r| r.squelch = true)],
        &line,
    )
    .unwrap();
    assert_eq!(none, line);
    assert_eq!(one(Vec::new(), &line).unwrap(), line);
}

#[test]
fn a_squelch_hides_the_line_whatever_else_matched() {
    let line = finished("The rat squeaks.").unwrap();
    let shown = respond(
        vec![
            trigger("colour", words("rat"), |r| {
                r.look = Some(look(Some(RED), None, Span::Match));
                r.redirect = Some(Redirect {
                    stream: "combat".into(),
                    copy: true,
                });
            }),
            trigger("hide", words("squeaks"), |r| r.squelch = true),
        ],
        &line,
    )
    .unwrap();
    assert!(shown.is_empty(), "{shown:?}");
}

/// The substitute takes the run it starts in: the rat is still the rat's
/// link, so a click on `RAT` acts on the creature.
#[test]
fn a_substitute_replaces_the_words_and_keeps_the_link() {
    let line = finished("You see <a exist=\"1\" noun=\"rat\">a pale green rat</a> here.").unwrap();
    let shown = one(
        vec![trigger("rat", words("pale green rat"), |r| {
            r.substitute = Some("RAT".into());
        })],
        &line,
    )
    .unwrap();
    assert_eq!(shown.text(), "You see a RAT here.");
    let rat = shown
        .runs
        .runs
        .iter()
        .find(|run| run.text == "RAT")
        .unwrap();
    assert!(
        matches!(&rat.object().unwrap().kind, LinkKind::Exist { noun, .. } if noun == "rat"),
        "{rat:?}"
    );
    assert_eq!(
        line.text(),
        "You see a pale green rat here.",
        "the input is untouched"
    );
}

#[test]
fn a_regex_substitute_fills_in_its_groups() {
    let line = finished("Dicate whispers, \"meet me at the gate\"").unwrap();
    let shown = one(
        vec![trigger(
            "whisper",
            regex(r#"^(\w+) whispers, "(.*)"$"#),
            |r| {
                r.substitute = Some("${1}: $2".into());
            },
        )],
        &line,
    )
    .unwrap();
    assert_eq!(shown.text(), "Dicate: meet me at the gate");
    // A literal's `$1` is only text.
    let literal = one(
        vec![trigger("dollar", words("whispers"), |r| {
            r.substitute = Some("$1".into());
        })],
        &line,
    )
    .unwrap();
    assert_eq!(literal.text(), "Dicate $1, \"meet me at the gate\"");
}

/// Two substitutes over the same words: the better-ranked one is made, the
/// other skipped. One elsewhere in the line is made as well.
#[test]
fn of_two_overlapping_substitutes_the_better_ranked_is_made() {
    let line = finished("You stun the rat and the rat runs.").unwrap();
    let shown = one(
        vec![
            trigger("you-stun", words("You stun"), |r| {
                r.substitute = Some("STUN!".into());
            }),
            trigger("stun", words("stun"), |r| r.substitute = Some("zap".into())),
            trigger("runs", words("runs"), |r| {
                r.substitute = Some("flees".into());
            }),
        ],
        &line,
    )
    .unwrap();
    assert_eq!(shown.text(), "STUN! the rat and the rat flees.");
}

#[test]
fn a_look_paints_the_match_a_group_or_the_line() {
    let line = finished("Roundtime: 3 sec.").unwrap();
    let text = |shown: &Line, paint: &Paint| shown.text()[paint.span.clone()].to_owned();

    let matched = one(
        vec![trigger("rt", words("Roundtime"), |r| {
            r.look = Some(look(Some(RED), None, Span::Match));
        })],
        &line,
    )
    .unwrap();
    assert_eq!(matched.paint, [paint(0..9, Some(RED), None)]);
    assert_eq!(text(&matched, &matched.paint[0]), "Roundtime");

    let group = one(
        vec![trigger("rt", regex(r"Roundtime: (\d+) sec"), |r| {
            r.look = Some(look(Some(RED), None, Span::Group(1)));
        })],
        &line,
    )
    .unwrap();
    assert_eq!(text(&group, &group.paint[0]), "3");

    let whole = one(
        vec![trigger("rt", words("Roundtime"), |r| {
            r.look = Some(look(None, Some(BLUE), Span::Line));
        })],
        &line,
    )
    .unwrap();
    assert_eq!(whole.paint, [paint(0..17, None, Some(BLUE))]);
}

/// A whole-line look is the backdrop; a word's look sits on it, field by
/// field, so the line's background shows behind the word's colour.
#[test]
fn a_words_look_sits_on_the_lines_backdrop() {
    let line = finished("The rat bites you.").unwrap();
    let shown = one(
        vec![
            // Given first, so file order alone would put it on top.
            trigger("line", words("bites you"), |r| {
                r.look = Some(look(Some(GREEN), Some(BLUE), Span::Line));
            }),
            trigger("rat", words("rat"), |r| {
                r.look = Some(look(Some(RED), None, Span::Match));
            }),
        ],
        &line,
    )
    .unwrap();
    assert_eq!(
        shown.paint,
        [
            paint(0..4, Some(GREEN), Some(BLUE)),
            paint(4..7, Some(RED), Some(BLUE)),
            paint(7..18, Some(GREEN), Some(BLUE)),
        ]
    );
}

/// Priority is said on purpose, so it beats the match-over-line rule.
#[test]
fn priority_beats_a_narrower_look() {
    let line = finished("The rat bites you.").unwrap();
    let shown = one(
        vec![
            trigger("line", words("bites you"), |r| {
                r.priority = 5;
                r.look = Some(look(Some(GREEN), None, Span::Line));
            }),
            trigger("rat", words("rat"), |r| {
                r.look = Some(look(Some(RED), None, Span::Match));
            }),
        ],
        &line,
    )
    .unwrap();
    assert_eq!(shown.paint, [paint(0..18, Some(GREEN), None)]);
}

/// A substitute before a look moves it; a look over replaced words covers
/// what replaced them.
#[test]
fn a_look_follows_the_text_a_substitute_moved() {
    let line = finished("Nisugi says, the pale green rat is here.").unwrap();
    let shown = one(
        vec![
            trigger("name", words("Nisugi"), |r| r.substitute = Some("N".into())),
            trigger("rat", words("pale green rat"), |r| {
                r.substitute = Some("RAT".into());
                r.look = Some(look(Some(RED), None, Span::Match));
            }),
            trigger("here", words("here"), |r| {
                r.look = Some(look(Some(BLUE), None, Span::Match));
            }),
        ],
        &line,
    )
    .unwrap();
    let text = shown.text();
    assert_eq!(text, "N says, the RAT is here.");
    let painted: Vec<&str> = shown
        .paint
        .iter()
        .map(|paint| &text[paint.span.clone()])
        .collect();
    assert_eq!(painted, ["RAT", "here"]);
}

#[test]
fn a_redirect_moves_or_copies_the_line_and_the_best_ranked_decides() {
    let line = finished("Dicate whispers, \"hello\"").unwrap();
    let to = |stream: &str, copy: bool| {
        Some(Redirect {
            stream: stream.into(),
            copy,
        })
    };
    let moved = respond(
        vec![
            trigger("first", words("whispers"), |r| {
                r.redirect = to("whispers", false);
            }),
            trigger("second", words("Dicate"), |r| {
                r.redirect = to("friends", true);
            }),
        ],
        &line,
    )
    .unwrap();
    let streams: Vec<&str> = moved.iter().map(|line| line.stream.as_str()).collect();
    assert_eq!(streams, ["whispers"]);

    let copied = respond(
        vec![trigger("copy", words("whispers"), |r| {
            r.redirect = to("whispers", true);
        })],
        &line,
    )
    .unwrap();
    let streams: Vec<&str> = copied.iter().map(|line| line.stream.as_str()).collect();
    assert_eq!(streams, ["", "whispers"]);
    assert_eq!(copied[0].runs, copied[1].runs);
}

/// Stage 7: a name in the room window's players is only painted -- a
/// squelch or a substitute on it does nothing there -- by the triggers on
/// every stream or on `room players`, never one kept to another, and only
/// while its `only_if` holds.
#[test]
fn an_entry_in_a_list_is_only_painted() {
    let state = GameState::default();
    let matcher = Matcher::new(vec![
        trigger("friend", words("Maravel"), |rule| {
            rule.look = Some(look(Some(RED), None, Span::Match));
        }),
        trigger("hush", words("Maravel"), |rule| rule.squelch = true),
        trigger("rename", words("Orsen"), |rule| {
            rule.substitute = Some("Bob".into());
        }),
        trigger("main only", words("Orsen"), |rule| {
            rule.stream = Some(String::new());
            rule.look = Some(look(Some(GREEN), None, Span::Line));
        }),
        trigger("window only", words("Nythra"), |rule| {
            rule.stream = Some("room players".into());
            rule.look = Some(look(None, Some(GREEN), Span::Line));
        }),
        trigger("while hidden", words("Kelmond"), |rule| {
            rule.only_if = cena_model::guard::Condition::parse_group("hidden").unwrap_or_default();
            rule.look = Some(look(Some(RED), None, Span::Line));
        }),
    ])
    .unwrap_or_default();
    let entry = |text: &str| matcher.paint_entry("room players", text, &state);

    let maravel = entry("Maravel");
    assert_eq!(maravel.text(), "Maravel", "not hidden");
    assert_eq!(
        maravel.paint,
        [Paint {
            span: 0..7,
            color: Some(RED),
            background: None,
            bold: false,
        }]
    );
    let orsen = entry("Orsen");
    assert_eq!(orsen.text(), "Orsen", "not rewritten");
    assert!(
        orsen.paint.is_empty(),
        "kept to the main window: {:?}",
        orsen.paint
    );
    assert_eq!(entry("Nythra").paint.len(), 1, "kept to the room window");
    assert!(entry("Maravels").paint.is_empty(), "whole words");
    assert!(
        entry("Kelmond").paint.is_empty(),
        "not hidden, so not painted"
    );
}
