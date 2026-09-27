//! `plan/45` Stage 3: what a trigger's sound, notification and banner say,
//! and how often they come.

use cena_model::GameState;
use cena_model::guard::Condition;
use cena_model::line::Line;
use cena_model::state::chunks::ChunkLine;
use cena_model::trigger::{Attention, Cooldowns, Matcher, Pattern, Rule, Say, Trigger};

fn line(text: &str) -> Line {
    Line::new("", ChunkLine::plain(text).runs)
}

fn trigger(name: &str, rule: Rule) -> Trigger {
    Trigger {
        name: name.into(),
        rule,
    }
}

#[test]
fn true_says_the_line_as_the_game_sent_it_and_words_fill_in_groups() {
    let matcher = Matcher::new(vec![
        trigger(
            "whisper",
            Rule {
                pattern: Some(Pattern::Regex(r"^(\w+) whispers".into())),
                notify: Some(Say::Words("$1 whispered".into())),
                alert: Some(Say::Line),
                sound: Some("ding.wav".into()),
                ..Rule::default()
            },
        ),
        trigger(
            "hide it",
            Rule {
                pattern: Some(Pattern::Literal {
                    text: "whispers".into(),
                    whole_word: true,
                }),
                squelch: true,
                substitute: Some("says".into()),
                ..Rule::default()
            },
        ),
    ])
    .unwrap();
    let answer = matcher.answer(&line("Dicate whispers, \"hi\""), &GameState::default());
    assert!(answer.lines.is_empty(), "squelched");
    assert_eq!(
        answer.attention,
        [Attention {
            trigger: "whisper".into(),
            sound: Some("ding.wav".into()),
            notify: Some("Dicate whispered".into()),
            alert: Some("Dicate whispers, \"hi\"".into()),
            cooldown: 3,
        }],
        "on a squelched line, in the game's words"
    );
}

#[test]
fn a_condition_says_its_name_and_a_trigger_with_no_attention_calls_for_none() {
    let matcher = Matcher::new(vec![
        trigger(
            "low health",
            Rule {
                condition: Condition::parse_group("!health_at_least 30").unwrap_or_default(),
                notify: Some(Say::Line),
                alert: Some(Say::Words("Heal!".into())),
                ..Rule::default()
            },
        ),
        trigger(
            "quiet",
            Rule {
                pattern: Some(Pattern::Literal {
                    text: "x".into(),
                    whole_word: true,
                }),
                squelch: true,
                ..Rule::default()
            },
        ),
    ])
    .unwrap();
    let called = matcher.condition_attention(0).unwrap();
    assert_eq!(called.notify.as_deref(), Some("low health"));
    assert_eq!(called.alert.as_deref(), Some("Heal!"));
    assert_eq!(matcher.condition_attention(1), None);
    let answer = matcher.answer(&line("x"), &GameState::default());
    assert_eq!(answer.fired, [1]);
    assert!(answer.attention.is_empty());
}

#[test]
fn a_trigger_does_what_it_does_beyond_the_line_at_most_once_in_its_cooldown() {
    let mut cooldowns = Cooldowns::default();
    assert!(cooldowns.admit("a", 3, Some(100)));
    assert!(cooldowns.admit("b", 0, Some(100)));
    assert!(
        !cooldowns.admit("a", 3, Some(102)),
        "a cools for three seconds"
    );
    assert!(cooldowns.admit("b", 0, Some(102)), "b has none");
    assert!(
        cooldowns.admit("a", 3, Some(103)),
        "cooled, from 100 and not from the call it held back"
    );
    // With no clock, each may, and none starts a cooldown.
    let mut fresh = Cooldowns::default();
    assert!(fresh.admit("a", 3, None));
    assert!(fresh.admit("a", 3, Some(100)));
}
