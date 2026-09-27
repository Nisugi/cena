//! `plan/45` Stage 5: what a trigger sends, and the pace its sends keep.

use cena_model::GameState;
use cena_model::guard::Condition;
use cena_model::line::Line;
use cena_model::state::chunks::ChunkLine;
use cena_model::trigger::{Act, MAX_SENDS, Matcher, Pace, Pattern, Rule, SEND_WINDOW, Trigger};

fn line(text: &str) -> Line {
    Line::new("", ChunkLine::plain(text).runs)
}

fn act(trigger: &str) -> Act {
    Act {
        trigger: trigger.into(),
        line: "stand".into(),
        cooldown: 3,
    }
}

#[test]
fn a_send_fills_in_its_groups_on_a_squelched_line_too() {
    let matcher = Matcher::new(vec![Trigger {
        name: "react".into(),
        rule: Rule {
            pattern: Some(Pattern::Regex(
                r"You could use this opportunity to (\w+)".into(),
            )),
            squelch: true,
            send: Some("weapon $1".into()),
            ..Rule::default()
        },
    }])
    .unwrap();
    let answer = matcher.answer(
        &line("You could use this opportunity to tackle!"),
        &GameState::default(),
    );
    assert!(answer.lines.is_empty());
    assert_eq!(
        answer.acts,
        [Act {
            trigger: "react".into(),
            line: "weapon tackle".into(),
            cooldown: 3,
        }]
    );
}

#[test]
fn a_condition_sends_its_line_as_written() {
    let matcher = Matcher::new(vec![Trigger {
        name: "low".into(),
        rule: Rule {
            condition: Condition::parse_group("!health_at_least 30").unwrap_or_default(),
            send: Some(";heal".into()),
            ..Rule::default()
        },
    }])
    .unwrap();
    assert_eq!(
        matcher.condition_act(0).map(|act| act.line),
        Some(";heal".to_owned())
    );
}

#[test]
fn triggers_send_at_most_five_lines_in_ten_seconds_between_them() {
    let mut pace = Pace::default();
    let many = |count: usize| {
        (0..count)
            .map(|i| act(&format!("t{i}")))
            .collect::<Vec<_>>()
    };
    let (sent, held) = pace.admit(many(MAX_SENDS + 2), Some(100));
    assert_eq!((sent.len(), held.len()), (MAX_SENDS, 2));
    let (sent, held) = pace.admit(many(1), Some(100 + SEND_WINDOW - 1));
    assert_eq!((sent.len(), held.len()), (0, 1), "the window is still full");
    let (sent, held) = pace.admit(many(1), Some(100 + SEND_WINDOW));
    assert_eq!(
        (sent.len(), held.len()),
        (1, 0),
        "the first five have left it"
    );
    // With no clock the pace cannot be kept, and nothing is sent.
    let (sent, held) = Pace::default().admit(many(1), None);
    assert_eq!((sent.len(), held.len()), (0, 1));
}
