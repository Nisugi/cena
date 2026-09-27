//! `plan/45` Stages 2 and 3 in the session: a trigger's flag is set in the
//! session's state and published, so whoever folds the events sets it too;
//! a condition is read at each prompt; a call for attention is published,
//! once in its cooldown.

use cena_platform::ReplaySource;
use cena_session::flags::{FlagChange, Until};
use cena_session::guard::Condition;
use cena_session::trigger::{Flag, Matcher, Pattern, Rule, Say, Trigger};
use cena_session::{Event, GameState, Session};

/// What the session published over `wire` with `triggers`, and its state at
/// the end.
async fn run(wire: &str, triggers: Vec<Trigger>) -> (Vec<Event>, GameState) {
    let session = Session::new(ReplaySource::from_bytes(wire.as_bytes()));
    session
        .handle()
        .set_triggers(Matcher::new(triggers).unwrap_or_default());
    let (_, mut events) = session.subscribe();
    let end = Box::pin(session.into_actor().run()).await;
    let events = std::iter::from_fn(|| events.try_recv().ok()).collect();
    (events, end.state)
}

fn flags(events: &[Event]) -> Vec<FlagChange> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Flag(change) => Some(change.clone()),
            _ => None,
        })
        .collect()
}

fn flag(name: &str, seconds: Option<u32>) -> Flag {
    Flag {
        name: name.into(),
        seconds,
        clear: false,
    }
}

fn on_words(name: &str, text: &str, rule: Rule) -> Trigger {
    Trigger {
        name: name.into(),
        rule: Rule {
            pattern: Some(Pattern::Literal {
                text: text.into(),
                whole_word: true,
            }),
            ..rule
        },
    }
}

const SWING: &str = "You swing a broadsword at <pushBold/><a exist=\"101\" noun=\"lizard\">a cave lizard</a><popBold/>!\n";

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_squelched_line_sets_its_flag_and_the_change_is_published_once() {
    let wire = format!("<prompt time=\"999\">&gt;</prompt>\n{SWING}{SWING}");
    let (events, state) = run(
        &wire,
        vec![on_words(
            "swung",
            "You swing",
            Rule {
                squelch: true,
                flag: Some(flag("swung", None)),
                ..Rule::default()
            },
        )],
    )
    .await;
    assert!(
        !events.iter().any(|event| matches!(event, Event::Line(_))),
        "both swings squelched"
    );
    assert_eq!(
        flags(&events),
        [FlagChange {
            name: "swung".into(),
            until: Some(Until::Cleared),
        }],
        "the second swing changed nothing, so nothing more was published"
    );
    assert_eq!(
        state.flags.holds("swung", state.game_time_now()),
        Some(true)
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_timed_flag_runs_on_the_games_clock() {
    let wire = format!("<prompt time=\"999\">&gt;</prompt>\n{SWING}");
    let (events, _) = run(
        &wire,
        vec![on_words(
            "swung",
            "You swing",
            Rule {
                flag: Some(flag("swung", Some(30))),
                ..Rule::default()
            },
        )],
    )
    .await;
    assert_eq!(
        flags(&events).first().and_then(|change| change.until),
        Some(Until::Second(1029))
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_condition_is_read_at_each_prompt_and_fires_on_its_rise() {
    let wire = concat!(
        "<indicator id=\"IconHIDDEN\" visible=\"y\"/>\n<prompt time=\"100\">&gt;</prompt>\n",
        "<indicator id=\"IconHIDDEN\" visible=\"n\"/>\n<prompt time=\"101\">&gt;</prompt>\n",
        "<indicator id=\"IconHIDDEN\" visible=\"y\"/>\n<prompt time=\"102\">&gt;</prompt>\n",
    );
    let (events, state) = run(
        wire,
        vec![Trigger {
            name: "hid".into(),
            rule: Rule {
                condition: Condition::parse_group("hidden").unwrap_or_default(),
                flag: Some(flag("hid", None)),
                ..Rule::default()
            },
        }],
    )
    .await;
    // Hidden at the first prompt is taken silently; hidden again at the
    // third, after a prompt unhidden, is the rise.
    assert_eq!(
        flags(&events),
        [FlagChange {
            name: "hid".into(),
            until: Some(Until::Cleared),
        }]
    );
    let prompts_before_flag = events
        .iter()
        .take_while(|event| !matches!(event, Event::Flag(_)))
        .filter(|event| {
            matches!(event, Event::Frame(frame) if matches!(**frame, cena_session::Frame::Prompt { .. }))
        })
        .count();
    assert_eq!(prompts_before_flag, 3, "set at the third prompt");
    assert_eq!(state.flags.holds("hid", None), Some(true));
}

fn sounding(name: &str, text: &str) -> Trigger {
    on_words(
        name,
        text,
        Rule {
            sound: Some("ding.wav".into()),
            ..Rule::default()
        },
    )
}

fn called(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Attention(call) => Some(call.trigger.clone()),
            _ => None,
        })
        .collect()
}

/// A trigger's attention is published, at most once in its cooldown, by the
/// game's clock.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn attention_is_published_once_in_its_cooldown() {
    let wire = format!(
        "<prompt time=\"999\">&gt;</prompt>\n{SWING}{SWING}\
         <prompt time=\"1001\">&gt;</prompt>\n{SWING}\
         <prompt time=\"1002\">&gt;</prompt>\n{SWING}"
    );
    let (events, _) = run(&wire, vec![sounding("swing", "You swing")]).await;
    assert_eq!(
        called(&events),
        ["swing", "swing"],
        "at 999, and at 1002 once three seconds have passed; not at 999 again or 1001"
    );
}

/// A condition's attention says its name, at the prompt it fires on.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_condition_calls_for_attention_when_it_fires() {
    let wire = concat!(
        "<indicator id=\"IconHIDDEN\" visible=\"n\"/>\n<prompt time=\"100\">&gt;</prompt>\n",
        "<indicator id=\"IconHIDDEN\" visible=\"y\"/>\n<prompt time=\"101\">&gt;</prompt>\n",
    );
    let (events, _) = run(
        wire,
        vec![Trigger {
            name: "hid".into(),
            rule: Rule {
                condition: Condition::parse_group("hidden").unwrap_or_default(),
                notify: Some(Say::Line),
                ..Rule::default()
            },
        }],
    )
    .await;
    let notified: Vec<Option<String>> = events
        .iter()
        .filter_map(|event| match event {
            Event::Attention(call) => Some(call.notify.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(notified, [Some("hid".to_owned())]);
}

fn sent(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Act(act) => Some(act.line.clone()),
            _ => None,
        })
        .collect()
}

fn warned(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Notice(notice) => Some(notice.lines().join(" ")),
            _ => None,
        })
        .collect()
}

/// A trigger's send is published with its groups filled in; the session
/// does not send it itself, the binary does.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_send_is_published_for_the_binary_to_send() {
    let wire = "<prompt time=\"999\">&gt;</prompt>\nYou could use this opportunity to tackle!\n";
    let (events, _) = run(
        wire,
        vec![Trigger {
            name: "react".into(),
            rule: Rule {
                pattern: Some(Pattern::Regex(
                    r"You could use this opportunity to (\w+)".into(),
                )),
                send: Some("weapon $1".into()),
                ..Rule::default()
            },
        }],
    )
    .await;
    assert_eq!(sent(&events), ["weapon tackle"]);
}

/// Triggers that answer each other are held back past the pace, and said
/// once, not once a line.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn sends_past_the_pace_are_held_back_and_said_once() {
    let lines: Vec<String> = (0..8).map(|i| format!("ping {i}")).collect();
    let wire = format!("<prompt time=\"999\">&gt;</prompt>\n{}\n", lines.join("\n"));
    let triggers = (0..8)
        .map(|i| {
            on_words(
                &format!("t{i}"),
                &format!("ping {i}"),
                Rule {
                    send: Some(format!("pong {i}")),
                    ..Rule::default()
                },
            )
        })
        .collect();
    let (events, _) = run(&wire, triggers).await;
    assert_eq!(
        sent(&events),
        ["pong 0", "pong 1", "pong 2", "pong 3", "pong 4"]
    );
    let said = warned(&events);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].contains("`pong 5` (t5)") && said[0].contains("answering its own line"),
        "{said:?}"
    );
}

/// A trigger that sounds and sends is admitted once in its cooldown, for
/// both: its sound does not start a cooldown that eats its send.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_trigger_that_sounds_and_sends_does_both() {
    let wire = format!("<prompt time=\"999\">&gt;</prompt>\n{SWING}");
    let (events, _) = run(
        &wire,
        vec![on_words(
            "swing",
            "You swing",
            Rule {
                sound: Some("ding.wav".into()),
                send: Some("stance defensive".into()),
                ..Rule::default()
            },
        )],
    )
    .await;
    assert_eq!(called(&events), ["swing"]);
    assert_eq!(sent(&events), ["stance defensive"]);
}
